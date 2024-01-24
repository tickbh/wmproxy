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
    str::FromStr,
};

use serde::{
    Deserialize, Serialize,
};
use serde_with::{serde_as, DisplayFromStr};
use webparse::{Method, Scheme, WebError};
use wenmeng::RecvRequest;

use crate::IpSets;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchMethod(pub HashSet<Method>);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchScheme(pub HashSet<Scheme>);

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
    pub fn is_match_rule(&self, path: &String, req: &RecvRequest) -> bool {
        if let Some(p) = &self.path {
            if path.find(&*p).is_none() {
                return false;
            }
        }
        if let Some(m) = &self.method {
            if !m.0.contains(req.method()) {
                return false;
            }
        }

        true
        // if self.method.is_some()
        //     && !self
        //         .method
        //         .as_ref()
        //         .unwrap()
        //         .eq_ignore_ascii_case(method.as_str())
        // {
        //     return false;
        // }
        // if let Some(_) = path.find(&self.rule) {
        //     return true;
        // } else {
        //     false
        // }
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
