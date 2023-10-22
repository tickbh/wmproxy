use std::{
    collections::HashSet,
    fs::File,
    io::{self, BufReader},
    net::SocketAddr,
    sync::Arc,
};

use crate::{ProxyResult};
use rustls::{
    server::ResolvesServerCertUsingSni,
    sign::{self, CertifiedKey},
    Certificate, PrivateKey,
};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    sync::Mutex,
};
use tokio_rustls::TlsAcceptor;
use webparse::{Request, Response};
use wenmeng::{ProtError, ProtResult, RecvStream, Server};

use super::{ServerConfig, UpstreamConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    #[serde(default = "Vec::new")]
    pub server: Vec<ServerConfig>,
    #[serde(default = "Vec::new")]
    pub upstream: Vec<UpstreamConfig>,
}

impl HttpConfig {
    pub fn new() -> Self {
        HttpConfig {
            server: vec![],
            upstream: vec![],
        }
    }
    
    /// 将配置参数提前共享给子级
    pub fn copy_to_child(&mut self) {
        for server in &mut self.server {
            server.upstream.append(&mut self.upstream.clone());
            server.copy_to_child();
        }
    }

    fn load_certs(path: &Option<String>) -> io::Result<Vec<Certificate>> {
        if let Some(path) = path {
            let file = File::open(path)?;
            let mut reader = BufReader::new(file);
            let certs = rustls_pemfile::certs(&mut reader)?;
            Ok(certs.into_iter().map(Certificate).collect())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "unknow certs"))
        }
    }

    fn load_keys(path: &Option<String>) -> io::Result<PrivateKey> {
        let mut keys = if let Some(path) = path {
            let file = File::open(&path)?;
            let mut reader = BufReader::new(file);
            rustls_pemfile::rsa_private_keys(&mut reader)?
        } else {
            return Err(io::Error::new(io::ErrorKind::Other, "unknow keys"));
        };

        match keys.len() {
            0 => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("No PKCS8-encoded private key found"),
            )),
            1 => Ok(PrivateKey(keys.remove(0))),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("More than one PKCS8-encoded private key found"),
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
                    println!("{:?}", e);
                    ProtError::Extension("key error")
                })?;
                is_ssl = true;
            }

            if bind_port.contains(&value.bind_addr.port()) {
                continue;
            }
            bind_port.insert(value.bind_addr.port());
            let listener = TcpListener::bind(value.bind_addr).await?;
            listeners.push(listener);
            tlss.push(is_ssl);
        }

        let config = config
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(resolve));
        Ok((Some(TlsAcceptor::from(Arc::new(config))), tlss, listeners))
    }

    async fn inner_operate(mut req: Request<RecvStream>) -> ProtResult<Response<RecvStream>> {
        println!("receiver req = {:?}", req.url());
        let data = req.extensions_mut().remove::<Arc<Mutex<Arc<HttpConfig>>>>();
        if data.is_none() {
            return Err(ProtError::Extension("unknow data"));
        }
        let data = data.unwrap();
        let value = data.lock().await;
        let server_len = value.server.len();
        let host = req.get_host().unwrap_or(String::new());
        // 不管有没有匹配, 都执行最后一个
        for (index, s) in value.server.iter().enumerate() {
            if s.server_name == host || host.is_empty() || index == server_len - 1 {
                let path = req.path().clone();
                for l in s.location.iter() {
                    if l.is_match_rule(&path, req.method()) {
                        return l.deal_request(req).await;
                    }
                }
                return Ok(Response::builder()
                    .status(503)
                    .body("unknow location to deal")
                    .unwrap()
                    .into_type());
            }
        }
        return Ok(Response::builder()
            .status(503)
            .body("unknow location")
            .unwrap()
            .into_type());
    }

    async fn operate(req: Request<RecvStream>) -> ProtResult<Response<RecvStream>> {
        let mut value = Self::inner_operate(req).await?;
        value.headers_mut().insert("server", "wmproxy");
        Ok(value)
    }

    pub async fn process<T>(http: Arc<HttpConfig>, inbound: T, addr: SocketAddr) -> ProxyResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin + std::marker::Send + 'static,
    {
        tokio::spawn(async move {
            let mut server = Server::new(inbound, Some(addr), http);
            if let Err(e) = server.incoming(Self::operate).await {
                log::info!("反向代理：处理信息时发生错误：{:?}", e);
            }
        });
        Ok(())
    }
}
