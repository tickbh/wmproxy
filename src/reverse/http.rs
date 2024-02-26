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
    collections::{HashMap, HashSet}, net::{IpAddr, SocketAddr}, str::FromStr, sync::Arc
};

use crate::{data::LimitReqData, FileServer, Helper, ProxyResult};
use async_trait::async_trait;
use console::Style;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    sync::mpsc::{Receiver, Sender},
};
use webparse::{Request, Response};
use wenmeng::{
    Body, HttpTrait, Middleware, ProtError, ProtResult, RecvRequest, RecvResponse, Server,
};

use super::{
    common::CommonConfig, limit_req::LimitReqZone, ws::ServerWsOperate, LimitReqMiddleware, LocationConfig, Matcher, ServerConfig, UpstreamConfig, WrapTlsAccepter
};
use async_recursion::async_recursion;

struct Operate {
    inner: InnerHttpOper,
}

#[async_trait]
impl HttpTrait for Operate {
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

    fn is_continue_next(&self) -> bool {
        true
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
        if !self.comm.log_format.contains_key(&"main".to_string()) {
            self.comm.log_format.insert("main".to_string(), "{d(%Y-%m-%d %H:%M:%S)} {client_ip} {l} {url} path:{path} query:{query} host:{host} status: {status} {up_status} referer: {referer} user_agent: {user_agent} cookie: {cookie}".to_string());
        }
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

    pub async fn bind(
        &mut self,
    ) -> ProxyResult<(Vec<Option<WrapTlsAccepter>>, Vec<bool>, Vec<TcpListener>)> {
        let mut listeners = vec![];
        let mut tlss = vec![];
        let mut bind_addr_set = HashSet::new();
        let mut accepters = vec![];
        for value in &mut self.server {
            for v in &value.bind_addr.0 {
                if bind_addr_set.contains(&v) {
                    continue;
                }
                bind_addr_set.insert(v);
                let url = format!("http://{}", v);
                log::info!(
                    "HTTP服务：{}，提供http处理及转发功能。",
                    Style::new().blink().green().apply_to(url)
                );
                let listener = Helper::bind(v).await?;
                listeners.push(listener);
                tlss.push(false);
                accepters.push(None);
            }

            let mut has_acme = false;
            for v in &value.bind_ssl.0 {
                if bind_addr_set.contains(&v) {
                    continue;
                }
                bind_addr_set.insert(v);
                let url = format!("https://{}", v);
                log::info!(
                    "HTTPs服务：{}，提供https处理及转发功能。",
                    Style::new().blink().green().apply_to(url)
                );
                let listener = Helper::bind(v).await?;
                listeners.push(listener);
                if value.cert.is_some() && value.key.is_some() {
                    accepters.push(Some(WrapTlsAccepter::new_cert(&value.cert, &value.key)?));
                } else {
                    has_acme = true;
                    let (_, domain) = value.get_addr_domain()?;
                    if domain.is_none() {
                        return Err(crate::ProxyError::Extension("未配置域名且未配置证书"));
                    }
                    accepters.push(Some(WrapTlsAccepter::new(domain.unwrap())));
                    let mut has_http = false;
                    for v in &value.bind_addr.0 {
                        if v.port() == 80 {
                            has_http = true;
                        }
                    }
                    if !has_http {
                        return Err(crate::ProxyError::Extension("未配置证书需要求HTTP端口配合"));
                    }
                }
                tlss.push(true);
            }

            if has_acme {
                let mut location = LocationConfig::new();
                let file_server = FileServer::new(
                    ".well-known/acme-challenge".to_string(),
                    "/.well-known/acme-challenge".to_string(),
                );
                location.rule = Matcher::from_str("/.well-known/acme-challenge/").expect("matcher error");
                location.file_server = Some(file_server);
                value.location.insert(0, location);
            }
        }
        Ok((accepters, tlss, listeners))
    }

    #[async_recursion]
    async fn deal_match_location(
        req: &mut Request<Body>,
        // 缓存客户端请求
        cache: &mut HashMap<
            LocationConfig,
            (Sender<Request<Body>>, Receiver<ProtResult<Response<Body>>>),
        >,
        // 该Server的配置选项
        server: Arc<ServerConfig>,
        // 已处理的匹配路由
        deals: &mut HashSet<usize>,
        // 已处理的TryPath匹配路由
        try_deals: &mut HashSet<usize>,
    ) -> ProtResult<Response<Body>> {
        let path = req.path().clone();
        let mut l = None;
        let mut now = usize::MAX;
        for idx in 0..server.location.len() {
            if deals.contains(&idx) {
                continue;
            }
            if server.location[idx].is_match_rule(&path, req) {
                l = Some(&server.location[idx]);
                now = idx;
                break;
            }
        }
        if l.is_none() {
            return Ok(Response::status404()
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

        // 判定该try是否处理过, 防止死循环
        if !try_deals.contains(&now) && l.try_paths.is_some() {
            let try_paths = l.try_paths.as_ref().unwrap();
            try_deals.insert(now);
            let ori_path = req.path().clone();
            for val in try_paths.list.iter() {
                deals.clear();
                // 重写path好方便做数据格式化
                req.set_path(ori_path.clone());
                let new_path = Helper::format_req_may_regex(req, &**val);
                // 重写path好方便后续处理无感
                req.set_path(new_path);
                if let Ok(res) =
                    Self::deal_match_location(req, cache, server.clone(), deals, try_deals).await
                {
                    if !res.status().is_client_error() && !res.status().is_server_error() {
                        return Ok(res);
                    }
                }
            }
            return Ok(Response::text()
                .status(try_paths.fail_status)
                .body("未发现合适的Try进行服务")
                .unwrap()
                .into_type());
        } else {
            deals.insert(now);
            let clone = l.clone_only_hash();
            if cache.contains_key(&clone) {
                let mut cache_client = cache.remove(&clone).unwrap();
                if !cache_client.0.is_closed() {
                    let _send = cache_client.0.send(req.replace_clone(Body::empty())).await;
                    match cache_client.1.recv().await {
                        Some(res) => {
                            if let Ok(r) = &res {
                                log::trace!("复用连接收到Response {}", r.status());
                                cache.insert(clone, cache_client);
                            }
                            return res;
                        }
                        None => {
                            log::trace!("复用连接收到空消息,关闭复用连接");
                            return Ok(Response::status503()
                                .body("意外的服务端关闭连接")
                                .unwrap()
                                .into_type());
                        }
                    }
                }
            } else {
                let (res, sender, receiver) = l.deal_request(req).await?;
                if sender.is_some() && receiver.is_some() {
                    cache.insert(clone, (sender.unwrap(), receiver.unwrap()));
                }
                for h in res.headers().iter() {
                    println!("header = {}:{:?}", h.0, h.1.as_string());
                }
                return Ok(res);
            }
        }

        return Ok(Response::status404()
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
            if s.up_name == host || host.is_empty() || index == server_len - 1 {
                return Self::deal_match_location(
                    req,
                    cache,
                    s.clone(),
                    &mut HashSet::new(),
                    &mut HashSet::new(),
                )
                .await;
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
        let oper = InnerHttpOper::new(servers.clone());
        tokio::spawn(async move {
            let timeout = oper.servers[0].comm.build_client_timeout();
            let mut server = Server::builder()
                .addr(addr)
                .timeout_layer(timeout)
                .stream(inbound);
            // 设置HTTP回调
            server.set_callback_http(Box::new(Operate { inner: oper }));
            // 设置websocket回调,客户端有可能升级到websocket协议
            server.set_callback_ws(Box::new(ServerWsOperate::new(servers)));
            if let Err(e) = server.incoming().await {
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
