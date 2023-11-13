use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{self, BufReader},
    net::SocketAddr,
    sync::Arc,
};

use crate::{Helper, ProxyResult};
use rustls::{
    server::ResolvesServerCertUsingSni,
    sign::{self, CertifiedKey},
    Certificate, PrivateKey,
};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    sync::mpsc::{Receiver, Sender},
    sync::Mutex,
};
use tokio_rustls::TlsAcceptor;
use webparse::{Request, Response};
use wenmeng::{ProtError, ProtResult, RecvStream, Server};

use super::{common::CommonConfig, LocationConfig, ServerConfig, UpstreamConfig};

struct InnerHttpOper {
    pub servers: Vec<Arc<ServerConfig>>,
    pub cache_sender:
        HashMap<LocationConfig, (Sender<Request<RecvStream>>, Receiver<ProtResult<Response<RecvStream>>>)>,
}

impl InnerHttpOper {
    pub fn new(http: Vec<Arc<ServerConfig>>) -> Self {
        Self {
            servers: http,
            cache_sender: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    #[serde(default = "Vec::new")]
    pub server: Vec<ServerConfig>,
    #[serde(default = "Vec::new")]
    pub upstream: Vec<UpstreamConfig>,

    #[serde(flatten)]
    #[serde(default = "CommonConfig::new")]
    pub comm: CommonConfig,
}

impl HttpConfig {
    pub fn new() -> Self {
        HttpConfig {
            server: vec![],
            upstream: vec![],
            comm: CommonConfig::new(),
        }
    }

    /// 将配置参数提前共享给子级
    pub fn copy_to_child(&mut self) {
        for server in &mut self.server {
            server.upstream.append(&mut self.upstream.clone());
            server.comm.copy_from_parent(&self.comm);
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

    // async fn inner_http_request(
    //     http: &HttpConfig,
    //     req: Request<RecvStream>,
    // ) -> ProtResult<(
    //     Response<RecvStream>,
    //     Option<Sender<Request<RecvStream>>>,
    //     Option<Receiver<Response<RecvStream>>>,
    // )> {
    //     let http = value.http.lock().await;
    //     let server_len = http.server.len();
    //     let host = req.get_host().unwrap_or(String::new());
    //     // 不管有没有匹配, 都执行最后一个
    //     for (index, s) in http.server.iter().enumerate() {
    //         if s.server_name == host || host.is_empty() || index == server_len - 1 {
    //             let path = req.path().clone();
    //             for l in s.location.iter() {
    //                 if l.is_match_rule(&path, req.method()) {
    //                     let (res, sender, receiver) = l.deal_request(req).await?;
    //                     value.sender = sender;
    //                     value.receiver = receiver;
    //                     return Ok(res);
    //                 }
    //             }
    //             return Ok(Response::builder()
    //                 .status(503)
    //                 .body("unknow location to deal")
    //                 .unwrap()
    //                 .into_type());
    //         }
    //     }
    //     return Ok(Response::builder()
    //         .status(503)
    //         .body("unknow location")
    //         .unwrap()
    //         .into_type());
    // }

    async fn inner_operate_by_http(
        req: Request<RecvStream>,
        cache: &mut HashMap<
            LocationConfig,
            (Sender<Request<RecvStream>>, Receiver<ProtResult<Response<RecvStream>>>),
        >,
        servers: Vec<Arc<ServerConfig>>,
    ) -> ProtResult<Response<RecvStream>> {
        let server_len = servers.len();
        let host = req.get_host().unwrap_or(String::new());
        // 不管有没有匹配, 都执行最后一个
        for (index, s) in servers.iter().enumerate() {
            if s.server_name == host || host.is_empty() || index == server_len - 1 {
                let path = req.path().clone();
                for l in s.location.iter() {
                    if l.is_match_rule(&path, req.method()) {
                        let clone = l.clone_only_hash();
                        if cache.contains_key(&clone) {
                            let mut cache_client = cache.remove(&clone).unwrap();
                            if !cache_client.0.is_closed() {
                                let send = cache_client.0.send(req).await;
                                println!("send request = {:?}", send);
                                match cache_client.1.recv().await {
                                    Some(res) => {
                                        if res.is_ok() {
                                            println!("cache client receive  response");
                                            cache.insert(clone, cache_client);
                                        }
                                        return res;
                                    }
                                    None => {
                                        println!("cache client close response");
                                        return Ok(Response::status503()
                                            .body("already lose connection")
                                            .unwrap()
                                            .into_type());
                                    }
                                }
                            }
                        }
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
        }
        return Ok(Response::status503()
            .body("unknow location")
            .unwrap()
            .into_type());
    }

    async fn inner_operate(mut req: Request<RecvStream>) -> ProtResult<Response<RecvStream>> {
        let data = req.extensions_mut().remove::<Arc<Mutex<InnerHttpOper>>>();
        if data.is_none() {
            return Err(ProtError::Extension("unknow data"));
        }
        let data = data.unwrap();
        let mut value = data.lock().await;
        let servers = value.servers.clone();
        return Self::inner_operate_by_http(req, &mut value.cache_sender, servers).await;
        
    }

    async fn operate(req: Request<RecvStream>) -> ProtResult<Response<RecvStream>> {
        // body的内容可能重新解密又再重新再加过密, 后续可考虑直接做数据
        match Self::inner_operate(req).await {
            Ok(mut value) => {
                value.headers_mut().insert("server", "wmproxy");
                Ok(value)
            }
            Err(e) => {
                println!("e === {:?}", e);
                let (is_timeout, is_client) = e.is_read_timeout();
                if is_timeout && !is_client {
                    Ok(Response::text().status(408).body("operate timeout")?.into_type())
                } else {
                    Ok(Response::status500().body("server inner error")?.into_type())
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
                .stream_data(inbound, Arc::new(Mutex::new(oper)));
            if let Err(e) = server.incoming(Self::operate).await {
                log::info!("反向代理：处理信息时发生错误：{:?}", e);
            }
        });
        Ok(())
    }
    
    pub fn get_log_names(&self, names: &mut HashMap<String, String>)  {
        self.comm.get_log_names(names);
        for s in &self.server {
            s.get_log_names(names);
        }
    }
}
