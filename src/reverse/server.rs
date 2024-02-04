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
// Created Date: 2023/10/18 02:32:15

use std::{collections::HashMap, net::{SocketAddr, ToSocketAddrs}};

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use wenmeng::ProtResult;


use crate::{ConfigHeader, WrapVecAddr};

use super::{LocationConfig, UpstreamConfig, common::CommonConfig, ReverseHelper};

fn default_bind_mode() -> String {
    "tcp".to_string()
}

fn default_up_name() -> String {
    "".to_string()
}
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {

    #[serde_as(as = "DisplayFromStr")]
    pub bind_addr: WrapVecAddr,

    #[serde_as(as = "DisplayFromStr")]
    pub bind_ssl: WrapVecAddr,
    
    #[serde(default = "default_up_name")]
    pub up_name: String,
    pub root: Option<String>,
    pub cert: Option<String>,
    pub key: Option<String>,

    #[serde(default = "default_bind_mode")]
    pub bind_mode: String,
    
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[serde(default = "Vec::new")]
    pub headers: Vec<ConfigHeader>,
    #[serde(default = "Vec::new")]
    pub location: Vec<LocationConfig>,
    #[serde(default = "Vec::new")]
    pub upstream: Vec<UpstreamConfig>,

    #[serde(flatten)]
    #[serde(default = "CommonConfig::new")]
    pub comm: CommonConfig,
}

impl ServerConfig {
    pub fn new(bind_addr: WrapVecAddr) -> Self {
        ServerConfig {
            bind_addr,
            bind_ssl: WrapVecAddr::empty(),
            up_name: default_up_name(),
            root: None,
            cert: None,
            key: None,
            bind_mode: default_bind_mode(),
            headers: vec![],
            location: vec![],
            upstream: vec![],
            comm: CommonConfig::new(),
        }
    }

    pub fn new_ssl(bind_ssl: WrapVecAddr) -> Self {
        ServerConfig {
            bind_addr: WrapVecAddr::empty(),
            bind_ssl,
            up_name: default_up_name(),
            root: None,
            cert: None,
            key: None,
            bind_mode: default_bind_mode(),
            headers: vec![],
            location: vec![],
            upstream: vec![],
            comm: CommonConfig::new(),
        }
    }
    /// 将配置参数提前共享给子级
    pub fn copy_to_child(&mut self) {
        for l in &mut self.location {
            l.comm.copy_from_parent(&self.comm);
            l.comm.pre_deal();
            if let Some(n) = l.rule.get_match_name() {
                if l.comm.match_names.contains_key(&n) {
                    l.rule = l.comm.match_names[&n].clone();
                } else {
                    log::error!("配置匹配名字@{},但未找到相应的配置", n);
                    println!("配置匹配名字@{},但未找到相应的配置", n);
                }
            }
            l.up_name = Some(self.up_name.clone());
            l.upstream.append(&mut self.upstream.clone());
            l.headers.append(&mut self.headers.clone());
            if l.root.is_none() && self.root.is_some() {
                l.root = self.root.clone();
                if let Some(file_server) = &mut l.file_server {
                    if file_server.root.is_none() && self.root.is_some() {
                        file_server.root = self.root.clone();
                    }
                    if file_server.prefix.is_empty() {
                        file_server.set_prefix(l.rule.get_path());
                    }
                    file_server.set_common(l.comm.clone());
                }
            }
        }
    }

    
    pub fn get_log_names(&self, names: &mut HashMap<String, String>)  {
        self.comm.get_log_names(names);
        for l in &self.location {
            l.get_log_names(names);
        }
    }

    pub fn build_url(&self, addr: &SocketAddr) -> String {
        if self.comm.proxy_url.is_some() {
            let mut url = self.comm.proxy_url.clone().unwrap();
            url.domain = Some(format!("{}", addr.ip()));
            url.port = Some(addr.port());
            format!("{}", url)
        } else {
            format!("http://{}", addr)
        }
    }

    pub fn get_addr_domain(&self) -> ProtResult<(Option<SocketAddr>, Option<String>)> {
        let mut domain = self.comm.domain.clone();
        let mut addr = None;
        if self.comm.proxy_url.is_some() {
            if domain.is_none() {
                domain = self.comm.proxy_url.as_ref().unwrap().domain.clone();
            }
            if let Some(domain) = &self.comm.proxy_url.as_ref().unwrap().domain {
                addr = ReverseHelper::get_upstream_addr(&self.upstream, &domain);
                if addr.is_some() && self.comm.proxy_url.as_ref().unwrap().port.is_some() {
                    addr.as_mut().unwrap().set_port(self.comm.proxy_url.as_ref().unwrap().port.unwrap());
                }
            }
            if addr.is_none() {
                if let Some(c) = self.comm.proxy_url.as_ref().unwrap().get_connect_url() {
                    addr = c.to_socket_addrs()?.next();
                }
            }
        }

        if addr.is_none() {
            addr = ReverseHelper::get_upstream_addr(&self.upstream, &self.up_name)
        }
        Ok((addr, domain))
    }
}
