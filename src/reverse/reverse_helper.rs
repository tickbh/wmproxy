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
// Created Date: 2023/10/21 10:39:07

use std::{net::SocketAddr, sync::Arc};

use wenmeng::{RecvRequest};

use super::{UpstreamConfig, ServerConfig, LocationConfig};


pub struct ReverseHelper;

impl ReverseHelper {

    pub fn get_upstream_addr(upstream: &Vec<UpstreamConfig>, name: &str) -> Option<SocketAddr> {
        for stream in upstream {
            if &stream.name == name {
                return stream.get_server_addr()
            } else if name == "" {
                return stream.get_server_addr()
            }
        }
        return None;
    }
    
    pub fn get_location_by_req<'a>(servers: &'a Vec<Arc<ServerConfig>>, req: &RecvRequest) -> Option<&'a LocationConfig> {
        let server_len = servers.len();
        let host = req.get_host().unwrap_or(String::new());
        // 不管有没有匹配, 都执行最后一个
        for (index, s) in servers.iter().enumerate() {
            if s.up_name == host || host.is_empty() || index == server_len - 1 {
                let path = req.path().clone();
                for idx in 0..s.location.len() {
                    if s.location[idx].is_match_rule(&path, req.method()) {
                        return Some(&s.location[idx]);
                    }
                }
            }
        }
        return None;
    }
}