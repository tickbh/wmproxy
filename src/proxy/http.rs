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
use base64::engine::general_purpose;
use tokio::{io::{copy_bidirectional, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf}, net::TcpStream, sync::mpsc::{Receiver, Sender}};
use webparse::{BinaryMut, Buf, BufMut, HttpError, Method, WebError, Response};
use wenmeng::{OperateTrait, RecvRequest, ProtResult, RecvResponse, Server, Client, ClientOption, ProtError, MaybeHttpsStream, RecvStream};

pub struct ProxyHttp {}

struct Operate {
    username: Option<String>,
    password: Option<String>,
    stream: Option<TcpStream>,
    sender: Option<Sender<RecvRequest>>,
    receiver: Option<Receiver<ProtResult<RecvResponse>>>,
}

impl Operate {
    
    pub fn check_basic_auth<U, P>(value: &str) -> bool
    where
        U: std::fmt::Display,
        P: std::fmt::Display,
    {
        use base64::prelude::BASE64_STANDARD;
        use base64::read::DecoderReader;
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
        }

        return false;
    }
}

#[async_trait]
impl OperateTrait for &mut Operate {
    async fn operate(&mut self, request: &mut RecvRequest) -> ProtResult<RecvResponse> {
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

        if self.username.is_some() && self.password.is_some() {
            let mut auth = false;
            if let Some(auth) = request.headers().get_option_value(&"Proxy-Authenticate") {
                if let Some(val) = auth.as_string() {

                }
            }
            if !auth {
                return Ok(Response::builder().status(407).body("")?.into_type());
            }
            
            
        }
        
        match request.method() {
            &Method::Connect => {
                self.stream = Some(stream);
                return Ok(Response::builder().status(200).body("")?.into_type());
            }
            _ => {
                let client = Client::new(ClientOption::default(), MaybeHttpsStream::Http(stream));
                let (mut recv, sender) = client.send2(request.replace_clone(RecvStream::empty())).await?;
                self.sender = Some(sender);
                match recv.recv().await {
                    Some(res) => {
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
    async fn err_server_status<T>(mut inbound: T, status: u16) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut res = webparse::Response::builder().status(status).body(())?;
        inbound.write_all(&res.httpdata()?).await?;
        Ok(())
    }

    // pub async fn process<T>(
    //     username: &Option<String>,
    //     password: &Option<String>,
    //     mut inbound: T,
    // ) -> Result<(), ProxyError<T>>
    // where
    //     T: AsyncRead + AsyncWrite + Unpin,
    // {
    //     let mut outbound;
    //     let mut request;
    //     let mut buffer = BinaryMut::new();
    //     loop {
    //         let size = {
    //             let mut buf = ReadBuf::uninit(buffer.chunk_mut());
    //             inbound.read_buf(&mut buf).await?;
    //             buf.filled().len()
    //         };

    //         if size == 0 {
    //             return Err(ProxyError::Extension("empty"));
    //         }
    //         unsafe {
    //             buffer.advance_mut(size);
    //         }
    //         request = webparse::Request::new();
    //         // 通过该方法解析标头是否合法, 若是partial(部分)则继续读数据
    //         // 若解析失败, 则表示非http协议能处理, 则抛出错误
    //         // 此处clone为浅拷贝，不确定是否一定能解析成功，不能影响偏移
    //         match request.parse_buffer(&mut buffer.clone()) {
    //             Ok(_) => match request.get_connect_url() {
    //                 Some(host) => {
    //                     match HealthCheck::connect(&host).await {
    //                         Ok(v) => outbound = v,
    //                         Err(e) => {
    //                             Self::err_server_status(inbound, 503).await?;
    //                             return Err(ProxyError::from(e));
    //                         }
    //                     }
    //                     break;
    //                 }
    //                 None => {
    //                     if !request.is_partial() {
    //                         Self::err_server_status(inbound, 503).await?;
    //                         return Err(ProxyError::UnknownHost);
    //                     }
    //                 }
    //             },
    //             Err(WebError::Http(HttpError::Partial)) => {
    //                 continue;
    //             }
    //             Err(_) => {
    //                 return Err(ProxyError::Continue((Some(buffer), inbound)));
    //             }
    //         }
    //     }

    //     match request.method() {
    //         &Method::Connect => {
    //             log::trace!(
    //                 "https connect {:?}",
    //                 String::from_utf8_lossy(buffer.chunk())
    //             );
    //             inbound.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await?;
    //         }
    //         _ => {
    //             outbound.write_all(buffer.chunk()).await?;
    //         }
    //     }
    //     let _ = copy_bidirectional(&mut inbound, &mut outbound).await?;
    //     Ok(())
    // }

    
    pub async fn process<T>(
        username: &Option<String>,
        password: &Option<String>,
        mut inbound: T,
    ) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        // let mut outbound;
        // let mut request;
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
        // socks5 协议
        if buffer.as_slice()[0] == 5 {
            return Err(ProxyError::Continue((Some(buffer), inbound)));
        }

        let mut max_req_num = usize::MAX;
        // https 协议, 以connect开头
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
        let e = server.incoming(&mut operate).await;
        if let Some(outbound) = &mut operate.stream {
            let mut inbound = server.into_io();
            let _ = copy_bidirectional(&mut inbound, outbound).await?;
        }
        // request = webparse::Request::new();
        // // 通过该方法解析标头是否合法, 若是partial(部分)则继续读数据
        // // 若解析失败, 则表示非http协议能处理, 则抛出错误
        // // 此处clone为浅拷贝，不确定是否一定能解析成功，不能影响偏移
        // match request.parse_buffer(&mut buffer.clone()) {
        //     Ok(_) => match request.get_connect_url() {
        //         Some(host) => {
        //             match HealthCheck::connect(&host).await {
        //                 Ok(v) => outbound = v,
        //                 Err(e) => {
        //                     Self::err_server_status(inbound, 503).await?;
        //                     return Err(ProxyError::from(e));
        //                 }
        //             }
        //             break;
        //         }
        //         None => {
        //             if !request.is_partial() {
        //                 Self::err_server_status(inbound, 503).await?;
        //                 return Err(ProxyError::UnknownHost);
        //             }
        //         }
        //     },
        //     Err(WebError::Http(HttpError::Partial)) => {
        //         continue;
        //     }
        //     Err(_) => {
        //         return Err(ProxyError::Continue((Some(buffer), inbound)));
        //     }
        // }

        // match request.method() {
        //     &Method::Connect => {
        //         log::trace!(
        //             "https connect {:?}",
        //             String::from_utf8_lossy(buffer.chunk())
        //         );
        //         inbound.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await?;
        //     }
        //     _ => {
        //         outbound.write_all(buffer.chunk()).await?;
        //     }
        // }
        // let _ = copy_bidirectional(&mut inbound, &mut outbound).await?;
        Ok(())
    }
}
