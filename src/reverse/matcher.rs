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
// Created Date: 2024/01/24 03:12:37

use std::{
    collections::HashSet,
    fmt::{self, Display},
    str::FromStr, net::IpAddr,
};

use serde::{
    Deserialize, Serialize,
};
use serde_with::{serde_as, DisplayFromStr};
use webparse::{Method, Scheme, WebError};
use wenmeng::{RecvRequest, ProtResult, ProtError};

use crate::{Helper, IpSets};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchMethod(pub HashSet<Method>);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchScheme(pub HashSet<Scheme>);

/// location匹配，将根据该类的匹配信息进行是否匹配
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Matcher {
    path: Option<String>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    client_ip: Option<IpSets>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    remote_ip: Option<IpSets>,
    host: Option<String>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    method: Option<MatchMethod>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    scheme: Option<MatchScheme>,
}

impl Matcher {
    pub fn new() -> Self {
        Self {
            path: Some("*".to_string()),
            ..Default::default()
        }
    }

    pub fn get_match_name(&self) -> Option<String> {
        if let Some(p) = &self.path {
            if p.starts_with("@") {
                return Some(p.replace("@", ""));
            }
        }
        None
    } 

    pub fn get_path(&self) -> String {
        if let Some(p) = &self.path {
            if p.contains("*") {
                let v = p.replace("*", "");
                if v.len() != 0 {
                    return v;
                }
            } else {
                return p.clone();
            }
        }
        "/".to_string()
    }

    /// 当本地限制方法时,优先匹配方法,在进行路径的匹配
    pub fn is_match_rule(&self, path: &String, req: &RecvRequest) -> ProtResult<bool>  {
        if let Some(p) = &self.path {
            let mut is_match = false;
            println!("path = {path} p === {p}");
            if Helper::is_match(&path, p) {
                is_match = true;
            }
            if !is_match {
                if let Some(re) = Helper::try_cache_regex(&p) {
                    if !re.is_match(path) {
                        return Ok(false);
                    }
                } else {
                    return Ok(false);
                }
            }
        }

        if let Some(m) = &self.method {
            if !m.0.contains(req.method()) {
                return Ok(false);
            }
        }

        if let Some(s) = &self.scheme {
            if !s.0.contains(req.scheme()) {
                return Ok(false);
            }
        }

        if let Some(h) = &self.host {
            match req.get_host() {
                Some(host) if &host == h => {},
                _ => return Ok(false),
            }
        }

        if let Some(c) = &self.client_ip {
            match req.headers().system_get("{client_ip}") {
                Some(ip) => {
                    let ip = ip
                    .parse::<IpAddr>()
                    .map_err(|_| ProtError::Extension("client ip error"))?;
                    if !c.contains(&ip) {
                        return Ok(false)
                    }
                },
                None => return Ok(false),
            }
        }

        Ok(true)
    }
}

impl FromStr for MatchMethod {
    type Err = WebError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut hash = HashSet::new();
        let vals: Vec<&str> = s.split_whitespace().collect();
        for v in vals {
            let m = v.parse::<Method>()?;
            hash.insert(m);
        }
        Ok(Self(hash))
    }
}

impl Display for MatchMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for h in &self.0 {
            f.write_fmt(format_args!("{}", h))?;
        }
        Ok(())
    }
}

impl FromStr for MatchScheme {
    type Err = WebError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut hash = HashSet::new();
        let vals: Vec<&str> = s.split_whitespace().collect();
        for v in vals {
            let m = v.parse::<Scheme>()?;
            hash.insert(m);
        }
        Ok(Self(hash))
    }
}

impl Display for MatchScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for h in &self.0 {
            f.write_fmt(format_args!("{}", h))?;
        }
        Ok(())
    }
}

impl Default for Matcher {
    fn default() -> Self {
        Self {
            path: Default::default(),
            client_ip: Default::default(),
            remote_ip: Default::default(),
            host: Default::default(),
            method: Default::default(),
            scheme: Default::default(),
        }
    }
}

impl FromStr for Matcher {
    type Err = WebError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            path: Some(s.to_string()),
            ..Default::default()
        })
    }
}

impl Display for Matcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(p) = &self.path {
            f.write_str(&*p)?;
        }
        if let Some(p) = &self.host {
            f.write_str(&*p)?;
        }
        Ok(())
    }
}
