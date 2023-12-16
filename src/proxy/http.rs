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
// Created Date: 2023/09/15 03:12:20

use std::io::Cursor;

use crate::{HealthCheck, ProxyError};
use async_trait::async_trait;
use tokio::{io::{copy_bidirectional, AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf}, net::TcpStream, sync::mpsc::{Receiver, Sender}};
use webparse::{BinaryMut, BufMut, Method, Response};
use wenmeng::{OperateTrait, RecvRequest, ProtResult, RecvResponse, Server, Client, ClientOption, ProtError, MaybeHttpsStream, Body};

pub struct ProxyHttp {}

/// http代理类处理类
struct Operate {
    /// 用户名
    username: Option<String>,
    /// 密码
    password: Option<String>,
    /// Stream类, https连接后给后续https使用
    stream: Option<TcpStream>,
    /// http代理keep-alive的复用
    sender: Option<Sender<RecvRequest>>,
    /// http代理keep-alive的复用
    receiver: Option<Receiver<ProtResult<RecvResponse>>>,
}

impl Operate {
    
    pub fn check_basic_auth(&self, value: &str) -> bool
    {
        use base64::engine::general_purpose;
        use std::io::Read;

        let vals: Vec<&str> = value.split_whitespace().collect();
        if vals.len() == 1 {
            return false;
        }

        let mut wrapped_reader = Cursor::new(vals[1].as_bytes());
        let mut decoder = base64::read::DecoderReader::new(
            &mut wrapped_reader,
            &general_purpose::STANDARD);
        // handle errors as you normally would
        let mut result: Vec<u8> = Vec::new();
        decoder.read_to_end(&mut result).unwrap();

        if let Ok(value) = String::from_utf8(result) {
            let up: Vec<&str> = value.split(":").collect();
            if up.len() != 2 {
                return false;
            }
            if up[0] == self.username.as_ref().unwrap() ||
                up[1] == self.password.as_ref().unwrap() {
                return true;
            }
        }

        return false;
    }
}

#[async_trait]
impl OperateTrait for &mut Operate {
    async fn operate(&mut self, request: &mut RecvRequest) -> ProtResult<RecvResponse> {
        // 已连接直接进行后续处理
        if let Some(sender) = &self.sender {
            sender.send(request.replace_clone(Body::empty())).await?;
            if let Some(res) = self.receiver.as_mut().unwrap().recv().await {
                return Ok(res?)
            }
            return Err(ProtError::Extension("already close by other"))
        }
        // 获取要连接的对象
        let stream = if let Some(host) = request.get_connect_url() {
            match HealthCheck::connect(&host).await {
                Ok(v) => v,
                Err(e) => {
                    return Err(ProtError::from(e));
                }
            }
        } else {
            return Err(ProtError::Extension("unknow tcp stream"));
        };

        // 账号密码存在，将获取`Proxy-Authorization`进行校验，如果检验错误返回407协议
        if self.username.is_some() && self.password.is_some() {
            let mut is_auth = false;
            if let Some(auth) = request.headers_mut().remove(&"Proxy-Authorization") {
                if let Some(val) = auth.as_string() {
                    is_auth = self.check_basic_auth(&val);
                }
            }
            if !is_auth {
                return Ok(Response::builder().status(407).body("")?.into_type());
            }
        }

        // 判断用户协议
        match request.method() {
            &Method::Connect => {
                // https返回200内容直接进行远端和客户端的双向绑定
                self.stream = Some(stream);
                return Ok(Response::builder().status(200).body("")?.into_type());
            }
            _ => {
                // http协议，需要将客户端的内容转发到服务端，并将服务端数据转回客户端
                let client = Client::new(ClientOption::default(), MaybeHttpsStream::Http(stream));
                let (mut recv, sender) = client.send2(request.replace_clone(Body::empty())).await?;
                match recv.recv().await {
                    Some(res) => {
                        self.sender = Some(sender);
                        self.receiver = Some(recv);
                        return Ok(res?)
                    },
                    None => return Err(ProtError::Extension("already close by other")),
                }
            }
        }

    }
}

impl ProxyHttp {
    pub async fn process<T>(
        username: &Option<String>,
        password: &Option<String>,
        mut inbound: T,
    ) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        // 预读数据找出对应的协议 
        let mut buffer = BinaryMut::with_capacity(24);
        let size = {
            let mut buf = ReadBuf::uninit(buffer.chunk_mut());
            inbound.read_buf(&mut buf).await?;
            buf.filled().len()
        };

        if size == 0 {
            return Err(ProxyError::Extension("empty"));
        }
        unsafe {
            buffer.advance_mut(size);
        }
        // socks5 协议, 直接返回, 交给socks5层处理
        if buffer.as_slice()[0] == 5 {
            return Err(ProxyError::Continue((Some(buffer), inbound)));
        }

        let mut max_req_num = usize::MAX;
        // https 协议, 以connect开头, 仅处理一条HTTP请求
        if buffer.as_slice()[0] == b'C' || buffer.as_slice()[0] == b'c' {
            max_req_num = 1;
        }
        
        let mut server = Server::new_by_cache(inbound, None, buffer);
        let mut operate = Operate {
            username: username.clone(),
            password: password.clone(),
            stream: None,
            sender: None,
            receiver: None,
        };
        server.set_max_req(max_req_num);
        let _e = server.incoming(&mut operate).await?;
        if let Some(outbound) = &mut operate.stream {
            let mut inbound = server.into_io();
            let _ = copy_bidirectional(&mut inbound, outbound).await?;
        }
        Ok(())
    }
}