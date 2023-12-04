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
// Created Date: 2023/11/28 10:14:47

use lazy_static::lazy_static;
use std::collections::HashMap;
use std::ops::Sub;
use std::time::{Duration, Instant};
use std::{borrow::Borrow, sync::RwLock};
use wenmeng::{ProtError, ProtResult};

lazy_static! {
    // 静态全局请求限制
    static ref GLOBAL_LIMIT_REQ: RwLock<HashMap<&'static str, LimitReqData>> =
        RwLock::new(HashMap::new());
}

pub struct LimitReqData {
    /// 记录所有的ip数据的限制情况
    ips: HashMap<String, InnerLimit>,
    /// IP个数
    limit: u64,
    /// 周期内可以通行的数据
    nums: u64,
    /// 每个周期的时间
    per: Duration,

    /// 最后清理IP的时间
    last_remove: Instant,
}

#[derive(Debug)]
pub enum LimitResult {
    Ok,
    Refuse,
    Delay(Duration),
}

struct InnerLimit {
    last: Instant,
    nums: u64,
}

impl InnerLimit {
    pub fn new() -> Self {
        Self {
            last: Instant::now(),
            nums: 1,
        }
    }

    pub fn recv_req(&mut self, per: &Duration) -> u64 {
        let now = Instant::now();
        if &now.sub(self.last) > per {
            self.nums = 1;
        } else {
            self.nums += 1;
        }
        self.last = now;
        self.nums
    }
}

impl LimitReqData {
    pub fn new(limit: u64, nums: u64, per: Duration) -> Self {
        Self {
            ips: HashMap::new(),
            limit,
            nums,
            per,
            last_remove: Instant::now(),
        }
    }

    pub fn try_remove_unuse(&mut self) {
        // 未超过限制数
        if self.ips.len() < self.limit as usize / 10 {
            return;
        }

        let now = Instant::now();

        // 未超过当前时间轮回的100倍
        if now.sub(self.last_remove) < 100 * self.per {
            return;
        }

        self.last_remove = now;

        let mut remove_keys = vec![];
        for (key, value) in &self.ips {
            if now.sub(value.last) > 50 * self.per {
                remove_keys.push(key.clone());
            }
        }

        for key in remove_keys {
            self.ips.remove(&key);
        }
    }

    pub fn inner_recv_new_req(&mut self, ip: &String, burst: u64) -> ProtResult<LimitResult> {
        self.try_remove_unuse();
        if self.ips.len() >= self.limit as usize {
            return Ok(LimitResult::Refuse);
        }
        if self.ips.contains_key(ip) {
            let nums = self.ips.get_mut(ip).unwrap().recv_req(&self.per);
            if nums <= self.nums {
                return Ok(LimitResult::Ok);
            } else if nums <= self.nums + burst {
                return Ok(LimitResult::Delay(self.per));
            } else {
                return Ok(LimitResult::Refuse);
            }
        } else {
            self.ips.insert(ip.clone(), InnerLimit::new());
        }
        Ok(LimitResult::Ok)
    }

    pub fn cache(key: String, limit: u64, nums: u64, per: Duration) -> ProtResult<()> {
        let mut write = GLOBAL_LIMIT_REQ
            .write()
            .map_err(|_| ProtError::Extension("unlock error"))?;
        if write.contains_key(&*key) {
            return Ok(());
        }
        write.insert(Box::leak(key.into_boxed_str()), Self::new(limit, nums, per));
        Ok(())
    }

    pub fn recv_new_req(key: &str, ip: &String, burst: u64) -> ProtResult<LimitResult> {
        let mut write = GLOBAL_LIMIT_REQ
            .write()
            .map_err(|_| ProtError::Extension("unlock error"))?;
        if !write.contains_key(&*key) {
            return Ok(LimitResult::Ok);
        }
        write.get_mut(key).unwrap().inner_recv_new_req(ip, burst)
    }
}
