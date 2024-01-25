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

use std::{fmt::Display, net::{AddrParseError, SocketAddr}, str::FromStr};


#[derive(Debug, Clone, Copy)]
pub struct WrapAddr(pub SocketAddr);

impl FromStr for WrapAddr {
    type Err = AddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with(":") {
            let addr = format!("127.0.0.1{s}").parse::<SocketAddr>()?;
            Ok(WrapAddr(addr))
        } else {
            let addr = s.parse::<SocketAddr>()?;
            Ok(WrapAddr(addr))
        }
    }
}

impl Display for WrapAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}
