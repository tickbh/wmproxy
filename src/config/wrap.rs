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
    fmt::Display, net::{AddrParseError, IpAddr, Ipv4Addr, SocketAddr}, str::FromStr
};

use local_ip_address::{local_ip, local_ipv6};

fn parse_socker_addr(s: &str) -> Result<Vec<SocketAddr>, AddrParseError> {
    if s.starts_with(":") {
        let port = s.trim_start_matches(':');
        let mut results = vec![];
        if let Ok(port) = port.parse::<u16>() {
            if let Ok(v) = local_ip() {
                results.push(SocketAddr::new(v, port));
            }
            if let Ok(v) = local_ipv6() {
                results.push(SocketAddr::new(v, port));
            }
            results.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port));
        } else {
            results.push(format!("127.0.0.1{s}").parse::<SocketAddr>()?);
        }
        Ok(results)
    } else {
        let addr = s.parse::<SocketAddr>()?;
        Ok(vec![addr])
    }
}
#[derive(Debug, Clone, Copy)]
pub struct WrapAddr(pub SocketAddr);

impl FromStr for WrapAddr {
    type Err = AddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(WrapAddr(parse_socker_addr(s)?[0]))
    }
}

impl Display for WrapAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

/// 地址解析中转
/// * 正常IP解析
///   - `127.0.0.1:8869` 解析成 ipv4 127.0.0.1 端口 8869，只接受本地来的连接信息
///   - `0.0.0.0:8869` 解析成 ipv4 0.0.0.0 端口 8869，可接受所有来自ipv4的连接信息
///
/// * 以`:`开头的地址，且不包含`-`
///   - `:8869` 解析成 ipv4 127.0.0.1 端口 8869 及 ipv4 192.168.0.100 端口 8869
///
/// * 包含`-`的地址
///   - `:8869-:8871` 解析成 ipv4 127.0.0.1 端口 8869 - 8871 三个端口地址 及 ipv4 192.168.0.100 端口 8869 - 8871 三个端口地址，总共6个端口地址
///   - `127.0.0.1:8869-:8871` 解析成 ipv4 127.0.0.1 端口 8869 - 8871 三个端口地址 总共3个端口地址
///   - `127.0.0.1:8869-192.168.0.100:8871` 解析成 ipv4 127.0.0.1 端口 8869 - 8871 三个端口地址 总共3个端口地址，忽略后面的地址，只接受端口号
/// 
/// * 手动多个地址，可以空格或者`,`做间隔
///   - `127.0.0.1:8869 127.0.0.1:8899 192.168.0.100:8899` 就相应的解析成三个端口地址
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
                return Ok(WrapVecAddr(start));
            } else {
                let end = parse_socker_addr(vals[1])?;
                let mut results = vec![];
                for port in start[0].port()..=end[1].port() {
                    for idx in &start {
                        let mut addr = idx.clone();
                        addr.set_port(port);
                        results.push(addr);
                    }
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
                results.extend(parse_socker_addr(s)?);
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
