use std::{
    io::{self},
    net::{IpAddr, SocketAddr},
    process,
    sync::Arc,
};

use futures::{future::select_all, FutureExt, StreamExt};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream}, sync::{mpsc::{channel, Sender, Receiver}, Mutex},
};
use tokio_rustls::{rustls, TlsAcceptor, TlsConnector};
use webparse::BinaryMut;

use crate::{
    error::ProxyTypeResult, option::ConfigOption, reverse::{HttpConfig, StreamConfig, StreamUdp}, CenterClient, CenterServer,
    Flag, HealthCheck, ProxyError, ProxyHttp, ProxyResult, ProxySocks5, OneHealth, ActiveHealth, Helper,
};

pub struct Proxy {
    option: ConfigOption,
    center_client: Option<CenterClient>,
    center_servers: Vec<CenterServer>,

    health_sender: Option<Sender<Vec<OneHealth>>>,
}

impl Proxy {
    pub fn new(option: ConfigOption) -> Proxy {
        Self {
            option,
            center_client: None,
            center_servers: vec![],
            health_sender: None,
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
        _addr: SocketAddr,
        tls_client: Option<Arc<rustls::ClientConfig>>,
    ) -> ProxyResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    {
        // 转发到服务端
        if let Some(client) = &mut self.center_client {
            return client.deal_new_stream(inbound).await;
        }
        if let Some(option) = &mut self.option.proxy {
            // 服务端开代理, 接收到客户端一律用协议处理
            if option.center && option.is_server() {
                let server = CenterServer::new(option.clone());
                self.center_servers.push(server);
                return self.center_servers.last_mut().unwrap().serve(inbound).await;
            }

            let flag = option.flag;
            let domain = option.domain.clone();
            if let Some(server) = option.server.clone() {
                tokio::spawn(async move {
                    // 转到上层服务器进行处理
                    let _e = Self::transfer_server(domain, tls_client, inbound, server).await;
                });
            } else {
                let username = option.username.clone();
                let password = option.password.clone();
                let udp_bind = option.udp_bind.clone();
                tokio::spawn(async move {
                    // tcp的连接被移动到该协程中，我们只要专注的处理该stream即可
                    let _ = Self::deal_proxy(inbound, flag, username, password, udp_bind).await;
                });
            }
        }

        Ok(())
    }

    pub async fn do_start_health_check(&mut self) -> ProxyResult<()> {
        let healths = self.option.get_health_check();
        let (sender, receiver) = channel::<Vec<OneHealth>>(1);
        let _active = ActiveHealth::new(healths, receiver);
        // active.do_start()?;
        self.health_sender = Some(sender);
        Ok(())
    }

    pub async fn start_serve(&mut self, mut receiver_close: Receiver<()>, mut sender_close: Option<Sender<()>>) -> ProxyResult<()> {
        log::trace!("开始启动服务器，正在加载配置中");
        let mut proxy_accept = None;
        let mut client = None;
        let mut center_listener = None;
        if let Some(option) = &mut self.option.proxy {
            let addr = option.bind_addr.clone();
            proxy_accept = option.get_tls_accept().await.ok();
            client = option.get_tls_request().await.ok();
            if option.center {
                if let Some(server) = option.server.clone() {
                    let mut center_client = CenterClient::new(
                        option.clone(),
                        server,
                        client.clone(),
                        option.domain.clone(),
                        option.mappings.clone(),
                    );
                    match center_client.connect().await {
                        Ok(true) => (),
                        Ok(false) => {
                            log::error!("未能正确连上服务端:{:?}", option.server.unwrap());
                            process::exit(1);
                        }
                        Err(err) => {
                            log::error!(
                                "未能正确连上服务端:{:?}, 发生错误:{:?}",
                                option.server.unwrap(),
                                err
                            );
                            process::exit(1);
                        }
                    }
                    let _ = center_client.serve().await;
                    self.center_client = Some(center_client);
                }
            }
            center_listener = Some(Helper::bind(addr).await?);
        }
        async fn tcp_listen_work(listen: &Option<TcpListener>) -> Option<(TcpStream, SocketAddr)> {
            if listen.is_some() {
                match listen.as_ref().unwrap().accept().await {
                    Ok((tcp, addr)) => Some((tcp, addr)),
                    Err(_e) => None,
                }
            } else {
                let pend = std::future::pending();
                let () = pend.await;
                None
            }
        }

        async fn multi_tcp_listen_work(
            listens: &mut Vec<TcpListener>,
        ) -> (io::Result<(TcpStream, SocketAddr)>, usize) {
            if !listens.is_empty() {
                let (conn, index, _) =
                    select_all(listens.iter_mut().map(|listener| listener.accept().boxed())).await;
                (conn, index)
            } else {
                let pend = std::future::pending();
                let () = pend.await;
                unreachable!()
            }
        }

        async fn multi_udp_listen_work(
            listens: &mut Vec<StreamUdp>,
        ) -> (io::Result<(Vec<u8>, SocketAddr)>, usize) {
            if !listens.is_empty() {
                let (data, index, _) =
                    select_all(listens.iter_mut().map(|listener| {
                        listener.next().boxed()
                    })).await;
                if data.is_none() {
                    return (Err(io::Error::new(io::ErrorKind::InvalidInput, "read none data")), index)
                }
                (data.unwrap(), index)
            } else {
                let pend = std::future::pending();
                let () = pend.await;
                unreachable!()
            }
        }

        let (accept, tlss, mut listeners) = if let Some(http) = &mut self.option.http {
            http.bind().await?
        } else {
            (None, vec![], vec![])
        };

        
        let (mut stream_listeners, mut stream_udp_listeners) = if let Some(stream) = &mut self.option.stream {
            stream.bind().await?
        } else {
            (vec![], vec![])
        };

        let servers = self.option.http.clone().unwrap_or(HttpConfig::new()).convert_server_config();
        let stream = Arc::new(Mutex::new(self.option.stream.clone().unwrap_or(StreamConfig::new())));
        let mut http_listener = None;
        let mut https_listener = None;
        let mut tcp_listener = None;
        let mut map_accept = None;
        if let Some(option) = &mut self.option.proxy {
            if let Some(ls) = &option.map_http_bind {
                http_listener = Some(Helper::bind(ls).await?);
            };
            if let Some(ls) = &option.map_https_bind {
                https_listener = Some(Helper::bind(ls).await?);
            };

            if https_listener.is_some() {
                let accept = option.get_map_tls_accept().await.ok();
                if accept.is_none() {
                    let _ = https_listener.take();
                }
                map_accept = accept;
            };

            if let Some(ls) = &option.map_tcp_bind {
                tcp_listener = Some(Helper::bind(ls).await?);
            };
        }

        if let Some(sender) = sender_close.take() {
            let _ = sender.send(()).await;
        }

        self.do_start_health_check().await?;

        loop {
            tokio::select! {
                Some((inbound, addr)) = tcp_listen_work(&center_listener) => {
                    log::trace!("代理收到客户端连接: {}->{}", addr, center_listener.as_ref().unwrap().local_addr()?);
                    if let Some(a) = proxy_accept.clone() {
                        let inbound = a.accept(inbound).await;
                        // 获取的流跟正常内容一样读写, 在内部实现了自动加解密
                        match inbound {
                            Ok(inbound) => {
                                let _ = self.deal_stream(inbound, addr, client.clone()).await;
                            }
                            Err(e) => {
                                log::warn!("接收来自下级代理的连接失败, 原因为: {:?}", e);
                            }
                        }
                    } else {
                        let _ = self.deal_stream(inbound, addr, client.clone()).await;
                    };
                }
                Some((inbound, addr)) = tcp_listen_work(&http_listener) => {
                    log::trace!("内网穿透:Http收到客户端连接: {}->{}", addr, http_listener.as_ref().unwrap().local_addr()?);
                    self.server_new_http(inbound, addr).await?;
                }
                Some((inbound, addr)) = tcp_listen_work(&https_listener) => {
                    log::trace!("内网穿透:Https收到客户端连接: {}->{}", addr, https_listener.as_ref().unwrap().local_addr()?);
                    self.server_new_https(inbound, addr, map_accept.clone().unwrap()).await?;
                }
                Some((inbound, addr)) = tcp_listen_work(&tcp_listener) => {
                    log::trace!("内网穿透:Tcp收到客户端连接: {}->{}", addr, tcp_listener.as_ref().unwrap().local_addr()?);
                    self.server_new_tcp(inbound, addr).await?;
                }
                (result, index) = multi_tcp_listen_work(&mut listeners) => {
                    if let Ok((conn, addr)) = result {
                        let local_port = listeners[index].local_addr()?.port();
                        log::trace!("反向代理:{}收到客户端连接: {}->{}", if tlss[index] { "https" } else { "http" }, addr, listeners[index].local_addr()?);
                        let mut local_servers = vec![];
                        for s in &servers {
                            if (*s).bind_addr.port() != local_port {
                                continue;
                            }
                            local_servers.push(s.clone());
                        }
                        if tlss[index] {
                            let tls_accept = accept.clone().unwrap();
                            tokio::spawn(async move {
                                if let Ok(stream) = tls_accept.accept(conn).await {
                                    let data = stream.get_ref();
                                    let server_name = data.1.server_name().clone().map(|s| s.to_string());
                                    for s in &local_servers {
                                        if server_name.is_some() && &s.server_name == server_name.as_ref().unwrap() {
                                            let _ = HttpConfig::process(vec![s.clone()], stream, addr).await;
                                            return;
                                        }
                                    }
                                    let _ = HttpConfig::process(local_servers, stream, addr).await;
                                }
                            });
                        } else {
                            let _ = HttpConfig::process(local_servers, conn, addr).await;
                        }
                    }
                }
                (result, index) = multi_tcp_listen_work(&mut stream_listeners) => {
                    if let Ok((conn, addr)) = result {
                        log::trace!("反向代理:{}收到客户端连接: {}->{}", "stream", addr, stream_listeners[index].local_addr()?);
                        let data = stream.clone();
                        let local_addr = stream_listeners[index].local_addr()?;
                        tokio::spawn(async move {
                            let _ = StreamConfig::process(data, local_addr, conn, addr).await;
                        });
                    }
                }
                (result, index) = multi_udp_listen_work(&mut stream_udp_listeners) => {
                    if let Ok((data, addr)) = result {
                        log::trace!("反向代理:{}收到客户端连接: {}->{}", "stream", addr, stream_udp_listeners[index].local_addr()?);

                        let udp = &mut stream_udp_listeners[index];
                        if let Err(e) = udp.process_data(data, addr).await {
                            log::info!("udp负载均衡的时候发生错误:{:?}", e);
                        }
                        // let data = stream.clone();
                        // let local_addr = stream_udp_listeners[index].local_addr()?;
                        // tokio::spawn(async move {
                        //     let _ = StreamConfig::process(data, local_addr, conn, addr).await;
                        // });
                    }
                }
                _ = receiver_close.recv() => {
                    log::info!("反向代理：接收到退出信号,来自配置的变更,退出当前线程");
                    return Ok(());
                }
            }
            log::trace!("处理一条连接完毕，循环继续处理下一条信息");
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
            let connector = TlsConnector::from(tls_client.unwrap());
            let stream = HealthCheck::connect(&server).await?;
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
            if let Ok(mut outbound) = HealthCheck::connect(&server).await {
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
                    log::info!("socks5代理发生错误：{:?}", err);
                    return Err(err);
                }
            };
        Ok(())
    }

    pub async fn server_new_http(
        &mut self,
        stream: TcpStream,
        addr: SocketAddr,
    ) -> ProxyResult<()> {
        for server in &mut self.center_servers {
            if !server.is_close() {
                return server.server_new_http(stream, addr).await;
            }
        }
        log::warn!("未发现任何http服务器，但收到http的内网穿透，请检查配置");
        Ok(())
    }

    pub async fn server_new_https(
        &mut self,
        stream: TcpStream,
        addr: SocketAddr,
        accept: TlsAcceptor,
    ) -> ProxyResult<()> {
        for server in &mut self.center_servers {
            if !server.is_close() {
                return server.server_new_https(stream, addr, accept).await;
            }
        }
        log::warn!("未发现任何https服务器，但收到https的内网穿透，请检查配置");
        Ok(())
    }

    pub async fn server_new_tcp(
        &mut self,
        stream: TcpStream,
        _addr: SocketAddr,
    ) -> ProxyResult<()> {
        for server in &mut self.center_servers {
            if !server.is_close() {
                return server.server_new_tcp(stream).await;
            }
        }
        log::warn!("未发现任何tcp服务器，但收到tcp的内网穿透，请检查配置");
        Ok(())
    }
}
