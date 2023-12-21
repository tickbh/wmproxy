// Copyright 2022 - 2023 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Author: tickbh
// -----
// Created Date: 2023/09/15 11:37:09

use std::{
    io::{self},
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use futures::{future::select_all, FutureExt, StreamExt};

use rustls::ClientConfig;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex,
    },
};
use tokio_rustls::{rustls, TlsAcceptor, TlsConnector};
use webparse::BinaryMut;

use crate::{
    error::ProxyTypeResult,
    option::ConfigOption,
    proxy::ProxyServer,
    reverse::{HttpConfig, ServerConfig, StreamConfig, StreamUdp},
    ActiveHealth, CenterClient, CenterServer, Flag, HealthCheck, OneHealth, ProxyError, ProxyHttp,
    ProxyResult, ProxySocks5,
};

pub struct WMCore {
    pub option: ConfigOption,
    pub center_client: Option<CenterClient>,
    pub center_servers: Vec<CenterServer>,
    health_sender: Option<Sender<Vec<OneHealth>>>,
    pub proxy_accept: Option<TlsAcceptor>,
    pub proxy_client: Option<Arc<ClientConfig>>,
    pub center_listener: Option<TcpListener>,

    pub map_http_listener: Option<TcpListener>,
    pub map_https_listener: Option<TcpListener>,
    pub map_tcp_listener: Option<TcpListener>,
    pub map_proxy_listener: Option<TcpListener>,
    pub map_accept: Option<TlsAcceptor>,

    pub http_servers: Vec<Arc<ServerConfig>>,
    pub http_accept: Option<TlsAcceptor>,
    pub http_tlss: Vec<bool>,
    pub http_listeners: Vec<TcpListener>,

    pub stream_config: Option<Arc<Mutex<StreamConfig>>>,
    pub stream_listeners: Vec<TcpListener>,
    pub stream_udp_listeners: Vec<StreamUdp>,
}

impl WMCore {
    pub fn new(option: ConfigOption) -> WMCore {
        Self {
            option,
            center_client: None,
            center_servers: vec![],
            health_sender: None,
            proxy_accept: None,
            proxy_client: None,
            center_listener: None,

            map_http_listener: None,
            map_https_listener: None,
            map_tcp_listener: None,
            map_proxy_listener: None,
            map_accept: None,

            http_servers: vec![],
            http_accept: None,
            http_tlss: vec![],
            http_listeners: vec![],

            stream_config: None,
            stream_listeners: vec![],
            stream_udp_listeners: vec![],
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
                let proxy_server = ProxyServer::new(
                    flag,
                    option.username.clone(),
                    option.password.clone(),
                    option.udp_bind.clone(),
                    None,
                );
                tokio::spawn(async move {
                    // tcp的连接被移动到该协程中，我们只要专注的处理该stream即可
                    let _ = proxy_server.deal_proxy(inbound).await;
                });
            }
        }

        Ok(())
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
                select_all(listens.iter_mut().map(|listener| listener.next().boxed())).await;
            if data.is_none() {
                return (
                    Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "read none data",
                    )),
                    index,
                );
            }
            (data.unwrap(), index)
        } else {
            let pend = std::future::pending();
            let () = pend.await;
            unreachable!()
        }
    }

    pub async fn do_start_health_check(&mut self) -> ProxyResult<()> {
        let healths = self.option.get_health_check();
        let (sender, receiver) = channel::<Vec<OneHealth>>(1);
        let _active = ActiveHealth::new(healths, receiver);
        // active.do_start()?;
        self.health_sender = Some(sender);
        Ok(())
    }

    pub async fn ready_serve(&mut self) -> ProxyResult<()> {
        if let Some(option) = &mut self.option.proxy {
            (
                self.proxy_accept,
                self.proxy_client,
                self.center_listener,
                self.center_client,
            ) = option.bind().await?;
        }

        if let Some(option) = &mut self.option.proxy {
            (
                self.map_http_listener,
                self.map_https_listener,
                self.map_tcp_listener,
                self.map_proxy_listener,
                self.map_accept,
            ) = option.bind_map().await?;
        }

        self.http_servers = self
            .option
            .http
            .clone()
            .unwrap_or(HttpConfig::new())
            .convert_server_config();

        self.stream_config = Some(Arc::new(Mutex::new(
            self.option.stream.clone().unwrap_or(StreamConfig::new()),
        )));

        if let Some(http) = &mut self.option.http {
            (self.http_accept, self.http_tlss, self.http_listeners) = http.bind().await?;
        }

        if let Some(stream) = &mut self.option.stream {
            (self.stream_listeners, self.stream_udp_listeners) = stream.bind().await?;
        }
        Ok(())
    }

    pub async fn run_serve(
        &mut self,
        mut receiver_close: Receiver<()>,
        mut sender_close: Option<Sender<()>>,
    ) -> ProxyResult<()> {
        if let Some(sender) = sender_close.take() {
            let _ = sender.send(()).await;
        }
        self.do_start_health_check().await?;

        loop {
            tokio::select! {
                Some((inbound, addr)) = Self::tcp_listen_work(&self.center_listener) => {
                    log::trace!("代理收到客户端连接: {}->{}", addr, self.center_listener.as_ref().unwrap().local_addr()?);
                    if let Some(a) = self.proxy_accept.clone() {
                        let inbound = a.accept(inbound).await;
                        // 获取的流跟正常内容一样读写, 在内部实现了自动加解密
                        match inbound {
                            Ok(inbound) => {
                                let _ = self.deal_stream(inbound, addr, self.proxy_client.clone()).await;
                            }
                            Err(e) => {
                                log::warn!("接收来自下级代理的连接失败, 原因为: {:?}", e);
                            }
                        }
                    } else {
                        let _ = self.deal_stream(inbound, addr, self.proxy_client.clone()).await;
                    };
                }
                Some((inbound, addr)) = Self::tcp_listen_work(&self.map_http_listener) => {
                    log::trace!("内网穿透:Http收到客户端连接: {}->{}", addr, self.map_http_listener.as_ref().unwrap().local_addr()?);
                    self.server_new_http(inbound, addr).await?;
                }
                Some((inbound, addr)) = Self::tcp_listen_work(&self.map_https_listener) => {
                    log::trace!("内网穿透:Https收到客户端连接: {}->{}", addr, self.map_https_listener.as_ref().unwrap().local_addr()?);
                    self.server_new_https(inbound, addr, self.map_accept.clone().unwrap()).await?;
                }
                Some((inbound, addr)) = Self::tcp_listen_work(&self.map_tcp_listener) => {
                    log::trace!("内网穿透:Tcp收到客户端连接: {}->{}", addr, self.map_tcp_listener.as_ref().unwrap().local_addr()?);
                    self.server_new_tcp(inbound, addr).await?;
                }
                Some((inbound, addr)) = Self::tcp_listen_work(&self.map_proxy_listener) => {
                    log::trace!("内网穿透:Proxy收到客户端连接: {}->{}", addr, self.map_proxy_listener.as_ref().unwrap().local_addr()?);
                    self.server_new_proxy(inbound, addr).await?;
                }
                (result, index) = Self::multi_tcp_listen_work(&mut self.http_listeners) => {
                    if let Ok((conn, addr)) = result {
                        let local_port = self.http_listeners[index].local_addr()?.port();
                        log::trace!("反向代理:{}收到客户端连接: {}->{}", if self.http_tlss[index] { "https" } else { "http" }, addr,self.http_listeners[index].local_addr()?);
                        let mut local_servers = vec![];
                        for s in &self.http_servers {
                            if (*s).bind_addr.port() != local_port {
                                continue;
                            }
                            local_servers.push(s.clone());
                        }
                        if self.http_tlss[index] {
                            let tls_accept = self.http_accept.clone().unwrap();
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
                (result, index) = Self::multi_tcp_listen_work(&mut self.stream_listeners) => {
                    if let Ok((conn, addr)) = result {
                        log::trace!("反向代理:{}收到客户端连接: {}->{}", "stream", addr, self.stream_listeners[index].local_addr()?);
                        let data = self.stream_config.clone();
                        let local_addr = self.stream_listeners[index].local_addr()?;
                        tokio::spawn(async move {
                            let _ = StreamConfig::process(data.unwrap(), local_addr, conn, addr).await;
                        });
                    }
                }
                (result, index) = Self::multi_udp_listen_work(&mut self.stream_udp_listeners) => {
                    if let Ok((data, addr)) = result {
                        log::trace!("反向代理:{}收到客户端连接: {}->{}", "stream", addr, self.stream_udp_listeners[index].local_addr()?);

                        let udp = &mut self.stream_udp_listeners[index];
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
        Ok(())
    }

    pub async fn start_serve(
        &mut self,
        receiver_close: Receiver<()>,
        sender_close: Option<Sender<()>>,
    ) -> ProxyResult<()> {
        log::trace!("开始启动服务器，正在加载配置中");
        self.ready_serve().await?;
        self.run_serve(receiver_close, sender_close).await?;
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

    pub async fn server_new_proxy(
        &mut self,
        stream: TcpStream,
        _addr: SocketAddr,
    ) -> ProxyResult<()> {
        for server in &mut self.center_servers {
            if !server.is_close() {
                return server.server_new_prxoy(stream).await;
            }
        }
        log::warn!("未发现任何tcp服务器，但收到tcp的内网穿透，请检查配置");
        Ok(())
    }
}
