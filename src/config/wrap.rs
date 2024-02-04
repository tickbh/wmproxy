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
// Created Date: 2024/01/25 02:13:35

use std::{
    fmt::Display,
    net::{AddrParseError, SocketAddr},
    str::FromStr,
};

fn parse_socker_addr(s: &str) -> Result<SocketAddr, AddrParseError> {
    if s.starts_with(":") {
        let addr = format!("127.0.0.1{s}").parse::<SocketAddr>()?;
        Ok(addr)
    } else {
        let addr = s.parse::<SocketAddr>()?;
        Ok(addr)
    }
}
#[derive(Debug, Clone, Copy)]
pub struct WrapAddr(pub SocketAddr);

impl FromStr for WrapAddr {
    type Err = AddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(WrapAddr(parse_socker_addr(s)?))
    }
}

impl Display for WrapAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

#[derive(Debug, Clone)]
pub struct WrapVecAddr(pub Vec<SocketAddr>);
impl FromStr for WrapVecAddr {
    type Err = AddrParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 范围的如:8080-:8090, 表示11端口
        if s.contains("-") {
            let vals = s
                .split(&['-'])
                .filter(|s| !s.is_empty())
                .collect::<Vec<&str>>();
            let start = parse_socker_addr(vals[0])?;
            if vals.len() != 2 {
                return Ok(WrapVecAddr(vec![start]));
            } else {
                let end = parse_socker_addr(vals[1])?;
                let mut results = vec![];
                for port in start.port()..=end.port() {
                    let mut addr = start.clone();
                    addr.set_port(port);
                    results.push(addr);
                }
                return Ok(WrapVecAddr(results));
            }
        } else {
            let vals = s
                .split(&[',', ' '])
                .filter(|s| !s.is_empty())
                .collect::<Vec<&str>>();
            let mut results = vec![];
            for s in vals {
                results.push(parse_socker_addr(s)?);
            }
            Ok(WrapVecAddr(results))
        }
    }
}

impl Display for WrapVecAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut values = vec![];
        for a in &self.0 {
            values.push(format!("{}", a));
        }
        f.write_str(&values.join(","))
    }
}

impl WrapVecAddr {
    pub fn empty() -> Self {
        WrapVecAddr(vec![])
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    
    pub fn contains(&self, port: u16) -> bool {
        for v in &self.0 {
            if v.port() == port {
                return true;
            }
        }
        false
    }
}
