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
// Created Date: 2023/11/24 03:29:55

use std::{fmt::Display, str::FromStr};

use webparse::Response;
use wenmeng::{Middleware};

use async_trait::async_trait;

use wenmeng::{ProtResult, RecvRequest, RecvResponse, Rate};

use crate::{data::LimitReqData, data::LimitResult, ConfigDuration, ConfigSize, ProxyError, ConfigRate};

#[derive(Debug, Clone)]
pub struct LimitReqZone {
    /// 键值的匹配方式
    pub key: String,
    /// IP个数
    pub limit: u64,

    pub rate: Rate,
}

impl LimitReqZone {
    pub fn new(key: String, limit: u64, rate: Rate) -> Self {
        Self {
            key,
            limit,
            rate,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitReq {
    zone: String,
    burst: u64,
}

impl LimitReq {
    pub fn new(zone: String, burst: u64) -> Self {
        Self { zone, burst }
    }
}

pub struct LimitReqMiddleware {
    req: LimitReq,
}

impl LimitReqMiddleware {
    pub fn new(req: LimitReq) -> Self {
        Self { req }
    }
}

#[async_trait]
impl Middleware for LimitReqMiddleware {
    async fn process_request(
        &mut self,
        request: &mut RecvRequest,
    ) -> ProtResult<Option<RecvResponse>> {
        if let Some(client_ip) = request.headers().system_get("{client_ip}") {
            // let a = LimitReqData::recv_new_req(&self.req.zone, client_ip, self.req.burst)?;
            // println!("a === {:?}", a);
            match LimitReqData::recv_new_req(&self.req.zone, client_ip, self.req.burst)? {
                LimitResult::Ok => return Ok(None),
                LimitResult::Refuse => {
                    return Ok(Some(
                        Response::text().status(404).body("limit req")?.into_type(),
                    ));
                }
                LimitResult::Delay(delay) => {
                    tokio::time::sleep(delay).await;
                    return Ok(None);
                }
            }
        }
        Ok(None)
    }
    async fn process_response(
        &mut self,
        _request: &mut RecvRequest,
        _response: &mut RecvResponse,
    ) -> ProtResult<()> {
        Ok(())
    }
}

impl Display for LimitReqZone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{} limit={} rate={}r/{}",
            self.key,
            ConfigSize::new(self.limit),
            self.rate.nums,
            ConfigDuration::new(self.rate.per)
        ))
    }
}

impl FromStr for LimitReqZone {
    type Err = ProxyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v = s.split(" ").collect::<Vec<&str>>();
        let key = v[0].to_string();
        let mut limit = 0;
        let mut rate = Rate::default();
        for idx in 1..v.len() {
            let key_value = v[idx].split("=").map(|k| k.trim()).collect::<Vec<&str>>();
            if key_value.len() <= 1 {
                return Err(ProxyError::Extension("未知的LimitReq"));
            }
            match key_value[0] {
                "limit" => {
                    let s = ConfigSize::from_str(key_value[1])?;
                    limit = s.0;
                }
                "rate" => {
                    let c = ConfigRate::from_str(key_value[1])?;
                    rate = c.0;
                }
                _ => {
                    return Err(ProxyError::Extension("未知的LimitReq"));
                }
            }
        }

        Ok(LimitReqZone::new(key, limit, rate))
    }
}

impl Display for LimitReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("zone={} brust={}", self.zone, self.burst))
    }
}

impl FromStr for LimitReq {
    type Err = ProxyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v = s.split(" ").collect::<Vec<&str>>();
        let mut zone = String::new();
        let mut brust = 0;
        for idx in 0..v.len() {
            let key_value = v[idx].split("=").map(|k| k.trim()).collect::<Vec<&str>>();
            if key_value.len() <= 1 {
                return Err(ProxyError::Extension("未知的LimitReq"));
            }
            match key_value[0] {
                "zone" => {
                    zone = key_value[1].to_string();
                }
                "brust" => {
                    brust = key_value[1]
                        .parse::<u64>()
                        .map_err(|_e| ProxyError::Extension("parse error"))?;
                }
                _ => {
                    return Err(ProxyError::Extension("未知的LimitReq"));
                }
            }
        }

        Ok(LimitReq::new(zone, brust))
    }
}
