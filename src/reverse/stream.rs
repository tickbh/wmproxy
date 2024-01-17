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
// Created Date: 2023/10/18 02:32:27

use std::{
    collections::{HashMap, HashSet, LinkedList},
    io,
    net::SocketAddr,
    sync::Arc,
    task::{ready, Poll},
    time::{Instant, Duration},
};


use futures_core::Stream;

use serde::{Deserialize, Serialize};
use tokio::{
    io::{copy_bidirectional, AsyncRead, AsyncWrite, ReadBuf, Interest},
    net::{TcpListener, UdpSocket},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex,
    }, time::sleep,
};
use tokio_util::sync::PollSender;
use webparse::{BinaryMut, Buf, BufMut};
use wenmeng::{plugins::{StreamToWs, WsToStream}};


use crate::{HealthCheck, Helper, ProxyResult, ProxyError};

use super::{ServerConfig, UpstreamConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    #[serde(default = "Vec::new")]
    pub server: Vec<ServerConfig>,
    #[serde(default = "Vec::new")]
    pub upstream: Vec<UpstreamConfig>,
}

impl StreamConfig {
    pub fn new() -> Self {
        StreamConfig {
            server: vec![],
            upstream: vec![],
        }
    }

    /// 将配置参数提前共享给子级
    pub fn copy_to_child(&mut self) {
        for server in &mut self.server {
            server.upstream.append(&mut self.upstream.clone());
            server.copy_to_child();
        }
    }

    /// stream的绑定，按bind_mode区分出udp或者是tcp，返回相应的列表
    pub async fn bind(&mut self) -> ProxyResult<(Vec<TcpListener>, Vec<StreamUdp>)> {
        let mut listeners = vec![];
        let mut udp_listeners = vec![];
        let mut bind_port = HashSet::new();
        for value in &self.server.clone() {
            if bind_port.contains(&value.bind_addr.port()) {
                continue;
            }
            bind_port.insert(value.bind_addr.port());
            if value.bind_mode == "udp" {
                log::info!("负载均衡,stream：{:?}，提供stream中的udp转发功能。", value.bind_addr);
                let listener = Helper::bind_upd(value.bind_addr).await?;
                udp_listeners.push(StreamUdp::new(listener, value.clone()));
            } else {
                log::info!("负载均衡,stream：{:?}，提供stream中的tcp转发功能。", value.bind_addr);

                let listener = Helper::bind(value.bind_addr).await?;
                listeners.push(listener);
            }
        }

        Ok((listeners, udp_listeners))
    }

    pub async fn process<T>(
        data: Arc<Mutex<StreamConfig>>,
        local_addr: SocketAddr,
        mut inbound: T,
        _addr: SocketAddr,
    ) -> ProxyResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin + std::marker::Send + 'static,
    {
        let value = data.lock().await;
        for (_, s) in value.server.iter().enumerate() {
            if s.bind_addr.port() == local_addr.port() {
                
                let (addr, domain) = s.get_addr_domain()?;
                if addr.is_none() {
                    return Err(ProxyError::Extension("unknow addr"));
                }
                let addr = addr.unwrap();
                if s.bind_mode == "ws2tcp" {
                    let mut ws_to_stream = WsToStream::new(inbound, addr)?;
                    if domain.is_some() {
                        ws_to_stream.set_domain(domain.unwrap());
                    }
                    let _ = ws_to_stream.copy_bidirectional().await;
                } else if s.bind_mode == "tcp2ws" {
                    let mut stream_to_ws = StreamToWs::new(inbound, format!("ws://{}", addr))?;
                    if domain.is_some() {
                        stream_to_ws.set_domain(domain.unwrap());
                    }
                    let _ = stream_to_ws.copy_bidirectional().await;
                } else if s.bind_mode == "tcp2wss" {
                    let mut stream_to_ws = StreamToWs::new(inbound, format!("wss://{}", addr))?;
                    if domain.is_some() {
                        stream_to_ws.set_domain(domain.unwrap());
                    }
                    let _ = stream_to_ws.copy_bidirectional().await;
                } else {
                    let mut connect = HealthCheck::connect(&addr).await?;
                    copy_bidirectional(&mut inbound, &mut connect).await?;
                }
                break;
            }
        }
        Ok(())
    }
}

struct InnerUdp {
    pub sender: PollSender<(Vec<u8>, SocketAddr)>,
    pub last_time: Instant,
    pub timeout: Duration,
}

impl InnerUdp {
    pub fn is_timeout(&self) -> bool {
        Instant::now().duration_since(self.last_time) > self.timeout
    }
}

/// Udp转发的处理结构，缓存一些数值以做中转
pub struct StreamUdp {
    /// 读的缓冲类，避免每次都释放
    pub buf: BinaryMut,
    /// 核心的udp绑定端口
    pub socket: UdpSocket,
    pub server: ServerConfig,

    /// 如果接收该数据大小为0，那么则代表通知数据关闭
    pub receiver: Receiver<(Vec<u8>, SocketAddr)>,
    /// 将发送器传达给每个子协程
    pub sender: Sender<(Vec<u8>, SocketAddr)>,

    /// 接收的缓存数据，无法保证全部直接进行发送完毕
    pub cache_data: LinkedList<(Vec<u8>, SocketAddr)>,
    /// 发送的缓存数据，无法保证全部直接进行发送完毕
    pub send_cache_data: LinkedList<(Vec<u8>, SocketAddr)>,
    /// 每个地址绑定的对象，包含Sender，最后操作时间，超时时间
    remote_sockets: HashMap<SocketAddr, InnerUdp>,
}

impl StreamUdp {
    pub fn new(socket: UdpSocket, server: ServerConfig) -> Self {
        let (sender, receiver) = channel(10);
        Self {
            buf: BinaryMut::new(),
            socket,
            server,
            receiver,
            sender,
            cache_data: LinkedList::new(),
            send_cache_data: LinkedList::new(),
            remote_sockets: HashMap::new(),
        }
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub async fn deal_udp_bind(
        sender: &mut Sender<(Vec<u8>, SocketAddr)>,
        mut receiver: Receiver<(Vec<u8>, SocketAddr)>,
        data: Vec<u8>,
        origin_addr: SocketAddr,
        remote_addr: SocketAddr,
        timeout: Duration,
    ) -> io::Result<()> {
        let udp = match UdpSocket::bind("0.0.0.0:0").await {
            Ok(udp) => udp,
            Err(_) => {
                return Ok(());
            }
        };
        let mut cache = vec![0u8; 9096];
        let mut send_cache = LinkedList::<Vec<u8>>::new();
        send_cache.push_back(data);
        loop {
            let mut interest = Interest::READABLE;
            if !send_cache.is_empty() {
                interest = interest | Interest::WRITABLE;
            }
            tokio::select! {
                v = udp.ready(interest) => {
                    let r = v?;
                    if r.is_readable() {
                        match udp.try_recv_from(&mut cache) {
                            Ok((s, _)) => {
                                sender.send((cache[..s].to_vec(), origin_addr)).await.map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "sender close"))?;
                            },
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {},
                            Err(e) => return Err(e),
                        }
                    }
                    if r.is_writable() {
                        let value = send_cache.pop_front().unwrap();
                        match udp.send_to(&value, remote_addr).await {
                            Ok(_) => {},
                            Err(e) => {
                                return Err(e)
                            },
                        }
                    }
                }
                r = receiver.recv() => {
                    match r {
                        None => {
                            return Ok(());
                        }
                        Some(v) => {
                            send_cache.push_back(v.0);
                        }
                    }
                }
                _ = sleep(timeout) => {
                    log::trace!("UDP进程操作超时({:?})，已退出进程", timeout);
                    return Ok(());
                }
            }
        }
    }

    pub async fn process_data(&mut self, data: Vec<u8>, addr: SocketAddr) -> ProxyResult<()> {
        if self.remote_sockets.contains_key(&addr) {
            {
                let inner = self.remote_sockets.get_mut(&addr).unwrap();
                if !inner.is_timeout() {
                    inner.last_time = Instant::now();
                    self.send_cache_data.push_back((data, addr));
                    return Ok(());
                }
            }
            self.remote_sockets.remove(&addr);
        }
        let mut remote_addr = None;
        for up in &self.server.upstream {
            if up.name == self.server.up_name {
                remote_addr = up.get_server_addr();
            }
        }
        if remote_addr.is_none() {
            return Err(crate::ProxyError::Extension("当前负载地址不存在"));
        }

        let remote_addr = remote_addr.unwrap();
        let (sender, receiver) = channel(10);
        let mut timeout = Duration::new(60, 0);
        if self.server.comm.client_timeout.is_some() {
            timeout = self.server.comm.client_timeout.clone().unwrap().0;
        }
        self.remote_sockets.insert(addr, InnerUdp { sender: PollSender::new(sender), last_time: Instant::now(), timeout } );
        let mut sender_clone = self.sender.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::deal_udp_bind(&mut sender_clone, receiver, data, addr, remote_addr, timeout).await {
                log::info!("处理UDP信息发生错误，退出:{:?}", e);
            }
            let _ = sender_clone.send((vec![], addr)).await;
        });
        Ok(())
    }

    pub fn poll_read(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<io::Result<(Vec<u8>, SocketAddr)>>> {
        self.buf.clear();
        let (size, client_addr) = {
            let mut buf = ReadBuf::uninit(self.buf.chunk_mut());
            let addr = ready!(self.socket.poll_recv_from(cx, &mut buf))?;
            (buf.filled().len(), addr)
        };
        unsafe {
            self.buf.advance_mut(size);
        }
        Poll::Ready(Some(Ok((self.buf.chunk().to_vec(), client_addr))))
    }

    pub fn poll_sender(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<io::Result<()>>> {
        if self.send_cache_data.is_empty() {
            return Poll::Pending;
        }
        let mut new_cache_data = LinkedList::new();
        while !self.send_cache_data.is_empty() {
            let first = self.send_cache_data.pop_front().unwrap();
            if self.remote_sockets.contains_key(&first.1) {
                let sender = &mut self.remote_sockets.get_mut(&first.1).unwrap().sender;
                match sender.poll_reserve(cx) {
                    Poll::Ready(Ok(_)) => {
                        let _ = sender.send_item(first);
                    }
                    Poll::Ready(Err(_)) => {}
                    Poll::Pending => {
                        new_cache_data.push_back(first);
                    }
                }
            }
        }
        self.send_cache_data = new_cache_data;
        Poll::Pending
    }

    pub fn poll_write(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<io::Result<()>>> {
        loop {
            match self.receiver.poll_recv(cx) {
                Poll::Pending => break,
                Poll::Ready(None) => {
                    return Poll::Ready(None);
                }
                Poll::Ready(Some((val, addr))) => {
                    self.cache_data.push_back((val, addr));
                }
            }
        }
        loop {
            if self.cache_data.is_empty() {
                break;
            }
            let first = self.cache_data.pop_front().unwrap();
            match self.socket.poll_send_to(cx, &first.0, first.1) {
                Poll::Pending => {
                    self.cache_data.push_front((first.0, first.1));
                    return Poll::Pending;
                }
                Poll::Ready(Ok(_)) => {}
                Poll::Ready(Err(e)) => {
                    return Poll::Ready(Some(Err(e)));
                }
            };
        }

        return Poll::Pending;
    }
}

impl Stream for StreamUdp {
    type Item = io::Result<(Vec<u8>, SocketAddr)>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let _ = self.poll_write(cx)?;
        let _ = self.poll_sender(cx)?;
        self.poll_read(cx)
    }
}
