use std::net::{SocketAddr, IpAddr};

use commander::Commander;
use tokio::net::{TcpListener, TcpStream};
use webparse::BinaryMut;

use crate::{Flag, ProxyError, ProxyHttp, ProxyResult, ProxySocks5};

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

    pub fn server(self, addr: Option<SocketAddr>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.server = addr;
            Ok(proxy)
        })
    }


    pub fn tls(self, is_tls: bool) -> Builder {
        self.and_then(|mut proxy| {
            proxy.is_tls = is_tls;
            Ok(proxy)
        })
    }

    pub fn username(self, username: Option<String>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.username = username;
            Ok(proxy)
        })
    }
    
    pub fn password(self, password: Option<String>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.password = password;
            Ok(proxy)
        })
    }

    pub fn udp_bind(self, udp_bind: Option<IpAddr>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.udp_bind = udp_bind;
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
    is_tls: bool,
    bind_addr: String,
    bind_port: u16,
    server: Option<SocketAddr>,
    username: Option<String>,
    password: Option<String>,
    udp_bind: Option<IpAddr>,
}

impl Default for Proxy {
    fn default() -> Self {
        Self {
            flag: Flag::HTTP | Flag::HTTPS,
            is_tls: false,
            bind_addr: "127.0.0.1".to_string(),
            bind_port: 8090,
            server: None,
            username: None,
            password: None,
            udp_bind: None,
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
            .usage_desc("wmproxy -p 8090")
            .option_list(
                "-f, --flag [value]",
                "可兼容的方法, 如http https socks5",
                None,
            )
            .option("-t, --tls value", "是否加密端口", Some(true))
            .option_int("-p, --port value", "监听端口", Some(8090))
            .option_str(
                "-b, --bind value",
                "监听地址",
                Some("127.0.0.1".to_string()),
            )
            .option_str(
                "--user value",
                "auth的用户名",
                None,
            ).option_str(
                "-S value",
                "父级的监听端口地址,如127.0.0.1:8091",
                None,
            )
            .option_str(
                "--pass value",
                "auth的密码",
                None,
            ).option_str(
                "--udp value",
                "udp的监听地址,如127.0.0.1,socks5的udp协议用",
                None,
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
        builder = builder.flag(Flag::HTTP | Flag::HTTPS | Flag::SOCKS5);
        builder = builder.username(command.get_str("user"));
        builder = builder.password(command.get_str("pass"));
        builder = builder.tls(command.get("t").unwrap_or(false));
        if let Some(udp) = command.get_str("udp") {
            builder = builder.udp_bind(udp.parse::<IpAddr>().ok());
        };

        if let Some(s) = command.get_str("S") {
            builder = builder.server(s.parse::<SocketAddr>().ok());
        };

        builder.inner
    }

    async fn process_http(flag: Flag, inbound: TcpStream) -> ProxyResult<()> {
        if flag.contains(Flag::HTTP) || flag.contains(Flag::HTTPS) {
            ProxyHttp::process(inbound).await
        } else {
            Err(ProxyError::Continue((None, inbound)))
        }
    }

    
    async fn process_socks5(username: Option<String>, password: Option<String>, udp_bind: Option<IpAddr>, flag: Flag, inbound: TcpStream, buffer: Option<BinaryMut>) -> ProxyResult<()> {
        if flag.contains(Flag::SOCKS5) {
            let mut sock = ProxySocks5::new(username, password, udp_bind);
            sock.process(inbound, buffer).await
        } else {
            Err(ProxyError::Continue((buffer, inbound)))
        }
    }

    pub async fn start_serve(&mut self) -> ProxyResult<()> {
        let addr = format!("{}:{}", self.bind_addr, self.bind_port)
            .parse::<SocketAddr>()
            .map_err(|_| ProxyError::Extension("parse addr error"))?;
        let listener = TcpListener::bind(addr).await?;
        let flag = self.flag;
        while let Ok((inbound, _)) = listener.accept().await {

            if let Some(server) = self.server.clone() {
                tokio::spawn(async move {
                    // 转到上层服务器进行处理
                    let _ = Self::transfer_server(inbound, server).await;
                });
            } else {
                let username = self.username.clone();
                let password = self.password.clone();
                let udp_bind = self.udp_bind.clone();
                tokio::spawn(async move {
                    // tcp的连接被移动到该协程中，我们只要专注的处理该stream即可
                    let _ = Self::deal_proxy(inbound, flag, username, password, udp_bind).await;
                });
            }
            
        }
        Ok(())
    }

    async fn transfer_server(mut inbound: TcpStream, server: SocketAddr) -> ProxyResult<()> {
        let mut outbound = TcpStream::connect(server).await?;
        let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await?;
        Ok(())
    }

    async fn deal_proxy(inbound: TcpStream, flag: Flag, username: Option<String>, password: Option<String>, udp_bind: Option<IpAddr>) -> ProxyResult<()> {
        let (read_buf, inbound) = match Self::process_http(flag, inbound).await {
            Ok(()) => {
                return Ok(());
            }
            Err(ProxyError::Continue(buf)) => buf,
            Err(err) => return Err(err),
        };

        let _read_buf = match Self::process_socks5(username, password, udp_bind, flag, inbound, read_buf).await {
            Ok(()) => {
                return Ok(())
            }
            Err(ProxyError::Continue(buf)) => buf,
            Err(err) => {
                log::trace!("socks5 error {:?}", err);
                println!("socks5 error {:?}", err);
                return Err(err)
            },
        };
        Ok(())
    }
}
// #[derive(Debug)]
// pub struct Builder {
//     inner: WebResult<Parts>,
// }
