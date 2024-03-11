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


use tokio::net::TcpListener;
// #![deny(warnings)]
use wmproxy::{arg, ControlServer, Flag, Helper, ProxyApp, ProxyResult};
use wmproxy::core::{Listeners, Server, WrapListener};
use wmproxy::core::Service;

async fn run_main() -> ProxyResult<()> {
    let option = arg::parse_env()?;
    Helper::try_init_log(&option);
    let pidfile = option.pidfile.clone();
    let _ = Helper::try_create_pidfile(&pidfile);
    let control = ControlServer::new(option);
    control.start_serve().await?;
    let _ = Helper::try_remove_pidfile(&pidfile);
    Ok(())
}

// #[forever_rs::main]
// #[tokio::main]
// async fn main() {
//     if let Err(e) = run_main().await {
//         println!("运行wmproxy发生错误:{:?}", e);
//     }
// }

fn main() {
    let option = arg::parse_env().expect("load config failed");
    Helper::try_init_log(&option);
    let pidfile = option.pidfile.clone();
    let _ = Helper::try_create_pidfile(&pidfile);
    
    let mut server = Server::new(Some(option));
    let proxy = ProxyApp::new(Flag::all(), None, None, None, None);
    let mut listeners = Listeners::new();
    listeners.add(WrapListener::new("0.0.0.0:8090").expect("ok"));
    let service = proxy.build_services(listeners);
    // let service = Service::new("proxy".to_string(), ClientApp::new());
    server.add_service(service);
    server.run_loop();
}