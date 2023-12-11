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
// Created Date: 2023/09/15 11:39:49

use std::{str::FromStr, io, fmt::Display};

use bitflags::bitflags;
use serde::{Serialize, Deserialize};

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct Flag: u8 {
        /// 使用HTTP代理类型
        const HTTP = 0x1;
        /// 使用HTTPS代理类型
        const HTTPS = 0x2;
        /// 使用SOCKS5代理类型
        const SOCKS5 = 0x4;
        /// 纯TCP转发
        const TCP = 0x8;
        /// 纯UDP转发
        const UDP = 0x16;
    }
}

impl Default for Flag {
    fn default() -> Self {
        Flag::HTTP | Flag::HTTPS | Flag::SOCKS5
    }
}

impl Serialize for Flag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        serializer.serialize_u8(self.bits())
    }
}

impl<'a> Deserialize<'a> for Flag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a> {
        let v = u8::deserialize(deserializer)?;
        Ok(Flag::from_bits(v).unwrap_or(Flag::HTTP))
    }
}


impl FromStr for Flag {
    type Err=io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, ""));
        }

        let vals = s.split_whitespace().collect::<Vec<&str>>();
        let mut flag = Flag::empty();
        for v in vals {
            match v {
                "http" | "HTTP" => {
                    flag.set(Flag::HTTP, true);
                }
                "https" | "HTTPS" => {
                    flag.set(Flag::HTTPS, true);
                }
                "socks5" | "SOCKS5" => {
                    flag.set(Flag::SOCKS5, true);
                }
                _ => {}
            }
        }
        Ok(flag)
    }
}


impl Display for Flag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut value = String::new();
        if self.contains(Flag::HTTP) {
            value += "http";
        }
        
        if self.contains(Flag::HTTPS) {
            if !value.is_empty() {
                value += " https";
            } else {
                value += "https";
            }
        }
        
        if self.contains(Flag::SOCKS5) {
            if !value.is_empty() {
                value += " socks5";
            } else {
                value += "socks5";
            }
        }
        f.write_str(&value)
    }
}


