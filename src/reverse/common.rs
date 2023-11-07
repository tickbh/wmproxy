use crate::ConfigDuration;
use crate::ConfigSize;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use wenmeng::RateLimitLayer;
use wenmeng::TimeoutLayer;

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommonConfig {
    pub max_read_buf: Option<usize>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub rate_limit_nums: Option<ConfigSize>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub rate_limit_per: Option<ConfigDuration>,

    #[serde_as(as = "Option<DisplayFromStr>")]
    pub read_timeout: Option<ConfigDuration>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub write_timeout: Option<ConfigDuration>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub connect_timeout: Option<ConfigDuration>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub timeout: Option<ConfigDuration>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub ka_timeout: Option<ConfigDuration>,

}

impl CommonConfig {
    pub fn new() -> Self {
        Self {
            max_read_buf: None,
            rate_limit_nums: None,
            rate_limit_per: None,
            
            read_timeout: None,
            write_timeout: None,
            connect_timeout: None,
            timeout: None,
            ka_timeout: None,
        }
    }

    /// 将配置参数提前共享给子级
    pub fn copy_from_parent(&mut self, parent: &CommonConfig) {
        if self.max_read_buf.is_none() && parent.max_read_buf.is_some() {
            self.max_read_buf = parent.max_read_buf.clone();
        }
        if self.rate_limit_nums.is_none() && parent.rate_limit_nums.is_some() {
            self.rate_limit_nums = parent.rate_limit_nums.clone();
        }
        if self.rate_limit_per.is_none() && parent.rate_limit_per.is_some() {
            self.rate_limit_per = parent.rate_limit_per.clone();
        }
        if self.read_timeout.is_none() && parent.read_timeout.is_some() {
            self.read_timeout = parent.read_timeout.clone();
        }
        if self.write_timeout.is_none() && parent.write_timeout.is_some() {
            self.write_timeout = parent.write_timeout.clone();
        }
        if self.connect_timeout.is_none() && parent.connect_timeout.is_some() {
            self.connect_timeout = parent.connect_timeout.clone();
        }
        if self.timeout.is_none() && parent.timeout.is_some() {
            self.timeout = parent.timeout.clone();
        }
    }

    pub fn get_rate_limit(&self) -> Option<RateLimitLayer> {
        if self.rate_limit_nums.is_some() && self.rate_limit_per.is_some() {
            return Some(RateLimitLayer::new(self.rate_limit_nums.clone().unwrap().0, self.rate_limit_per.clone().unwrap().into()));
        } else {
            None
        }
    }

    pub fn build_timeout(&self) -> Option<TimeoutLayer> {
        let mut timeout = TimeoutLayer::new();
        let mut has_data = false;

        if let Some(connect) = &self.connect_timeout {
            timeout.set_connect_timeout(Some(connect.0.clone()));
            has_data = true;
        }
        if let Some(read) = &self.read_timeout {
            timeout.set_read_timeout(Some(read.0.clone()));
            has_data = true;
        }

        if let Some(write) = &self.write_timeout {
            timeout.set_write_timeout(Some(write.0.clone()));
            has_data = true;
        }

        if let Some(t) = &self.timeout {
            timeout.set_timeout(Some(t.0.clone()));
            has_data = true;
        }

        if let Some(ka) = &self.ka_timeout {
            timeout.set_ka_timeout(Some(ka.0.clone()));
            has_data = true;
        }

        if has_data {
            Some(timeout)
        } else {
            None
        }
    }
}
