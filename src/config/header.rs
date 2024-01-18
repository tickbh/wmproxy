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
// Created Date: 2023/12/04 10:44:24

use std::{fmt::Display, io, str::FromStr};

use crate::Helper;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderOper {
    Add,
    Del,
    Default,
    Replace,
}

impl HeaderOper {
    pub fn to_u8(&self) -> u8 {
        match &self {
            HeaderOper::Add => 1,
            HeaderOper::Del => 2,
            HeaderOper::Default => 3,
            HeaderOper::Replace => 4,
        }
    }

    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Add,
            2 => Self::Del,
            3 => Self::Default,
            _ => Self::Replace,
        }
    }
}

impl Display for HeaderOper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            HeaderOper::Add => f.write_str("+"),
            HeaderOper::Del => f.write_str("-"),
            HeaderOper::Replace => f.write_str(""),
            HeaderOper::Default => f.write_str("?"),
        }
    }
}

impl FromStr for HeaderOper {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v = match s {
            "+" => HeaderOper::Add,
            "-" => HeaderOper::Del,
            "?" => HeaderOper::Default,
            _ => HeaderOper::Replace,
        };
        Ok(v)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigHeader {
    pub oper: HeaderOper,
    pub is_proxy: bool,
    pub key: String,
    pub val: String,
}

impl ConfigHeader {
    pub fn new(oper: HeaderOper, is_proxy: bool, key: String, val: String) -> Self {
        Self {
            oper,
            is_proxy,
            key,
            val,
        }
    }
}

impl FromStr for ConfigHeader {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, ""));
        }

        let vals = Helper::split_by_whitespace(s);
        let mut oper = HeaderOper::Replace;
        let mut is_proxy = false;
        let (key, val) = {
            match (vals.len(), vals[0]) {
                (4, "proxy") => {
                    is_proxy = true;
                    oper = HeaderOper::from_str(vals[1])?;
                    (vals[2].to_string(), vals[3].to_string())
                }
                (3, "proxy") => {
                    is_proxy = true;
                    (vals[1].to_string(), vals[2].to_string())
                }
                (3, _) => {
                    oper = HeaderOper::from_str(vals[0])?;
                    (vals[1].to_string(), vals[2].to_string())
                }
                (2, "-") => {
                    oper = HeaderOper::from_str(vals[0])?;
                    (vals[1].to_string(), String::new())
                }
                (2, _) => (vals[0].to_string(), vals[1].to_string()),
                _ => {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, ""));
                }
            }
        };

        Ok(ConfigHeader::new(oper, is_proxy, key, val))
    }
}

impl Display for ConfigHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.is_proxy, &self.oper) {
            (true, HeaderOper::Default) => {
                f.write_fmt(format_args!("proxy {} {}", self.key, self.val))
            }
            (true, _) => f.write_fmt(format_args!(
                "proxy {} {} {}",
                self.oper, self.key, self.val
            )),
            (_, HeaderOper::Default) => f.write_fmt(format_args!("{} {}", self.key, self.val)),
            (_, _) => f.write_fmt(format_args!("{} {} {}", self.oper, self.key, self.val)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::HeaderOper;

    macro_rules! header_compare {
        ($raw:expr, $proxy:expr, $oper:expr, $key:expr, $val:expr) => {{
            let config = crate::ConfigHeader::from_str($raw).unwrap();
            assert_eq!(config.is_proxy, $proxy);
            assert_eq!(config.oper, $oper);
            assert_eq!(&config.key, $key);
            assert_eq!(&config.val, $val);
        }};
    }

    #[test]
    fn test_header() {
        header_compare!("proxy + key val", true, HeaderOper::Add, "key", "val");
        header_compare!("proxy key val", true, HeaderOper::Replace, "key", "val");
        header_compare!("+ key val", false, HeaderOper::Add, "key", "val");
        header_compare!("? key val", false, HeaderOper::Default, "key", "val");
        header_compare!("- key", false, HeaderOper::Del, "key", "");
        // 多空格
        header_compare!(
            "proxy     + key     val",
            true,
            HeaderOper::Add,
            "key",
            "val"
        );
        // 包含引号
        header_compare!(
            "proxy + key     \"val 1\"",
            true,
            HeaderOper::Add,
            "key",
            "val 1"
        );
        header_compare!(
            "proxy + key     'val 1'",
            true,
            HeaderOper::Add,
            "key",
            "val 1"
        );
    }
}
