use std::io;

use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

mod wrap_listener;
mod wrap_tls_accepter;

pub use wrap_tls_accepter::WrapTlsAccepter;

use crate::Helper;

pub use self::wrap_listener::WrapListener;

pub struct Listeners {
    pub listener: Vec<WrapListener>,
}

impl Listeners {
    pub fn new() -> Self {
        Self { listener: vec![] }
    }

    pub fn add(&mut self, listener: WrapListener) {
        self.listener.push(listener);
    }
}

