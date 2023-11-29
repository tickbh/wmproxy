use lazy_static::lazy_static;
use rbtree::RBTree;
use wenmeng::{ProtResult, ProtError};
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Sub;
use std::time::{Duration, Instant};
use std::{borrow::Borrow, sync::RwLock};


lazy_static! {
    static ref GLOABL_LIMIT_REQ: RwLock<HashMap<&'static str, LimitReqData>> =
        RwLock::new(HashMap::new());
}

pub struct LimitReqData {
    ips: HashMap<String, InnerLimit>,
    /// IP个数
    limit: u64,
    /// 周期内可以通行的数据
    nums: u64,
    /// 每个周期的时间
    per: Duration,

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

    pub fn inner_recv_new_req(&mut self, ip: &String, burst: u64) -> ProtResult<LimitResult> {
        if self.ips.contains_key(ip) {
            let nums = self.ips.get_mut(ip).unwrap().recv_req(&self.per);
            if nums <= self.nums {
                return Ok(LimitResult::Ok)
            } else if nums <= self.nums + burst {
                return Ok(LimitResult::Delay(self.per))
            } else {
                return Ok(LimitResult::Refuse)
            }
        } else {
            self.ips.insert(ip.clone(), InnerLimit::new());
        }
        Ok(LimitResult::Ok)
    }

    pub fn cache(key: String, limit: u64, nums: u64, per: Duration) -> ProtResult<()> {
        let mut write = GLOABL_LIMIT_REQ
            .write()
            .map_err(|_| ProtError::Extension("unlock error"))?;
        if write.contains_key(&*key) {
            return Ok(());
        }
        write.insert(Box::leak(key.into_boxed_str()), Self::new(limit, nums, per));
        Ok(())
    }

    pub fn recv_new_req(key: &str, ip: &String, burst: u64) -> ProtResult<LimitResult> {
        let mut write = GLOABL_LIMIT_REQ
            .write()
            .map_err(|_| ProtError::Extension("unlock error"))?;
        if !write.contains_key(&*key) {
            return Ok(LimitResult::Ok);
        }
        write.get_mut(key).unwrap().inner_recv_new_req(ip, burst)
    }
}
