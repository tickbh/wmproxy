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
// Created Date: 2023/09/25 10:08:56

use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{
        mpsc::{channel, Receiver},
        RwLock,
    },
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc::Sender,
};

use tokio_rustls::TlsAcceptor;
use webparse::BinaryMut;
use webparse::Buf;

use crate::{
    prot::{ProtClose, ProtFrame},
    proxy::ProxyServer,
    trans::{TransHttp, TransTcp},
    Helper, MappingConfig, ProtCreate, ProxyConfig, ProxyResult, VirtualStream, WMCore,
};

/// 中心服务端
/// 接受中心客户端的连接，并且将信息处理或者转发
pub struct CenterServer {
    /// 代理的详情信息，如用户密码这类
    option: ProxyConfig,

    /// 发送协议数据，接收到服务端的流数据，转发给相应的Stream
    sender: Sender<ProtFrame>,
    /// 接收协议数据，并转发到服务端。
    receiver: Option<Receiver<ProtFrame>>,

    /// 发送Create，并将绑定的Sender发到做绑定
    sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
    /// 接收的Sender绑定，开始服务时这值move到工作协程中，所以不能二次调用服务
    receiver_work: Option<Receiver<(ProtCreate, Sender<ProtFrame>)>>,
    /// 绑定的下一个sock_map映射，为双数
    next_id: u32,
    /// 内网映射的相关消息, 需要读写分离需加锁
    mappings: Arc<RwLock<Vec<MappingConfig>>>,
}

impl CenterServer {
    pub fn new(option: ProxyConfig) -> Self {
        let (sender, receiver) = channel::<ProtFrame>(100);
        let (sender_work, receiver_work) = channel::<(ProtCreate, Sender<ProtFrame>)>(10);
        Self {
            option,
            sender,
            receiver: Some(receiver),
            sender_work,
            receiver_work: Some(receiver_work),
            next_id: 2,
            mappings: Arc::new(RwLock::new(vec![])),
        }
    }

    pub fn sender(&self) -> Sender<ProtFrame> {
        self.sender.clone()
    }

    pub fn sender_work(&self) -> Sender<(ProtCreate, Sender<ProtFrame>)> {
        self.sender_work.clone()
    }

    pub fn is_close(&self) -> bool {
        self.sender.is_closed()
    }

    pub fn calc_next_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 2;
        id
    }

    pub async fn inner_serve<T>(
        stream: T,
        option: ProxyConfig,
        sender: Sender<ProtFrame>,
        mut receiver: Receiver<ProtFrame>,
        mut receiver_work: Receiver<(ProtCreate, Sender<ProtFrame>)>,
        mappings: Arc<RwLock<Vec<MappingConfig>>>,
    ) -> ProxyResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut map = HashMap::<u32, Sender<ProtFrame>>::new();
        let mut read_buf = BinaryMut::new();
        let mut write_buf = BinaryMut::new();
        let mut verify_succ = option.username.is_none() && option.password.is_none();

        let (mut reader, mut writer) = split(stream);
        let mut vec = Vec::with_capacity(4096);
        vec.resize(4096, 0);
        let is_closed;
        let mut is_ready_shutdown = false;
        loop {
            let _ = tokio::select! {
                // 严格的顺序流
                biased;
                // 新的流建立，这里接收Create并进行绑定
                r = receiver_work.recv() => {
                    if let Some((create, sender)) = r {
                        map.insert(create.sock_map(), sender);
                        let _ = create.encode(&mut write_buf);
                    }
                }
                // 数据的接收，并将数据写入给远程端
                r = receiver.recv() => {
                    if let Some(p) = r {
                        let _ = p.encode(&mut write_buf);
                    }
                }
                // 数据的等待读取，一旦流可读则触发，读到0则关闭主动关闭所有连接
                r = reader.read(&mut vec) => {
                    match r {
                        Ok(0)=>{
                            is_closed=true;
                            break;
                        }
                        Ok(n) => {
                            read_buf.put_slice(&vec[..n]);
                        }
                        Err(_) => {
                            is_closed = true;
                            break;
                        },
                    }
                }
                // 一旦有写数据，则尝试写入数据，写入成功后扣除相应的数据
                r = writer.write(write_buf.chunk()), if write_buf.has_remaining() => {
                    match r {
                        Ok(n) => {
                            write_buf.advance(n);
                            if !write_buf.has_remaining() {
                                write_buf.clear();

                                if is_ready_shutdown {
                                    return Ok(())
                                }
                            }
                        }
                        Err(_) => todo!(),
                    }
                }
            };
            if is_ready_shutdown {
                continue;
            }
            loop {
                // 将读出来的数据全部解析成ProtFrame并进行相应的处理，如果是0则是自身消息，其它进行转发
                match Helper::decode_frame(&mut read_buf)? {
                    Some(p) => {
                        match &p {
                            ProtFrame::Token(p) => {
                                if !verify_succ
                                    && p.is_check_succ(&option.username, &option.password)
                                {
                                    verify_succ = true;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                        if !verify_succ {
                            ProtFrame::new_close_reason(0, "not verify so close".to_string())
                                .encode(&mut write_buf)?;
                            is_ready_shutdown = true;
                            break;
                        }
                        match p {
                            ProtFrame::Create(p) => {
                                let (virtual_sender, virtual_receiver) = channel::<ProtFrame>(10);
                                map.insert(p.sock_map(), virtual_sender);
                                let stream = VirtualStream::new(
                                    p.sock_map(),
                                    sender.clone(),
                                    virtual_receiver,
                                );

                                let proxy_server = ProxyServer::new(
                                    option.flag,
                                    option.username.clone(),
                                    option.password.clone(),
                                    option.udp_bind.clone(),
                                    None,
                                );
                                tokio::spawn(async move {
                                    // 处理代理的能力
                                    let _ = proxy_server.deal_proxy(stream).await;
                                });
                            }
                            ProtFrame::Close(_) | ProtFrame::Data(_) => {
                                if let Some(sender) = map.get(&p.sock_map()) {
                                    let _ = sender.send(p).await;
                                }
                            }
                            ProtFrame::Mapping(p) => {
                                let mut guard = mappings.write().await;
                                *guard = p.into_mappings();
                            }
                            ProtFrame::Token(_) => unreachable!(),
                        }
                    }
                    None => {
                        break;
                    }
                }
            }
            if !read_buf.has_remaining() {
                read_buf.clear();
            }
        }
        if is_closed {
            for v in map {
                let _ = v.1.try_send(ProtFrame::Close(ProtClose::new(v.0)));
            }
        }
        Ok(())
    }

    pub async fn serve<T>(&mut self, stream: T) -> ProxyResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        if self.receiver.is_none() || self.receiver_work.is_none() {
            log::warn!("接收器为空，请检查是否出错");
            return Ok(());
        }
        let option = self.option.clone();
        let sender = self.sender.clone();
        let receiver = self.receiver.take().unwrap();
        let receiver_work = self.receiver_work.take().unwrap();
        let mapping = self.mappings.clone();
        tokio::spawn(async move {
            let _ =
                Self::inner_serve(stream, option, sender, receiver, receiver_work, mapping).await;
        });
        Ok(())
    }

    pub async fn server_new_http(
        &mut self,
        stream: TcpStream,
        addr: SocketAddr,
    ) -> ProxyResult<()> {
        let trans = TransHttp::new(
            self.sender(),
            self.sender_work(),
            self.calc_next_id(),
            self.mappings.clone(),
        );
        tokio::spawn(async move {
            if let Err(e) = trans.process(stream, addr).await {
                log::warn!("内网穿透:Http转发时发生错误:{:?}", e);
            }
        });
        return Ok(());
    }

    pub async fn server_new_https(
        &mut self,
        stream: TcpStream,
        addr: SocketAddr,
        accept: TlsAcceptor,
    ) -> ProxyResult<()> {
        let trans = TransHttp::new(
            self.sender(),
            self.sender_work(),
            self.calc_next_id(),
            self.mappings.clone(),
        );
        tokio::spawn(async move {
            match accept.accept(stream).await {
                Ok(tls_stream) => {
                    if let Err(e) = trans.process(tls_stream, addr).await {
                        log::warn!("内网穿透:修理Https转发时发生错误:{:?}", e);
                    }
                }
                Err(e) => {
                    log::warn!("内网穿透:Https握手时发生错误:{:?}", e);
                }
            }
        });
        return Ok(());
    }

    pub async fn server_new_tcp(&mut self, stream: TcpStream) -> ProxyResult<()> {
        let trans = TransTcp::new(
            self.sender(),
            self.sender_work(),
            self.calc_next_id(),
            self.mappings.clone(),
        );
        tokio::spawn(async move {
            if let Err(e) = trans.process(stream, "tcp").await {
                log::warn!("内网穿透:修理Tcp转发时发生错误:{:?}", e);
            }
        });
        return Ok(());
    }

    pub async fn server_new_prxoy(&mut self, stream: TcpStream) -> ProxyResult<()> {
        let trans = TransTcp::new(
            self.sender(),
            self.sender_work(),
            self.calc_next_id(),
            self.mappings.clone(),
        );
        tokio::spawn(async move {
            if let Err(e) = trans.process(stream, "proxy").await {
                log::warn!("内网穿透:修理Tcp转发时发生错误:{:?}", e);
            }
        });
        return Ok(());
    }
}
