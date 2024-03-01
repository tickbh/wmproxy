use std::io;

use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;


use crate::{core::Stream, Helper};

use super::wrap_tls_accepter::WrapTlsAccepter;

pub struct WrapListener {
    pub listener: TcpListener,
    pub accepter: Option<WrapTlsAccepter>,
}

impl WrapListener {
    pub async fn new(bind: &str) -> io::Result<WrapListener> {
        let listener = Helper::bind(bind).await?;
        Ok(Self::new_listener(listener))
    }

    pub fn new_listener(listener: TcpListener) -> WrapListener {
        Self {
            listener,
            accepter: None,
        }
    }

    pub async fn new_tls(bind: &str, cert: &str, key: &str) -> io::Result<WrapListener> {
        let listener = Helper::bind(bind).await?;
        Self::new_listener_tls(listener, cert, key).await
    }

    pub async fn new_listener_tls(
        listener: TcpListener,
        cert: &str,
        key: &str,
    ) -> io::Result<WrapListener> {
        let accepter = WrapTlsAccepter::new_cert(&Some(cert.to_string()), &Some(key.to_string()))?;
        Ok(Self {
            listener,
            accepter: Some(accepter),
        })
    }

    // pub async fn accept(&mut self) -> io::Result<Stream> {
    //     let stream = self.listener.accept().await?;
        
    // }
}
