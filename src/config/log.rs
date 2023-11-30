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
// Created Date: 2023/11/13 09:47:12


use std::{fmt::Display, str::FromStr};

use crate::ProxyError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigLog {
    pub name: String,
    pub format: String,
    pub level: log::Level,
}

impl ConfigLog {
    pub fn new(name: String, format: String, level: log::Level) -> Self {
        Self {
            name,
            format,
            level,
        }
    }

    pub fn as_error(&mut self) {
        if !self.format.is_empty() {
            if let Ok(level) = log::Level::from_str(&self.format.to_ascii_lowercase()) {
                self.level = level;
            } else {
                self.level = log::Level::Trace;
            }
            self.format = String::new();
        }
    }
}

impl Display for ConfigLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.format.is_empty() {
            f.write_fmt(format_args!("{} {}", self.name, self.level))
        } else {
            if self.level != log::Level::Trace {
                f.write_fmt(format_args!("{} {} {}", self.name, self.format, self.level))
            } else {
                f.write_fmt(format_args!("{} {}", self.name, self.format))
            }
        }
    }
}

impl FromStr for ConfigLog {
    type Err=ProxyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v: Vec<&str> = s.split(' ').collect();
        if v.len() < 2 {
            return Err(ProxyError::Extension("名称的格式间必须有空格"));
        }
        let name = v[0].to_string();
        let format = v[1].to_string();
        let mut level = log::Level::Trace;
        if v.len() == 3 {
            if let Ok(l) = log::Level::from_str(&v[2]) {
                level = l;
            }
        }
        Ok(Self::new(name, format, level))
    }
}
