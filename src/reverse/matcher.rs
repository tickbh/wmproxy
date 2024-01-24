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
    marker::PhantomData,
    str::FromStr,
};

use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use serde_with::{serde_as, DisplayFromStr};
use webparse::{Method, Scheme, WebError};

use crate::IpSets;

#[derive(Debug, Clone)]
pub struct MatchMethod(pub HashSet<Method>);
#[derive(Debug, Clone)]
pub struct MatchScheme(pub HashSet<Scheme>);

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
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
