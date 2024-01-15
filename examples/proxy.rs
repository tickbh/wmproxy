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
use wmproxy::{ConfigOption, ProxyResult, ProxyConfig, WMCore};


static HTTP_URL: &str = "http://www.baidu.com";
static HTTPS_URL: &str = "https://www.baidu.com";

async fn run_proxy(
    proxy: ProxyConfig,
) -> ProxyResult<(SocketAddr, Sender<()>)> {
    let option = ConfigOption::new_by_proxy(proxy);
    let (sender_close, receiver_close) = channel::<()>(1);
    let mut proxy = WMCore::new(option);
    proxy.ready_serve().await.unwrap();
    let addr = proxy.center_listener.as_ref().unwrap().local_addr()?;
    tokio::spawn(async move {
        let _ = proxy.run_serve(receiver_close, None).await;
    });
    Ok((addr, sender_close))
}

async fn test_proxy(
    addr: SocketAddr,
    url: &str,
    method: &str,
    auth: Option<(String, String)>,
    is_failed: bool,
) {
    let req = Request::builder().method("GET").url(url).body("").unwrap();
    let proxy = if auth.is_some() {
        format!(
            "{}://{}:{}@{}",
            method,
            auth.as_ref().unwrap().0,
            auth.as_ref().unwrap().1,
            addr
        )
    } else {
        format!("{}://{}", method, addr)
    };
    let client = match Client::builder()
        .add_proxy(&proxy)
        .unwrap()
        .url(url).unwrap()
        .connect()
        .await
    {
        Ok(client) => {
            client
        }
        Err(_) => {
            if is_failed {
                return;
            }
            assert!(false);
            unreachable!();
        }
    };
    let mut res = match client.send_now(req.into_type()).await {
        Ok(res) => {
            if is_failed {
                println!("status {:?}", res.status());
                assert!(res.status() != 200);
                return ;
            }
            res
        }
        Err(_) => {
            if is_failed {
                return;
            }
            assert!(false);
            unreachable!();
        }
    };
    res.body_mut().wait_all().await;
    let res = res.into_type::<String>();
    assert_eq!(res.status(), 200);
    assert!(unsafe { res.body().as_str().get_unchecked(0..15).contains("html") });
    assert!(!is_failed);
}


#[tokio::main]
async fn main() {

    
    let addr  = "localhost:123".parse::<SocketAddr>();
    println!("addr = {:?}", addr);

    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let username = "wmproxy".to_string();
        let password = "wmproxy".to_string();

        let proxy = ProxyConfig::builder()
            .bind_addr(addr)
            .username(Some(username.clone()))
            .password(Some(password.clone()))
            .center(true)
            .mode("server".to_string())
            .into_value()
            .unwrap();

        let (server_addr, _sender) = run_proxy(proxy)
            .await
            .unwrap();
        
        let proxy = ProxyConfig::builder()
            .bind_addr(addr)
            .username(Some(username.clone()))
            .password(Some(password.clone()))
            .center(true)
            .server(Some(format!("{}", server_addr)))
            .into_value()
            .unwrap();

        let (addr, _sender) = run_proxy(proxy)
            .await
            .unwrap();

        // test_proxy(addr, HTTP_URL, "http", None, true).await;
        // test_proxy(addr, HTTPS_URL, "http", None, true).await;
        // test_proxy(addr, HTTP_URL, "socks5", None, true).await;

        let auth = Some((username, password));
        test_proxy(addr, HTTP_URL, "http", auth.clone(), false).await;
        test_proxy(addr, HTTPS_URL, "http", auth.clone(), false).await;
        test_proxy(addr, HTTP_URL, "socks5", auth.clone(), false).await;
}