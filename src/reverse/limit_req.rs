use std::{fmt::Display, str::FromStr, time::Duration};

use wenmeng::{HeaderHelper, Middleware};

use async_trait::async_trait;

use wenmeng::{ProtError, ProtResult, RecvRequest, RecvResponse};

use crate::{ProxyError, ConfigSize, ConfigDuration};

#[derive(Debug, Clone)]
pub struct LimitReqZone {
    /// 键值的匹配方式
    key: String,
    /// IP个数
    limit: u64,
    /// 周期内可以通行的数据
    nums: u64,
    /// 每个周期的时间
    per: Duration,
}

impl LimitReqZone {
    pub fn new(key: String, limit: u64, nums: u64, per: Duration) -> Self {
        Self {
            key,
            limit,
            nums,
            per,
        }
    }
}

pub struct LimitReq {
    req: Option<LimitReqZone>,
    zone: String,
    burst: usize,
}

pub struct LimitReqMiddleware {
    req: LimitReq,
}

#[async_trait]
impl Middleware for LimitReqMiddleware {
    async fn process_request(&mut self, request: &mut RecvRequest) -> ProtResult<()> {
        Ok(())
    }
    async fn process_response(
        &mut self,
        _request: &mut RecvRequest,
        response: &mut RecvResponse,
    ) -> ProtResult<()> {
        Ok(())
    }
}

impl Display for LimitReqZone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{} limit={} rate={}r/{}", self.key, ConfigSize::new(self.limit), self.nums, ConfigDuration::new(self.per)))
    }
}

impl FromStr for LimitReqZone {
    type Err = ProxyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v = s.split(" ").collect::<Vec<&str>>();
        let key = v[0].to_string();
        let mut limit = 0;
        let mut nums = 0;
        let mut per = Duration::new(0, 0);
        for idx in 1..v.len() {
            let key_value = v[idx].split("=").map(|k| k.trim()).collect::<Vec<&str>>();
            if key_value.len() == 0 {
                return Err(ProxyError::Extension("未知的LimitReq"));
            }
            match key_value[0] {
                "limit" => {
                    let s = ConfigSize::from_str(key_value[1])?;
                    limit = s.0;
                }
                "rate" => {
                    let rate_key = key_value[1].split("/").map(|k| k.trim()).collect::<Vec<&str>>();
                    if rate_key.len() == 1 {
                        return Err(ProxyError::Extension("未知的LimitReq"));
                    }

                    let rate = rate_key[0].trim_end_matches("r");
                    nums = rate.parse::<u64>().map_err(|_e| ProxyError::Extension("parse error"))?;
                    
                    let s = ConfigDuration::from_str(format!("1{}", rate_key[1]).as_str())?;
                    per = s.0;
                }
                _ => {
                    return Err(ProxyError::Extension("未知的LimitReq"));
                }
            }
        }

        Ok(LimitReqZone::new(key, limit, nums, per))
    }
}
