// Copyright 2022 - 2024 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Author: tickbh
// -----
// Created Date: 2024/01/24 09:42:22

use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use webparse::Response;
use wenmeng::{RecvRequest, ProtResult, RecvResponse};

use crate::{Helper, ProxyError};

/// HTTP静态数据返回
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StaticResponse {
    body: &'static str,
}

impl StaticResponse {
    pub async fn deal_request(&self, req: &mut RecvRequest) -> ProtResult<RecvResponse> {
        let val = Helper::format_req(req, self.body);
        return Ok(Response::text().body(val).unwrap().into_type());
    }
}

impl FromStr for StaticResponse {
    type Err = ProxyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(StaticResponse {
            body: Helper::get_static_str(s),
        })
    }
}

impl Display for StaticResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.body)
    }
}
