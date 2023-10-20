use std::{
    collections::HashSet,
    fs::File,
    io::{self, BufReader},
    net::SocketAddr,
    sync::Arc,
};

use crate::{reverse::LocationConfig, ProxyResult};
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
use wenmeng::{FileServer, ProtError, ProtResult, RecvStream, Server};

use super::ServerConfig;

fn default_servers() -> Vec<ServerConfig> {
    vec![]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    #[serde(default = "default_servers")]
    pub server: Vec<ServerConfig>,
}

impl HttpConfig {
    pub fn new() -> Self {
        HttpConfig { server: vec![] }
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
                    println!("{:?}", e); ProtError::Extension("key error")
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
        let mut value = data.lock().await;
        let server_len = value.server.len();
        let host = req.get_host().unwrap_or(String::new());
        // 不管有没有匹配, 都执行最后一个
        for (index, s) in value.server.iter().enumerate() {
            if s.server_name == host || host.is_empty() || index == server_len - 1 {
                let path = req.path().clone();
                for (index, l) in s.location.iter().enumerate() {
                    if l.is_match_rule(&path) {
                        return LocationConfig::deal_request(&mut s.clone(), index, req).await;
                    }
                }
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
        println!("xxxxxxxxxxxxxxxxxxxx");
        tokio::spawn(async move {
            let mut server = Server::new(inbound, Some(addr), http);
            let _ret = server.incoming(Self::operate).await;
            if _ret.is_err() {
                println!("ret = {:?}", _ret);
            };
        });
        Ok(())
    }
}
