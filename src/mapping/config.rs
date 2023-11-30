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
// Created Date: 2023/10/07 09:40:42

use std::{net::SocketAddr, vec};

use serde::{Deserialize, Serialize};

fn default_domain() -> String {
    "".to_string()
}


fn default_header() -> Vec<Vec<String>> {
    vec![]
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MappingConfig {
    pub name: String,
    pub mode: String,
    pub local_addr: Option<SocketAddr>,
    #[serde(default = "default_domain")]
    pub domain: String,
    #[serde(default = "default_header")]
    pub headers: Vec<Vec<String>>,
}

impl MappingConfig {
    pub fn new(name: String, mode: String, domain: String, headers: Vec<Vec<String>>) -> Self {
        MappingConfig {
            name,
            mode,
            local_addr: None,
            domain,
            headers,
        }
    }

    pub fn is_http(&self) -> bool {
        self.mode.eq_ignore_ascii_case("http")
    }

    pub fn is_https(&self) -> bool {
        self.mode.eq_ignore_ascii_case("https")
    }

    pub fn is_tcp(&self) -> bool {
        self.mode.eq_ignore_ascii_case("tcp")
    }
}
