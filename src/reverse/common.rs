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
// Created Date: 2023/11/03 05:01:37

use std::collections::HashMap;

use crate::{ConfigDuration, ConfigLog, ConfigRate};
use crate::{ConfigSize, DisplayFromStrOrNumber};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use wenmeng::RateLimitLayer;
use wenmeng::TimeoutLayer;

use super::LimitReq;

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommonConfig {
    pub max_read_buf: Option<usize>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub rate_limit: Option<ConfigRate>,

    #[serde_as(as = "Option<DisplayFromStrOrNumber>")]
    pub client_read_timeout: Option<ConfigDuration>,
    #[serde_as(as = "Option<DisplayFromStrOrNumber>")]
    pub client_write_timeout: Option<ConfigDuration>,
    #[serde_as(as = "Option<DisplayFromStrOrNumber>")]
    pub client_timeout: Option<ConfigDuration>,
    #[serde_as(as = "Option<DisplayFromStrOrNumber>")]
    pub client_ka_timeout: Option<ConfigDuration>,

    #[serde_as(as = "Option<DisplayFromStrOrNumber>")]
    pub proxy_connect_timeout: Option<ConfigDuration>,
    #[serde_as(as = "Option<DisplayFromStrOrNumber>")]
    pub proxy_read_timeout: Option<ConfigDuration>,
    #[serde_as(as = "Option<DisplayFromStrOrNumber>")]
    pub proxy_write_timeout: Option<ConfigDuration>,
    #[serde_as(as = "Option<DisplayFromStrOrNumber>")]
    pub proxy_timeout: Option<ConfigDuration>,

    #[serde(default = "HashMap::new")]
    pub log_format: HashMap<String, String>,
    #[serde(default = "HashMap::new")]
    pub log_names: HashMap<String, String>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub access_log: Option<ConfigLog>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub error_log: Option<ConfigLog>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub limit_req: Option<LimitReq>,
}

impl CommonConfig {
    pub fn new() -> Self {
        Self {
            max_read_buf: None,
            rate_limit: None,
            
            client_read_timeout: None,
            client_write_timeout: None,
            client_timeout: None,
            client_ka_timeout: None,

            proxy_connect_timeout: None,
            proxy_timeout: None,
            proxy_read_timeout: None,
            proxy_write_timeout: None,

            log_format: HashMap::new(),
            log_names: HashMap::new(),

            access_log: None,
            error_log: None,

            limit_req: None,
        }
    }

    /// 将配置参数提前共享给子级
    pub fn copy_from_parent(&mut self, parent: &CommonConfig) {
        if self.max_read_buf.is_none() && parent.max_read_buf.is_some() {
            self.max_read_buf = parent.max_read_buf.clone();
        }
        if self.rate_limit.is_none() && parent.rate_limit.is_some() {
            self.rate_limit = parent.rate_limit.clone();
        }
        if self.client_read_timeout.is_none() && parent.client_read_timeout.is_some() {
            self.client_read_timeout = parent.client_read_timeout.clone();
        }
        if self.client_write_timeout.is_none() && parent.client_write_timeout.is_some() {
            self.client_write_timeout = parent.client_write_timeout.clone();
        }
        if self.proxy_connect_timeout.is_none() && parent.proxy_connect_timeout.is_some() {
            self.proxy_connect_timeout = parent.proxy_connect_timeout.clone();
        }
        if self.client_timeout.is_none() && parent.client_timeout.is_some() {
            self.client_timeout = parent.client_timeout.clone();
        }
        for h in &parent.log_names {
            self.log_names.insert(h.0.clone(), h.1.clone());
        }
        for h in &parent.log_format {
            self.log_format.insert(h.0.clone(), h.1.clone());
        }
        if self.access_log.is_none() {
            self.access_log = parent.access_log.clone();
        }
        if self.error_log.is_none() {
            self.error_log = parent.error_log.clone();
        }
        if self.limit_req.is_none() {
            self.limit_req = parent.limit_req.clone();
        }
    }

    pub fn pre_deal(&mut self) {
        if let Some(err) = &mut self.error_log {
            err.as_error();
        }
    }

    pub fn get_rate_limit(&self) -> Option<RateLimitLayer> {
        if self.rate_limit.is_some() {
            return Some(RateLimitLayer::new(self.rate_limit.clone().unwrap().0));
        } else {
            None
        }
    }
    
    pub fn build_proxy_timeout(&self) -> Option<TimeoutLayer> {
        let mut timeout = TimeoutLayer::new();
        let mut has_data = false;

        if let Some(connect) = &self.proxy_connect_timeout {
            timeout.set_connect_timeout(Some(connect.0.clone()));
            has_data = true;
        }

        if let Some(read) = &self.proxy_read_timeout {
            timeout.set_read_timeout(Some(read.0.clone()));
            has_data = true;
        }

        if let Some(write) = &self.proxy_write_timeout {
            timeout.set_write_timeout(Some(write.0.clone()));
            has_data = true;
        }

        if let Some(t) = &self.proxy_timeout {
            timeout.set_timeout(Some(t.0.clone()));
            has_data = true;
        }

        if has_data {
            Some(timeout)
        } else {
            None
        }
    }

    pub fn build_client_timeout(&self) -> Option<TimeoutLayer> {
        let mut timeout = TimeoutLayer::new();
        let mut has_data = false;

        if let Some(read) = &self.client_read_timeout {
            timeout.set_read_timeout(Some(read.0.clone()));
            has_data = true;
        }

        if let Some(write) = &self.client_write_timeout {
            timeout.set_write_timeout(Some(write.0.clone()));
            has_data = true;
        }

        if let Some(t) = &self.client_timeout {
            timeout.set_timeout(Some(t.0.clone()));
            has_data = true;
        }

        if let Some(ka) = &self.client_ka_timeout {
            timeout.set_ka_timeout(Some(ka.0.clone()));
            has_data = true;
        }

        if has_data {
            Some(timeout)
        } else {
            None
        }
    }
    
    pub fn get_log_names(&self, names: &mut HashMap<String, String>)  {
        for val in &self.log_names         {
            if !names.contains_key(val.0) {
                names.insert(val.0.clone(), val.1.clone());
            }
        }
    }

}
