use std::net::SocketAddr;

use crate::{Flag, ProxyResult};

pub struct Builder {
    inner: ProxyResult<Proxy>,
}

impl Builder {
    #[inline]
    pub fn new() -> Builder {
        Builder {
            inner: Ok(Proxy::default()),
        }
    }

    pub fn flag(self, flag: Flag) -> Builder {
        self.and_then(|mut proxy| {
            proxy.flag = flag;
            Ok(proxy)
        })
    }

    pub fn add_flag(self, flag: Flag) -> Builder {
        self.and_then(|mut proxy| {
            proxy.flag.set(flag, true);
            Ok(proxy)
        })
    }
    
    pub fn bind_addr(self, addr: String) -> Builder {
        self.and_then(|mut proxy| {
            proxy.bind_addr = addr;
            Ok(proxy)
        })
    }

    pub fn bind_port(self, port: u16) -> Builder {
        self.and_then(|mut proxy| {
            proxy.bind_port = port;
            Ok(proxy)
        })
    }

    pub fn server(self, addr: SocketAddr) -> Builder {
        self.and_then(|mut proxy| {
            proxy.server = Some(addr);
            Ok(proxy)
        })
    }


    fn and_then<F>(self, func: F) -> Self
    where
        F: FnOnce(Proxy) -> ProxyResult<Proxy>,
    {
        Builder {
            inner: self.inner.and_then(func),
        }
    }
}

/// 代理类, 一个代理类启动一种类型的代理
pub struct Proxy {
    flag: Flag,
    bind_addr: String,
    bind_port: u16,
    server: Option<SocketAddr>,
}

impl Default for Proxy {
    fn default() -> Self {
        Self {
            flag: Flag::empty(),
            bind_addr: "127.0.0.1".to_string(),
            bind_port: 8090,
            server: None,
        }
    }
}

impl Proxy {
    pub fn builder() -> Builder {
        Builder::new()
    }
}
// #[derive(Debug)]
// pub struct Builder {
//     inner: WebResult<Parts>,
// }
