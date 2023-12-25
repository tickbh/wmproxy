// Copyright 2022 - 2023 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Author: tickbh
// -----
// Created Date: 2023/10/18 02:32:23

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{self, BufReader},
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use crate::{data::LimitReqData, Helper, ProxyResult};
use async_trait::async_trait;
use rustls::{
    server::ResolvesServerCertUsingSni,
    sign::{self, CertifiedKey},
    Certificate, PrivateKey,
};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    sync::mpsc::{Receiver, Sender},
};
use tokio_rustls::TlsAcceptor;
use webparse::{Request, Response};
use wenmeng::{
    Body, Middleware, OperateTrait, ProtError, ProtResult, RecvRequest, RecvResponse, Server,
};

use super::{
    common::CommonConfig, limit_req::LimitReqZone, LimitReqMiddleware, LocationConfig,
    ServerConfig, UpstreamConfig,
};
use async_recursion::async_recursion;

struct Operate {
    inner: InnerHttpOper,
}

#[async_trait]
impl OperateTrait for Operate {
    async fn operate(&mut self, req: &mut RecvRequest) -> ProtResult<RecvResponse> {
        HttpConfig::operate(req, &mut self.inner).await
    }

    async fn middle_operate(
        &mut self,
        req: &mut RecvRequest,
        middles: &mut Vec<Box<dyn Middleware>>,
    ) -> ProtResult<()> {
        let _req = req;
        let _middle = middles;
        Ok(())
    }
}

struct InnerHttpOper {
    pub servers: Vec<Arc<ServerConfig>>,
    pub cache_sender:
        HashMap<LocationConfig, (Sender<Request<Body>>, Receiver<ProtResult<Response<Body>>>)>,
}

impl InnerHttpOper {
    pub fn new(http: Vec<Arc<ServerConfig>>) -> Self {
        Self {
            servers: http,
            cache_sender: HashMap::new(),
        }
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    #[serde(default = "Vec::new")]
    pub server: Vec<ServerConfig>,
    #[serde(default = "Vec::new")]
    pub upstream: Vec<UpstreamConfig>,

    #[serde_as(as = "HashMap<_, DisplayFromStr>")]
    #[serde(default = "HashMap::new")]
    pub limit_req_zone: HashMap<String, LimitReqZone>,

    #[serde(flatten)]
    #[serde(default = "CommonConfig::new")]
    pub comm: CommonConfig,
}

impl HttpConfig {
    pub fn new() -> Self {
        HttpConfig {
            server: vec![],
            upstream: vec![],
            limit_req_zone: HashMap::new(),
            comm: CommonConfig::new(),
        }
    }

    pub fn after_load_option(&mut self) -> ProtResult<()> {
        self.copy_to_child();
        for (k, zone) in &self.limit_req_zone {
            LimitReqData::cache(k.to_string(), zone.limit, zone.rate.nums, zone.rate.per)?;
        }
        Ok(())
    }

    /// 将配置参数提前共享给子级
    pub fn copy_to_child(&mut self) {
        self.comm.pre_deal();
        for server in &mut self.server {
            server.upstream.append(&mut self.upstream.clone());
            server.comm.copy_from_parent(&self.comm);
            server.comm.pre_deal();
            server.copy_to_child();
        }
    }

    fn load_certs(path: &Option<String>) -> io::Result<Vec<Certificate>> {
        if let Some(path) = path {
            match File::open(&path) {
                Ok(file) => {
                    let mut reader = BufReader::new(file);
                    let certs = rustls_pemfile::certs(&mut reader)?;
                    Ok(certs.into_iter().map(Certificate).collect())
                }
                Err(e) => {
                    log::warn!("加载公钥{}出错，错误内容:{:?}", path, e);
                    return Err(e);
                }
            }
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "unknow certs"))
        }
    }

    fn load_keys(path: &Option<String>) -> io::Result<PrivateKey> {
        let mut keys = if let Some(path) = path {
            match File::open(&path) {
                Ok(file) => {
                    let mut reader = BufReader::new(file);
                    rustls_pemfile::rsa_private_keys(&mut reader)?
                }
                Err(e) => {
                    log::warn!("加载私钥{}出错，错误内容:{:?}", path, e);
                    return Err(e);
                }
            }
        } else {
            return Err(io::Error::new(io::ErrorKind::Other, "unknow keys"));
        };

        match keys.len() {
            0 => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("No RSA private key found"),
            )),
            1 => Ok(PrivateKey(keys.remove(0))),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("More than one RSA private key found"),
            )),
        }
    }

    pub async fn bind(
        &mut self,
    ) -> ProxyResult<(Option<TlsAcceptor>, Vec<bool>, Vec<TcpListener>)> {
        let mut listeners = vec![];
        let mut tlss = vec![];
        let mut bind_port = HashSet::new();
        let config = rustls::ServerConfig::builder().with_safe_defaults();
        let mut resolve = ResolvesServerCertUsingSni::new();
        for value in &self.server.clone() {
            let mut is_ssl = false;
            if value.cert.is_some() && value.key.is_some() {
                let key = sign::any_supported_type(&Self::load_keys(&value.key)?)
                    .map_err(|_| ProtError::Extension("unvaild key"))?;
                let ck = CertifiedKey::new(Self::load_certs(&value.cert)?, key);
                resolve.add(&value.server_name, ck).map_err(|e| {
                    log::warn!("添加证书时失败:{:?}", e);
                    ProtError::Extension("key error")
                })?;
                is_ssl = true;
            }

            if bind_port.contains(&value.bind_addr.port()) {
                continue;
            }
            bind_port.insert(value.bind_addr.port());

            log::info!("负载均衡：{:?}，提供http转发功能。", value.bind_addr);
            let listener = Helper::bind(value.bind_addr).await?;
            listeners.push(listener);
            tlss.push(is_ssl);
        }

        let mut config = config
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(resolve));
        config.alpn_protocols.push("h2".as_bytes().to_vec());
        config.alpn_protocols.push("http/1.1".as_bytes().to_vec());
        Ok((Some(TlsAcceptor::from(Arc::new(config))), tlss, listeners))
    }

    #[async_recursion]
    async fn deal_match_location(
        req: &mut Request<Body>,
        cache: &mut HashMap<
            LocationConfig,
            (Sender<Request<Body>>, Receiver<ProtResult<Response<Body>>>),
        >,
        server: Arc<ServerConfig>,
        mut now: usize,
        deals: &mut HashSet<usize>,
        try_deals: &mut HashSet<usize>,
    ) -> ProtResult<Response<Body>> {
        let path = req.path().clone();
        let mut l = None;
        if now == usize::MAX {
            for idx in 0..server.location.len() {
                if deals.contains(&idx) {
                    continue;
                }
                if server.location[idx].is_match_rule(&path, req.method()) {
                    l = Some(&server.location[idx]);
                    now = idx;
                    break;
                }
            }
        } else {
            if !deals.contains(&now) && now < server.location.len() {
                l = Some(&server.location[now]);
            }
        };
        if l.is_none() {
            return Ok(Response::status503()
                .body("unknow location to deal")
                .unwrap()
                .into_type());
        }


        let l = l.unwrap();
        if let Some(limit_req) = &l.comm.limit_req {
            if let Some(res) = LimitReqMiddleware::new(limit_req.clone())
                .process_request(req)
                .await?
            {
                return Ok(res);
            }
        }
        if l.comm.deny_ip.is_some() || l.comm.allow_ip.is_some() {
            if let Some(ip) = req.headers().system_get("{client_ip}") {
                let ip = ip
                    .parse::<IpAddr>()
                    .map_err(|_| ProtError::Extension("client ip error"))?;
                if let Some(allow) = &l.comm.allow_ip {
                    if !allow.contains(&ip) {
                        return Ok(Response::status503()
                            .body("now allow ip")
                            .unwrap()
                            .into_type());
                    }
                }
                if let Some(deny) = &l.comm.deny_ip {
                    if deny.contains(&ip) {
                        return Ok(Response::status503().body("deny ip").unwrap().into_type());
                    }
                }
            }
        }

        if !try_deals.contains(&now) && l.try_paths.is_some() {
            let try_paths = l.try_paths.as_ref().unwrap();
            try_deals.insert(now);
            let ori_path = req.path().clone();
            for val in try_paths.list.iter() {
                deals.clear();
                req.set_path(ori_path.clone());
                let new_path = Helper::format_req(req, &**val);
                req.set_path(new_path);
                if let Ok(res) = Self::deal_match_location(
                    req,
                    cache,
                    server.clone(),
                    usize::MAX,
                    deals,
                    try_deals,
                )
                .await
                {
                    if !res.status().is_client_error() && !res.status().is_server_error() {
                        return Ok(res);
                    }
                }
            }
            return Ok(Response::builder()
                .status(try_paths.fail_status)
                .body("not valid to try")
                .unwrap()
                .into_type());
        } else {
            deals.insert(now);
            let clone = l.clone_only_hash();
            if cache.contains_key(&clone) {
                let mut cache_client = cache.remove(&clone).unwrap();
                if !cache_client.0.is_closed() {
                    println!("do req data by cache");
                    let _send = cache_client.0.send(req.replace_clone(Body::empty())).await;
                    match cache_client.1.recv().await {
                        Some(res) => {
                            if res.is_ok() {
                                log::trace!("cache client receive response");
                                cache.insert(clone, cache_client);
                            }
                            return res;
                        }
                        None => {
                            log::trace!("cache client close response");
                            return Ok(Response::status503()
                                .body("already lose connection")
                                .unwrap()
                                .into_type());
                        }
                    }
                }
            } else {
                log::trace!("do req data by new");
                let (res, sender, receiver) = l.deal_request(req).await?;
                if sender.is_some() && receiver.is_some() {
                    cache.insert(clone, (sender.unwrap(), receiver.unwrap()));
                }
                return Ok(res);
            }
        }

        return Ok(Response::status503()
            .body("unknow location to deal")
            .unwrap()
            .into_type());
    }

    async fn inner_operate_by_http(
        req: &mut Request<Body>,
        cache: &mut HashMap<
            LocationConfig,
            (Sender<Request<Body>>, Receiver<ProtResult<Response<Body>>>),
        >,
        servers: Vec<Arc<ServerConfig>>,
    ) -> ProtResult<Response<Body>> {
        let server_len = servers.len();
        let host = req.get_host().unwrap_or(String::new());
        // 不管有没有匹配, 都执行最后一个
        for (index, s) in servers.iter().enumerate() {
            if s.server_name == host || host.is_empty() || index == server_len - 1 {
                return Self::deal_match_location(
                    req,
                    cache,
                    s.clone(),
                    usize::MAX,
                    &mut HashSet::new(),
                    &mut HashSet::new(),
                )
                .await;
                // for l in s.location.iter() {
                //     if l.is_match_rule(&path, req.method()) {
                //         if let Some(limit_req) = &l.comm.limit_req {
                //             if let Some(res) = LimitReqMiddleware::new(limit_req.clone())
                //                 .process_request(req)
                //                 .await?
                //             {
                //                 return Ok(res);
                //             }
                //         }
                //         let clone = l.clone_only_hash();
                //         if cache.contains_key(&clone) {
                //             let mut cache_client = cache.remove(&clone).unwrap();
                //             if !cache_client.0.is_closed() {
                //                 println!("do req data by cache");
                //                 let _send =
                //                     cache_client.0.send(req.replace_clone(Body::empty())).await;
                //                 match cache_client.1.recv().await {
                //                     Some(res) => {
                //                         if res.is_ok() {
                //                             log::trace!("cache client receive response");
                //                             cache.insert(clone, cache_client);
                //                         }
                //                         return res;
                //                     }
                //                     None => {
                //                         log::trace!("cache client close response");
                //                         return Ok(Response::status503()
                //                             .body("already lose connection")
                //                             .unwrap()
                //                             .into_type());
                //                     }
                //                 }
                //             }
                //         } else {
                //             log::trace!("do req data by new");
                //             let (res, sender, receiver) = l.deal_request(req).await?;
                //             if sender.is_some() && receiver.is_some() {
                //                 cache.insert(clone, (sender.unwrap(), receiver.unwrap()));
                //             }
                //             return Ok(res);
                //         }
                //     }
                // }
                // return Ok(Response::status503()
                //     .body("unknow location to deal")
                //     .unwrap()
                //     .into_type());
            }
        }
        return Ok(Response::status503()
            .body("unknow location")
            .unwrap()
            .into_type());
    }

    async fn inner_operate(
        req: &mut Request<Body>,
        data: &mut InnerHttpOper,
    ) -> ProtResult<Response<Body>> {
        let servers = data.servers.clone();
        return Self::inner_operate_by_http(req, &mut data.cache_sender, servers).await;
    }

    async fn operate(
        req: &mut Request<Body>,
        data: &mut InnerHttpOper,
    ) -> ProtResult<Response<Body>> {
        // body的内容可能重新解密又再重新再加过密, 后续可考虑直接做数据
        match Self::inner_operate(req, data).await {
            Ok(mut value) => {
                value.headers_mut().insert("server", "wmproxy");
                Ok(value)
            }
            Err(e) => {
                log::trace!("处理HTTP服务发生错误: {:?}", e);
                let (is_timeout, is_client) = e.is_read_timeout();
                if is_timeout && !is_client {
                    Ok(Response::text()
                        .status(408)
                        .body("operate timeout")?
                        .into_type())
                } else {
                    Ok(Response::status500()
                        .body("server inner error")?
                        .into_type())
                }
            }
        }
    }

    pub fn convert_server_config(&self) -> Vec<Arc<ServerConfig>> {
        let mut vec = vec![];
        for v in &self.server {
            vec.push(Arc::new(v.clone()));
        }
        vec
    }

    pub async fn process<T>(
        servers: Vec<Arc<ServerConfig>>,
        inbound: T,
        addr: SocketAddr,
    ) -> ProxyResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin + std::marker::Send + 'static,
    {
        if servers.is_empty() {
            return Err(crate::ProxyError::Extension("unknown server"));
        }
        let oper = InnerHttpOper::new(servers);
        tokio::spawn(async move {
            let timeout = oper.servers[0].comm.build_client_timeout();
            let mut server = Server::builder()
                .addr(addr)
                .timeout_layer(timeout)
                .stream(inbound);

            if let Err(e) = server.incoming(Operate { inner: oper }).await {
                if server.get_req_num() == 0 {
                    log::info!("反向代理：未处理任何请求时发生错误：{:?}", e);
                } else {
                    if !e.is_io() {
                        log::info!("反向代理：处理信息时发生错误：{:?}", e);
                    }
                }
            }
        });
        Ok(())
    }

    pub fn get_log_names(&self, names: &mut HashMap<String, String>) {
        self.comm.get_log_names(names);
        for s in &self.server {
            s.get_log_names(names);
        }
    }
}
