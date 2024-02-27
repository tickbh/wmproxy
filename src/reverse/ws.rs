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
// Created Date: 2023/10/18 02:32:23

use std::sync::Arc;

use async_trait::async_trait;

use tokio::sync::mpsc::{channel, Receiver, Sender};

use webparse::ws::{CloseData, OwnedMessage};
use wenmeng::{
    ws::{WsHandshake, WsOption, WsTrait},
    Client, ProtError, ProtResult,
};

use super::{ReverseHelper, ServerConfig};

pub struct ServerWsOperate {
    inner: InnerWsOper,
    sender: Option<Sender<OwnedMessage>>,
}

#[async_trait]
impl WsTrait for ServerWsOperate {
    /// 握手完成后之后的回调,服务端返回了Response之后就认为握手成功
    async fn on_open(&mut self, shake: WsHandshake) -> ProtResult<Option<WsOption>> {
        if shake.request.is_none() {
            return Err(ProtError::Extension("miss request"));
        }
        let mut option = WsOption::new();
        if let Some(location) =
            ReverseHelper::get_location_by_req(&self.inner.servers, shake.request.as_ref().unwrap())
        {
            if !location.is_ws {
                return Err(ProtError::Extension("Not Support Ws"));
            }
            if let Ok((url, domain)) = location.get_reverse_url() {
                let mut client = Client::builder()
                    .url(url)?
                    .connect_with_domain(&domain)
                    .await?;

                let (serv_sender, serv_receiver) = channel::<OwnedMessage>(10);
                let (cli_sender, cli_receiver) = channel::<OwnedMessage>(10);
                option.set_receiver(serv_receiver);
                self.sender = Some(cli_sender);

                client.set_callback_ws(Box::new(ClientWsOperate {
                    sender: Some(serv_sender),
                    receiver: Some(cli_receiver),
                }));

                tokio::spawn(async move {
                    if let Err(e) = client
                        .wait_ws_operate_with_req(shake.request.unwrap())
                        .await
                    {
                        println!("error = {:?}", e);
                    };
                });
            }
            return Ok(Some(option));
        }
        return Err(ProtError::Extension("miss match"));
    }

    /// 接受到远端的关闭消息
    async fn on_close(&mut self, reason: &Option<CloseData>) {
        if let Some(s) = &self.sender {
            let _ = s.send(OwnedMessage::Close(reason.clone())).await;
        }
    }

    /// 收到来在远端的ping消息, 默认返回pong消息
    async fn on_ping(&mut self, val: Vec<u8>) -> ProtResult<Option<OwnedMessage>> {
        if let Some(s) = &self.sender {
            s.send(OwnedMessage::Ping(val.clone())).await?;
        }
        return Ok(None);
    }

    /// 收到来在远端的pong消息, 默认不做任何处理, 可自定义处理如ttl等
    async fn on_pong(&mut self, val: Vec<u8>) -> ProtResult<()> {
        if let Some(s) = &self.sender {
            let _ = s.send(OwnedMessage::Pong(val)).await?;
        }
        Ok(())
    }

    /// 收到来在远端的message消息, 必须覆写该函数
    async fn on_message(&mut self, msg: OwnedMessage) -> ProtResult<()> {
        if let Some(s) = &self.sender {
            s.send(msg).await?;
        }
        Ok(())
    }
}

struct InnerWsOper {
    pub servers: Vec<Arc<ServerConfig>>,
}

impl InnerWsOper {
    pub fn new(http: Vec<Arc<ServerConfig>>) -> Self {
        Self { servers: http }
    }
}

impl ServerWsOperate {
    pub fn new(http: Vec<Arc<ServerConfig>>) -> Self {
        Self {
            inner: InnerWsOper::new(http),
            sender: None,
        }
    }
}

pub struct ClientWsOperate {
    sender: Option<Sender<OwnedMessage>>,
    receiver: Option<Receiver<OwnedMessage>>,
}

#[async_trait]
impl WsTrait for ClientWsOperate {
    /// 握手完成后之后的回调,服务端返回了Response之后就认为握手成功
    async fn on_open(&mut self, _shake: WsHandshake) -> ProtResult<Option<WsOption>> {
        let mut option = WsOption::new();
        option.receiver = self.receiver.take();
        Ok(Some(option))
    }

    /// 接受到远端的关闭消息
    async fn on_close(&mut self, reason: &Option<CloseData>) {
        if let Some(s) = &self.sender {
            let _ = s.send(OwnedMessage::Close(reason.clone())).await;
        }
    }

    /// 收到来在远端的ping消息, 默认返回pong消息
    async fn on_ping(&mut self, val: Vec<u8>) -> ProtResult<Option<OwnedMessage>> {
        if let Some(s) = &self.sender {
            s.send(OwnedMessage::Ping(val)).await?;
        }
        return Ok(None);
    }

    /// 收到来在远端的pong消息, 默认不做任何处理, 可自定义处理如ttl等
    async fn on_pong(&mut self, val: Vec<u8>) -> ProtResult<()> {
        if let Some(s) = &self.sender {
            let _ = s.send(OwnedMessage::Pong(val)).await?;
        }
        Ok(())
    }

    /// 收到来在远端的message消息, 必须覆写该函数
    async fn on_message(&mut self, msg: OwnedMessage) -> ProtResult<()> {
        if let Some(s) = &self.sender {
            s.send(msg).await?;
        }
        Ok(())
    }
}
