

use std::{sync::Weak, net::SocketAddr, time::Duration};

use rand::Rng;
use serde::{Serialize, Deserialize};
use tokio::io::{AsyncWrite, AsyncRead};
use webparse::{Request, Response, Url, HeaderName};
use wenmeng::{FileServer, RecvStream, ProtResult, ProtError, Client, HeaderHelper};

use crate::{ProxyResult, HealthCheck};

use super::ServerConfig;

fn default_headers() -> Vec<Vec<String>> {
    vec![]
}


fn default_weight() -> u16 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleStreamConfig {
    /// 访问地址
    pub addr: SocketAddr,
    
    /// 权重
    #[serde(default = "default_weight")]
    pub weight: u16,
    // /// 失败的恢复时间
    // fail_timeout: Duration,
    // /// 当前连续失败的次数
    // fall_times: usize,
    // /// 当前连续成功的次数
    // rise_times: usize,

    pub status: Option<String>,
    // #[serde(skip, default = "default_null")]
    // pub weak_server: *const ServerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamConfig {
    pub name: String,
    #[serde(default = "Vec::new")]
    pub server: Vec<SingleStreamConfig>,
    // #[serde(skip, default = "default_null")]
    // pub weak_server: *const ServerConfig,
}

impl UpstreamConfig {
    pub fn get_server_addr(&self) -> ProtResult<SocketAddr> {
        if self.server.is_empty() {
            return Err(ProtError::Extension("empty upstream"));
        }
        let (sum, sum_all) = self.calc_sum_weight();
        let mut rng = rand::thread_rng();
        if sum != 0 {
            let mut random_weight = rng.gen_range(0..sum);
            for server in &self.server {
                if !HealthCheck::is_fall_down(&server.addr) {
                    if random_weight <= server.weight {
                        return Ok(server.addr.clone());
                    }
                    random_weight -= server.weight;
                }
            }
        } else {
            let mut random_weight = rng.gen_range(0..sum_all);
            for server in &self.server {
                if random_weight <= server.weight {
                    return Ok(server.addr.clone());
                }
                random_weight -= server.weight;
            }
        }
        return Err(ProtError::Extension("empty upstream"));
        
    }

    pub fn calc_sum_weight(&self) -> (u16, u16) {
        let mut sum = 0;
        let mut sum_all = 0;
        for server in &self.server {
            if !HealthCheck::is_fall_down(&server.addr) {
                sum += server.weight;
            }
            sum_all += server.weight;
        }
        return (sum, sum_all);
    }
}