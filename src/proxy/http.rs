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

use crate::{HealthCheck, ProxyError};
use async_trait::async_trait;
use tokio::io::{copy_bidirectional, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use webparse::{BinaryMut, Buf, BufMut, HttpError, Method, WebError, Response};
use wenmeng::{OperateTrait, RecvRequest, ProtResult, RecvResponse, Server, Client};

pub struct ProxyHttp {}

struct Operate {
    username: Option<String>,
    password: Option<String>,
    client: Option<Client>,
}

#[async_trait]
impl OperateTrait for Operate {
    async fn operate(&mut self, req: &mut RecvRequest) -> ProtResult<RecvResponse> {
        let response = Response::builder()
            .version(req.version().clone())
            .body("Hello World\r\n".to_string())?;
        Ok(response.into_type())
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
        
        let mut server = Server::new_by_cache(inbound, None, buffer);
        let operate = Operate {
            username: username.clone(),
            password: password.clone(),
            client: None
        };
        server.set_max_req(1);
        let e = server.incoming(operate).await;
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
