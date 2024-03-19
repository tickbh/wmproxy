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
// Created Date: 2023/09/15 11:37:09

use std::{
    io::{self},
    net::SocketAddr,
    sync::Arc,
};

use futures::{future::select_all, FutureExt, StreamExt};

use rustls::ClientConfig;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex,
    },
};
use tokio_rustls::{rustls, TlsAcceptor};

use crate::{
    arg,
    core::{Listeners, Server, ServiceTrait, WrapListener},
    option::ConfigOption,
    proxy::{CenterApp, MappingApp, ProxyServer},
    reverse::{HttpApp, HttpConfig, ServerConfig, StreamApp, StreamConfig, StreamUdp, StreamUdpService, WrapTlsAccepter},
    ActiveHealth, CenterClient, CenterServer, CenterTrans, Flag, Helper, OneHealth, ProxyApp,
    ProxyResult,
};

/// 核心处理类
pub struct WMCore {
    pub option: ConfigOption,
    pub sender: Sender<()>,
    pub receiver: Option<Receiver<()>>,
}

impl WMCore {
    pub fn new(option: ConfigOption) -> WMCore {
        let (sender, receiver) = channel(1); 
        Self {
            option,
            sender,
            receiver: Some(receiver),
        }
    }

    // pub async fn do_start_health_check(&mut self) -> ProxyResult<()> {
    //     let healths = self.option.get_health_check();
    //     let (sender, receiver) = channel::<Vec<OneHealth>>(1);
    //     let _active = ActiveHealth::new(healths, receiver);
    //     // active.do_start()?;
    //     self.health_sender = Some(sender);
    //     Ok(())
    // }
    
    pub fn get_sender(&self) -> Sender<()> {
        self.sender.clone()
    }

    pub fn take_receiver(&mut self) -> Option<Receiver<()>> {
        self.receiver.take()
    }

    pub fn init(&self) -> ProxyResult<()> {
        Helper::try_init_log(&self.option);
        let pidfile = self.option.pidfile.clone();
        let _ = Helper::try_create_pidfile(&pidfile);
        Ok(())
    }

    pub fn run_main() -> ProxyResult<()> {
        let option = arg::parse_env().expect("load config failed");
        Self::run_main_opt(option)?;
        Ok(())
    }

    
    pub fn run_main_opt(option: ConfigOption) -> ProxyResult<()> {
        let core = WMCore::new(option);
        core.init()?;
        let mut server = core.build_server()?;
        server.run_loop();
        Ok(())
    }

    
    pub fn run_main_service(option: ConfigOption, services: Vec<Box<dyn ServiceTrait>>) -> ProxyResult<()> {
        let core = WMCore::new(option);
        core.init()?;
        core.run_services(services)
    }

    pub fn run_server(&self, mut server: Server) -> ProxyResult<()> {
        server.run_loop();
        Ok(())
    }

    pub fn run_services(&self, services: Vec<Box<dyn ServiceTrait>>) -> ProxyResult<()> {
        let mut server = Server::new(Some(self.option.clone()));
        server.add_services(services);
        server.run_loop();
        Ok(())
    }

    pub fn build_server(&self) -> ProxyResult<Server> {
        let mut server = Server::new(Some(self.option.clone()));
        let services = self.build_services()?;
        server.add_services(services);
        Ok(server)
    }

    pub fn build_services(&self) -> ProxyResult<Vec<Box<dyn ServiceTrait>>> {
        let mut vecs: Vec<Box<dyn ServiceTrait>> = vec![];
        if let Some(config) = &self.option.proxy {
            println!("config = {:?}", config);
            if let Some(_) = config.bind {
                vecs.push(Box::new(ProxyApp::build_services(config.clone())?));
            }
            if let Some(_) = config.center_addr {
                let service = CenterApp::build_services(config.clone())?;
                vecs.push(Box::new(service));
            }
            if config.map_http_bind.is_some() || config.map_https_bind.is_some() || config.map_tcp_bind.is_some() {
                let service = MappingApp::build_services(config.clone())?;
                vecs.push(Box::new(service));
            }
        }

        if let Some(http) = &self.option.http {
            let app = HttpApp::build_services(http.clone())?;
            vecs.push(Box::new(app));
        }
        
        if let Some(stream)= &self.option.stream {
            let app = StreamApp::build_services(stream.clone())?;
            vecs.push(Box::new(app));

            let app = StreamUdpService::build_services(stream.clone())?;
            vecs.push(Box::new(app));
        }

        Ok(vecs)
    }
}
