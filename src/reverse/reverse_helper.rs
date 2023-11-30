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

use std::net::SocketAddr;

use wenmeng::{ProtResult, ProtError};

use super::UpstreamConfig;


pub struct ReverseHelper;

impl ReverseHelper {

    pub fn get_upstream_addr(upstream: &Vec<UpstreamConfig>, name: &str) -> ProtResult<SocketAddr> {
        for stream in upstream {
            if &stream.name == name {
                return stream.get_server_addr()
            } else if name == "" {
                return stream.get_server_addr()
            }
        }
        return Err(ProtError::Extension(""));
    }
}