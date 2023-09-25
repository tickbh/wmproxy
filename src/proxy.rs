use std::{
    io::{self},
    net::{IpAddr, SocketAddr},
    sync::Arc, process,
};



use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    sync::mpsc::{Receiver, Sender},
};
use tokio_rustls::{rustls, TlsConnector};
use webparse::BinaryMut;

use crate::{
    error::ProxyTypeResult,
    prot::{ProtFrame},
    Flag, ProxyError, ProxyHttp, ProxyResult, ProxySocks5, CenterClient, ProxyOption,
};

pub struct Proxy {
    option: ProxyOption,
    center_client: Option<CenterClient>,
    // center_server: Option<CenterServer>,
}

impl Proxy {
    pub fn new(option: ProxyOption) -> Proxy {
        Self { option, center_client: None }
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

    async fn deal_center_stream<T>(
        &mut self,
        inbound: T,
    ) -> ProxyResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    {
        if let Some(client) = &mut self.center_client {
            return client.deal_new_stream(inbound).await;
        }
        // CenterServer
        if self.option.proxy {

        }
        // CenterClient
        if let Some(_) = &self.option.server {
            tokio::spawn(async move {
                let _ = Self::transfer_center_server(None, None, inbound).await;
            });
        }
        // let flag = self.flag;
        // let domain = self.domain.clone();
        // if let Some(server) = self.server.clone() {
        //     tokio::spawn(async move {
        //         // 转到上层服务器进行处理
        //         let _e = Self::transfer_server(domain, tls_client, inbound, server).await;
        //     });
        // } else {
        //     let username = self.username.clone();
        //     let password = self.password.clone();
        //     let udp_bind = self.udp_bind.clone();
        //     tokio::spawn(async move {
        //         // tcp的连接被移动到该协程中，我们只要专注的处理该stream即可
        //         let _ = Self::deal_proxy(inbound, flag, username, password, udp_bind).await;
        //     });
        // }

        Ok(())
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
        if self.center_client.is_some() {
            return self.deal_center_stream(inbound).await;
        }

        // 服务端开代理
        if self.option.proxy {

        }

        if self.option.center {
            return self.deal_center_stream(inbound).await;
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
        let addr = format!("{}:{}", self.option.bind_addr, self.option.bind_port)
            .parse::<SocketAddr>()
            .map_err(|_| ProxyError::Extension("parse addr error"))?;
        let accept = self.option.get_tls_accept().await.ok();
        let client = self.option.get_tls_request().await.ok();
        if let Some(server) = self.option.server.clone() {
            let mut center_client = CenterClient::new(server, client.clone(), self.option.domain.clone());
            match center_client.connect().await {
                Ok(true) => (),
                Ok(false) => {
                    println!("未能正确连上服务端:{:?}", self.option.server.unwrap());
                    process::exit(1);
                },
                Err(err) => {
                    println!("未能正确连上服务端:{:?}, 发生错误:{:?}", self.option.server.unwrap(), err);
                    process::exit(1);
                },
            }
            center_client.serve().await;
            self.center_client = Some(center_client);
        }
        let listener = TcpListener::bind(addr).await?;
        println!(
            "accept = {:?} client = {:?}",
            accept.is_some(),
            client.is_some()
        );
        while let Ok((inbound, _)) = listener.accept().await {
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
        Ok(())
    }

    async fn transfer_center_server<T>(
        _in_sender: Option<Sender<ProtFrame>>,
        _out_receiver: Option<Receiver<ProtFrame>>,
        _inbound: T,
    ) -> ProxyResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        // let trans = TransStream::new(inbound, in_sender, out_receiver);
        // if tls_client.is_some() {
        //     println!("connect by tls");
        //     let connector = TlsConnector::from(tls_client.unwrap());
        //     let stream = TcpStream::connect(&server).await?;
        //     // 这里的域名只为认证设置
        //     let domain =
        //         rustls::ServerName::try_from(&*domain.unwrap_or("soft.wm-proxy.com".to_string()))
        //             .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname"))?;

        //     if let Ok(mut outbound) = connector.connect(domain, stream).await {
        //         // connect 之后的流跟正常内容一样读写, 在内部实现了自动加解密
        //         let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await?;
        //     } else {
        //         // TODO 返回对应协议的错误
        //         // let _ = Self::deal_proxy(inbound, flag, None, None, None).await;
        //     }
        // } else {
        //     println!("connect by normal");
        //     if let Ok(mut outbound) = TcpStream::connect(server).await {
        //         let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await?;
        //     } else {
        //         // TODO 返回对应协议的错误
        //         // let _ = Self::deal_proxy(inbound, flag, None, None, None).await;
        //     }
        // }
        Ok(())
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
                // let _ = Self::deal_proxy(inbound, flag, None, None, None).await;
            }
        } else {
            println!("connect by normal");
            if let Ok(mut outbound) = TcpStream::connect(server).await {
                let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await?;
            } else {
                // TODO 返回对应协议的错误
                // let _ = Self::deal_proxy(inbound, flag, None, None, None).await;
            }
        }
        Ok(())
    }

    async fn deal_proxy<T>(
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
}
