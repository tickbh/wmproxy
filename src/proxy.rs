use std::{
    io::{self},
    net::{IpAddr, SocketAddr},
    process,
    sync::Arc,
};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
};
use tokio_rustls::{rustls, TlsAcceptor, TlsConnector};
use webparse::BinaryMut;

use crate::{
    error::ProxyTypeResult, CenterClient, CenterServer, Flag, ProxyError,
    ProxyHttp, ProxyOption, ProxyResult, ProxySocks5,
};

pub struct Proxy {
    option: ProxyOption,
    center_client: Option<CenterClient>,
    center_servers: Vec<CenterServer>,
}

impl Proxy {
    pub fn new(option: ProxyOption) -> Proxy {
        Self {
            option,
            center_client: None,
            center_servers: vec![],
        }
    }

    async fn process_http<T>(flag: Flag, inbound: T) -> ProxyTypeResult<(), T>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        if flag.contains(Flag::HTTP) || flag.contains(Flag::HTTPS) {
            ProxyHttp::process(inbound).await
        } else {
            Err(ProxyError::Continue((None, inbound)))
        }
    }

    async fn process_socks5<T>(
        username: Option<String>,
        password: Option<String>,
        udp_bind: Option<IpAddr>,
        flag: Flag,
        inbound: T,
        buffer: Option<BinaryMut>,
    ) -> ProxyTypeResult<(), T>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        if flag.contains(Flag::SOCKS5) {
            let mut sock = ProxySocks5::new(username, password, udp_bind);
            sock.process(inbound, buffer).await
        } else {
            Err(ProxyError::Continue((buffer, inbound)))
        }
    }

    async fn deal_stream<T>(
        &mut self,
        inbound: T,
        tls_client: Option<Arc<rustls::ClientConfig>>,
    ) -> ProxyResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    {
        // 转发到服务端
        if let Some(client) = &mut self.center_client {
            return client.deal_new_stream(inbound).await;
        }

        // 服务端开代理, 接收到客户端一律用协议处理
        if self.option.center && self.option.is_server() {
            let server = CenterServer::new(self.option.clone());
            self.center_servers.push(server);
            return self.center_servers.last_mut().unwrap().serve(inbound).await;
        }

        println!(
            "server = {:?} tc = {:?} ts = {:?} tls_client = {:?}",
            self.option.server,
            self.option.tc,
            self.option.ts,
            tls_client.is_some()
        );
        let flag = self.option.flag;
        let domain = self.option.domain.clone();
        if let Some(server) = self.option.server.clone() {
            tokio::spawn(async move {
                // 转到上层服务器进行处理
                let _e = Self::transfer_server(domain, tls_client, inbound, server).await;
            });
        } else {
            let username = self.option.username.clone();
            let password = self.option.password.clone();
            let udp_bind = self.option.udp_bind.clone();
            tokio::spawn(async move {
                // tcp的连接被移动到该协程中，我们只要专注的处理该stream即可
                let _ = Self::deal_proxy(inbound, flag, username, password, udp_bind).await;
            });
        }

        Ok(())
    }

    pub async fn start_serve(&mut self) -> ProxyResult<()> {
        let addr = self.option.bind_addr.clone();
        let accept = self.option.get_tls_accept().await.ok();
        let client = self.option.get_tls_request().await.ok();
        if self.option.center {
            if let Some(server) = self.option.server.clone() {
                let mut center_client = CenterClient::new(
                    server,
                    client.clone(),
                    self.option.domain.clone(),
                    self.option.mappings.clone(),
                );
                match center_client.connect().await {
                    Ok(true) => (),
                    Ok(false) => {
                        println!("未能正确连上服务端:{:?}", self.option.server.unwrap());
                        process::exit(1);
                    }
                    Err(err) => {
                        println!(
                            "未能正确连上服务端:{:?}, 发生错误:{:?}",
                            self.option.server.unwrap(),
                            err
                        );
                        process::exit(1);
                    }
                }
                let _ = center_client.serve().await;
                self.center_client = Some(center_client);
            }
        }
        let center_listener = TcpListener::bind(addr).await?;
        println!(
            "accept = {:?} client = {:?}",
            accept.is_some(),
            client.is_some()
        );
        async fn tcp_listen_work(listen: &Option<TcpListener>) -> Option<TcpStream> {
            if listen.is_some() {
                match listen.as_ref().unwrap().accept().await {
                    Ok((tcp, _)) => Some(tcp),
                    Err(_e) => None,
                }
            } else {
                let pend = std::future::pending();
                let () = pend.await;
                None
            }
            // do work
        }

        let http_listener = if let Some(ls) = &self.option.map_http_bind {
            Some(TcpListener::bind(ls).await?)
        } else {
            None
        };
        let mut https_listener = if let Some(ls) = &self.option.map_https_bind {
            Some(TcpListener::bind(ls).await?)
        } else {
            None
        };

        let map_accept = if https_listener.is_some() {
            let map_accept = self.option.get_map_tls_accept().await.ok();
            if map_accept.is_none() {
                let _ = https_listener.take();
            }
            map_accept
        } else {
            None
        };

        let tcp_listener = if let Some(ls) = &self.option.map_tcp_bind {
            Some(TcpListener::bind(ls).await?)
        } else {
            None
        };

        // let pending = std::future::pending();
        // let fut: &mut dyn Future<Output = ()> = option_fut.as_mut().unwrap_or(&mut pending);

        loop {
            tokio::select! {
                v = center_listener.accept() => {
                    let (inbound, _) = v?;
                    if let Some(a) = accept.clone() {
                        let inbound = a.accept(inbound).await;
                        // 获取的流跟正常内容一样读写, 在内部实现了自动加解密
                        if let Ok(inbound) = inbound {
                            let _ = self.deal_stream(inbound, client.clone()).await;
                        } else {
                            println!("accept error = {:?}", inbound.err());
                        }
                    } else {
                        let _ = self.deal_stream(inbound, client.clone()).await;
                    };
                }
                Some(inbound) = tcp_listen_work(&http_listener) => {
                    self.server_new_http(inbound).await?;
                }
                Some(inbound) = tcp_listen_work(&https_listener) => {
                    self.server_new_https(inbound, map_accept.clone().unwrap()).await?;
                }
                Some(inbound) = tcp_listen_work(&tcp_listener) => {
                    self.server_new_tcp(inbound).await?;
                }
            }
            println!("aaaaaaaaaaaaaa");
        }
    }

    async fn transfer_server<T>(
        domain: Option<String>,
        tls_client: Option<Arc<rustls::ClientConfig>>,
        mut inbound: T,
        server: SocketAddr,
    ) -> ProxyResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        if tls_client.is_some() {
            println!("connect by tls");
            let connector = TlsConnector::from(tls_client.unwrap());
            let stream = TcpStream::connect(&server).await?;
            // 这里的域名只为认证设置
            let domain =
                rustls::ServerName::try_from(&*domain.unwrap_or("soft.wm-proxy.com".to_string()))
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname"))?;

            if let Ok(mut outbound) = connector.connect(domain, stream).await {
                // connect 之后的流跟正常内容一样读写, 在内部实现了自动加解密
                let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await?;
            } else {
                // TODO 返回对应协议的错误
            }
        } else {
            println!("connect by normal");
            if let Ok(mut outbound) = TcpStream::connect(server).await {
                let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await?;
            } else {
                // TODO 返回对应协议的错误
            }
        }
        Ok(())
    }

    pub async fn deal_proxy<T>(
        inbound: T,
        flag: Flag,
        username: Option<String>,
        password: Option<String>,
        udp_bind: Option<IpAddr>,
    ) -> ProxyTypeResult<(), T>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let (read_buf, inbound) = match Self::process_http(flag, inbound).await {
            Ok(()) => {
                return Ok(());
            }
            Err(ProxyError::Continue(buf)) => buf,
            Err(err) => return Err(err),
        };

        let _read_buf =
            match Self::process_socks5(username, password, udp_bind, flag, inbound, read_buf).await
            {
                Ok(()) => return Ok(()),
                Err(ProxyError::Continue(buf)) => buf,
                Err(err) => {
                    // log::trace!("socks5 error {:?}", err);
                    // println!("socks5 error {:?}", err);
                    return Err(err);
                }
            };
        Ok(())
    }

    pub async fn server_new_http(&mut self, stream: TcpStream) -> ProxyResult<()> {
        for server in &mut self.center_servers {
            if !server.is_close() {
                return server.server_new_http(stream).await;
            }
        }
        println!("no any clinet!!!!!!!!!!!!!!");
        Ok(())
    }

    pub async fn server_new_https(
        &mut self,
        stream: TcpStream,
        accept: TlsAcceptor,
    ) -> ProxyResult<()> {
        for server in &mut self.center_servers {
            if !server.is_close() {
                return server.server_new_https(stream, accept).await;
            }
        }
        println!("no any clinet!!!!!!!!!!!!!!");
        Ok(())
    }

    pub async fn server_new_tcp(&mut self, stream: TcpStream) -> ProxyResult<()> {
        for server in &mut self.center_servers {
            if !server.is_close() {
                return server.server_new_tcp(stream).await;
            }
        }
        println!("no any clinet!!!!!!!!!!!!!!");
        Ok(())
    }
}
