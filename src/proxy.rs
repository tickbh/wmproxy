use std::net::SocketAddr;

use commander::Commander;
use tokio::net::{TcpListener, TcpStream};

use crate::{Flag, ProxyError, ProxyHttp, ProxyResult};

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
            flag: Flag::HTTP | Flag::HTTPS,
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

    pub fn parse_env() -> ProxyResult<Proxy> {
        let command = Commander::new()
            .version(&env!("CARGO_PKG_VERSION").to_string())
            .usage("-b 127.0.0.1 -p 8090")
            .usage_desc("use http proxy")
            .option_list(
                "-f, --flag [value]",
                "可兼容的方法, 如http https socks5",
                None,
            )
            .option_int("-p, --port [value]", "listen port", Some(8090))
            .option_str(
                "-b, --bind [value]",
                "bind addr",
                Some("0.0.0.0".to_string()),
            )
            .parse_env_or_exit();

        let listen_port: u16 = command.get_int("p").unwrap() as u16;
        let listen_host = command.get_str("b").unwrap();

        let mut builder = Self::builder().bind_port(listen_port);
        println!("listener bind {} {}", listen_host, listen_port);
        match format!("{}:{}", listen_host, listen_port).parse::<SocketAddr>() {
            Err(_) => {
                builder = builder.bind_addr("127.0.0.1".to_string());
            }
            Ok(_) => {
                builder = builder.bind_addr(listen_host);
            }
        };

        builder.inner
    }

    async fn process_http(flag: Flag, inbound: &mut TcpStream) -> ProxyResult<bool> {
        if flag.contains(Flag::HTTP) || flag.contains(Flag::HTTPS) {
            ProxyHttp::process(inbound).await
        } else {
            Ok(false)
        }
    }

    pub async fn start_serve(&mut self) -> ProxyResult<()> {
        let addr = format!("{}:{}", self.bind_addr, self.bind_port)
            .parse::<SocketAddr>()
            .map_err(|_| ProxyError::Extension("parse addr error"))?;
        let listener = TcpListener::bind(addr).await?;
        let flag = self.flag;
        while let Ok((mut inbound, _)) = listener.accept().await {
            tokio::spawn(async move {
                let read_buf = match Self::process_http(flag, &mut inbound).await {
                    Ok(true) => {
                        return;
                    }
                    Ok(false) => None,
                    Err(ProxyError::Continue(buf)) => Some(buf),
                    Err(_) => return,
                };
            });
        }
        Ok(())
    }
}
// #[derive(Debug)]
// pub struct Builder {
//     inner: WebResult<Parts>,
// }
