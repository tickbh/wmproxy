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

use std::{net::SocketAddr, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

use crate::ConfigHeader;

fn default_domain() -> String {
    "".to_string()
}


#[serde_as]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MappingConfig {
    pub name: String,
    pub mode: String,
    pub local_addr: Option<SocketAddr>,
    #[serde(default = "default_domain")]
    pub domain: String,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[serde(default = "Vec::new")]
    pub headers: Vec<ConfigHeader>,
}

impl MappingConfig {
    pub fn new(name: String, mode: String, domain: String, headers: Vec<ConfigHeader>) -> Self {
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
    
    pub fn is_proxy(&self) -> bool {
        self.mode.eq_ignore_ascii_case("proxy")
    }
}

impl FromStr for MappingConfig {
    type Err=std::io::Error;

    fn from_str(_s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}