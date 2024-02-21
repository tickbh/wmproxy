use std::{fs::File, io::{self, BufReader}, sync::Arc};

use rustls::{pki_types::{CertificateDer, PrivateKeyDer}, ServerConnection};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::{Accept, TlsAcceptor};

#[derive(Clone)]
pub struct WrapTlsAccepter {
    pub domain: Option<String>,
    pub accepter: Option<TlsAcceptor>,
}

impl WrapTlsAccepter {
    fn load_certs(path: &Option<String>) -> io::Result<Vec<CertificateDer<'static>>> {
        if let Some(path) = path {
            match File::open(&path) {
                Ok(file) => {
                    let mut reader = BufReader::new(file);
                    let certs = rustls_pemfile::certs(&mut reader);
                    Ok(certs.into_iter().collect::<Result<Vec<_>, _>>()?)
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

    fn load_keys(path: &Option<String>) -> io::Result<PrivateKeyDer<'static>> {
        let mut keys = if let Some(path) = path {
            match File::open(&path) {
                Ok(file) => {
                    let mut reader = BufReader::new(file);
                    rustls_pemfile::rsa_private_keys(&mut reader).collect::<Result<Vec<_>, _>>()?
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
            1 => Ok(PrivateKeyDer::from(keys.remove(0))),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("More than one RSA private key found"),
            )),
        }
    }
    
    pub fn new(domain: String) -> WrapTlsAccepter {
        WrapTlsAccepter {
            domain: Some(domain),
            accepter: None,
        }
    }

    pub fn is_wait_acme(&self) -> bool {
        self.accepter.is_none()
    }
    
    pub fn new_cert(cert: &Option<String>, key: &Option<String>) -> io::Result<WrapTlsAccepter> {
        let one_key = Self::load_keys(&key)?;
        let one_cert = Self::load_certs(&cert)?;
        let config = rustls::ServerConfig::builder();
        let mut config = config
        .with_no_client_auth()
        .with_single_cert(one_cert, one_key)
        .map_err(|e| {
            log::warn!("添加证书时失败:{:?}", e);
            io::Error::new(io::ErrorKind::Other, "key error")
        })?;
        config.alpn_protocols.push("h2".as_bytes().to_vec());
        config.alpn_protocols.push("http/1.1".as_bytes().to_vec());
        Ok(WrapTlsAccepter {
            domain: None,
            accepter: Some(TlsAcceptor::from(Arc::new(config))),
        })
    }

    #[inline]
    pub fn accept<IO>(&self, stream: IO) -> Accept<IO>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        self.accept_with(stream, |_| ())
    }

    pub fn accept_with<IO, F>(&self, stream: IO, f: F) -> Accept<IO>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
        F: FnOnce(&mut ServerConnection),
    {
        if let Some(a) = &self.accepter {
            a.accept_with(stream, f)
        } else {
            unreachable!()
        }
    }
}
