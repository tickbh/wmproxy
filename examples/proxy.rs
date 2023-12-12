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
// Created Date: 2023/08/16 06:51:59

// #![deny(warnings)]

use std::{net::SocketAddr};

use tokio::sync::mpsc::{channel, Sender};
use webparse::Request;
use wenmeng::Client;
use wmproxy::{ConfigOption, ProxyResult, ControlServer, Helper, ProxyConfig, WMCore};


async fn run_proxy() -> ProxyResult<(SocketAddr, Sender<()>)> {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let proxy = ProxyConfig::builder().bind_addr(addr).into_value().unwrap();
    let option = ConfigOption::new_by_proxy(proxy);
    let (sender_close, receiver_close) = channel::<()>(1);
    let mut proxy = WMCore::new(option);
    proxy.ready_serve().await.unwrap();
    let addr = proxy.center_listener.as_ref().unwrap().local_addr()?;
    println!("addr = {:?}", addr);
    // proxy.
    tokio::spawn(async move {
        let _ = proxy.run_serve(receiver_close, None).await;
    });
    Ok((addr, sender_close))
}


async fn run_main() -> ProxyResult<()> {
    let (addr, sender) = run_proxy().await?;
    
    let url = "http://www.baidu.com";
    let req = Request::builder().method("GET").url(url).body("").unwrap();
    let client = Client::builder()
        .add_proxy(&format!("socks5://{}", addr))?
        .connect(url).await.unwrap();
    let (mut recv, _sender) = client.send2(req.into_type()).await?;
    let mut res = recv.recv().await.unwrap()?;
    res.body_mut().wait_all().await;
    let res = res.into_type::<String>();
    println!("res status = {:?}", res.status());
    println!("return body = {}", 
    unsafe {
        res.body().as_str().get_unchecked(0..15)
    }
    );
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run_main().await {
        println!("运行wmproxy发生错误:{:?}", e);
    }
}