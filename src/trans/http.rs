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
// Created Date: 2023/09/27 11:26:50

use std::{fmt::Debug, io::Read, net::SocketAddr, sync::Arc};

use async_trait::async_trait;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite},
    sync::{
        mpsc::{channel, Receiver, Sender},
        RwLock,
    },
};
use webparse::{Request, Response};
use wenmeng::{Body, Client, HttpTrait, ProtResult, RecvRequest, RecvResponse, Server};

use crate::{Helper, MappingConfig, ProtCreate, ProtFrame, ProxyError, VirtualStream};

static TIP_NOT_FOUND: &'static str = "当前连接未检测到与之匹配的域名，请检查配置是否正确，或者查看官方网站<a href=\"https://github.com/tickbh/wmproxy\"/>wmproxy</a>。";
struct Operate {
    oper: HttpOper,
}

#[async_trait]
impl HttpTrait for Operate {
    async fn operate(&mut self, req: &mut RecvRequest) -> ProtResult<RecvResponse> {
        let mut value = TransHttp::inner_operate(req, &mut self.oper).await?;
        value.headers_mut().insert("server", "wmproxy");
        Ok(value)
    }
}

pub struct TransHttp {
    sender: Sender<ProtFrame>,
    sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
    sock_map: u32,
    mappings: Arc<RwLock<Vec<MappingConfig>>>,
}

struct HttpOper {
    pub receiver: Receiver<ProtResult<Response<Body>>>,
    pub sender: Sender<Request<Body>>,
    pub virtual_sender: Option<Sender<ProtFrame>>,
    pub sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
    pub sock_map: u32,
    pub mappings: Arc<RwLock<Vec<MappingConfig>>>,
    pub http_map: Option<MappingConfig>,
}

impl TransHttp {
    pub fn new(
        sender: Sender<ProtFrame>,
        sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
        sock_map: u32,
        mappings: Arc<RwLock<Vec<MappingConfig>>>,
    ) -> Self {
        Self {
            sender,
            sender_work,
            sock_map,
            mappings,
        }
    }

    fn not_found_response() -> ProtResult<RecvResponse> {
        return Ok(Response::builder()
            .status(200)
            .header("Content-Type", "text/html; charset=utf-8")
            .body(TIP_NOT_FOUND)
            .ok()
            .unwrap()
            .into_type());
    }

    async fn inner_operate(
        req: &mut Request<Body>,
        oper: &mut HttpOper,
    ) -> ProtResult<Response<Body>> {
        
        let sender = oper.virtual_sender.take();
        // 传在该参数则为第一次, 第一次的时候发送Create创建绑定连接
        if sender.is_some() {
            let host_name = req.get_host().unwrap_or(String::new());
            // 取得相关的host数据，对内网的映射端做匹配，如果未匹配到返回错误，表示不支持
            {
                let mut config = None;
                let mut is_find = false;
                {
                    let read = oper.mappings.read().await;
                    for v in &*read {
                        if v.domain == host_name {
                            is_find = true;
                            config = Some(v.clone());
                        }
                    }
                }
                if !is_find {
                    return Self::not_found_response();
                }
                oper.http_map = config;
            }

            let create =
                ProtCreate::new(oper.sock_map, Some(req.get_host().unwrap_or(String::new())));
            let _ = oper.sender_work.send((create, sender.unwrap())).await;
        }

        if let Some(config) = &oper.http_map {
            // 复写Request的头文件信息
            Helper::rewrite_request(req, &config.headers);
            
            // 将请求发送出去
            oper.sender.send(req.replace_clone(Body::empty())).await?;
            // 等待返回数据的到来
            let res = oper.receiver.recv().await;
            if res.is_some() && res.as_ref().unwrap().is_ok() {
                let mut res = res.unwrap().unwrap();
                if let Some(config) = &oper.http_map {
                    // 复写Response的头文件信息
                    Helper::rewrite_response(&mut res, &config.headers);
                }
                return Ok(res);
            }
        }
        return Self::not_found_response();

    }

    pub async fn process<T>(self, inbound: T, addr: SocketAddr) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin + Debug,
    {
        log::trace!("内网穿透处理HTTP {:?}", addr);
        let build = Client::builder();
        let (virtual_sender, virtual_receiver) = channel::<ProtFrame>(10);
        let stream = VirtualStream::new(self.sock_map, self.sender.clone(), virtual_receiver);
        let mut client = Client::new(build.value(), wenmeng::MaybeHttpsStream::Http(stream));
        let (receiver, sender) = client.split().unwrap();
        let oper = HttpOper {
            receiver,
            sender,
            sender_work: self.sender_work.clone(),
            virtual_sender: Some(virtual_sender),
            sock_map: self.sock_map,
            mappings: self.mappings.clone(),
            http_map: None,
        };
        let mut server = Server::new(inbound, Some(addr));
        tokio::spawn(async move {
            let _ = client.wait_operate().await;
        });

        server.set_callback_http(Box::new(Operate { oper }));
        if let Err(e) = server.incoming().await {
            log::info!("处理内网穿透时发生错误：{:?}", e);
        };
        Ok(())
    }
}
