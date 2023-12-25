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
// Created Date: 2023/12/19 03:47:12

use std::{fmt::Display, str::FromStr};

use webparse::{Response, StatusCode};
use wenmeng::Middleware;

use async_trait::async_trait;

use wenmeng::{ProtResult, Rate, RecvRequest, RecvResponse};

use crate::{
    data::LimitReqData, data::LimitResult, ConfigDuration, ConfigRate, ConfigSize, ProxyError,
};

#[derive(Debug, Clone)]
pub struct TryPathsConfig {
    pub list: Vec<String>,
    pub fail_status: StatusCode,
}

impl TryPathsConfig {
    pub fn new(list: Vec<String>, fail_status: StatusCode) -> Self {
        Self { list, fail_status }
    }
}

impl FromStr for TryPathsConfig {
    type Err = ProxyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v = s.split(" ").collect::<Vec<&str>>();
        let mut list = vec![];
        let mut status = StatusCode::from_u16(404).unwrap();
        for idx in 0..v.len() {
            let val = v[idx];
            if val.starts_with("=") {
                if let Ok(code) = val.trim_start_matches("=").parse::<u16>() {
                    if let Ok(code) = StatusCode::from_u16(code) {
                        status = code;
                    }
                }
            } else {
                list.push(val.to_string());
            }
        }

        Ok(TryPathsConfig::new(list, status))
    }
}

impl Display for TryPathsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{} {}", self.list.join(" "), self.fail_status))
    }
}
