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

use std::{net::SocketAddr, collections::HashMap};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};


use crate::ConfigHeader;

use super::{LocationConfig, UpstreamConfig, common::CommonConfig};

fn default_bind_mode() -> String {
    "tcp".to_string()
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub server_name: String,
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
    /// 将配置参数提前共享给子级
    pub fn copy_to_child(&mut self) {
        for l in &mut self.location {
            l.comm.copy_from_parent(&self.comm);
            l.comm.pre_deal();
            l.server_name = Some(self.server_name.clone());
            l.upstream.append(&mut self.upstream.clone());
            l.headers.append(&mut self.headers.clone());
            if l.root.is_none() && self.root.is_some() {
                l.root = self.root.clone();
                if let Some(file_server) = &mut l.file_server {
                    if file_server.root.is_none() && self.root.is_some() {
                        file_server.root = self.root.clone();
                    }
                    if file_server.prefix.is_empty() {
                        file_server.set_prefix(l.rule.clone());
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
}
