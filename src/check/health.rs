use std::{
    collections::HashMap,
    io,
    net::{SocketAddr, ToSocketAddrs},
    sync::RwLock,
    time::{Duration, Instant},
};

use lazy_static::lazy_static;
use tokio::net::TcpStream;

lazy_static! {
    static ref HEALTH_CHECK: RwLock<HealthCheck> = RwLock::new(HealthCheck::new(60, 3, 2));
}

struct HealthRecord {
    last_record: Instant,
    fail_timeout: Duration,
    fall_times: usize,
    rise_times: usize,
    status: u8,
}

impl HealthRecord {
    pub fn new() -> Self {
        Self {
            last_record: Instant::now(),
            fail_timeout: Duration::new(60, 0),
            fall_times: 0,
            rise_times: 0,
            status: 0,
        }
    }

    pub fn clear_status(&mut self) {
        self.fall_times = 0;
        self.rise_times = 0;
        self.status = 0;
    }
}

/// 健康检查的控制中心
pub struct HealthCheck {
    /// 健康检查的重置时间, 失败超过该时间会重新检查, 统一单位秒
    fail_timeout: usize,
    /// 最大失败次数, 一定时间内超过该次数认为不可访问
    max_fails: usize,
    /// 最小上线次数, 到达这个次数被认为存活
    min_rises: usize,
    /// 记录跟地址相关的信息
    health_map: HashMap<SocketAddr, HealthRecord>,
}

impl HealthCheck {
    pub fn new(fail_timeout: usize, max_fails: usize, min_rises: usize) -> Self {
        Self {
            fail_timeout,
            max_fails,
            min_rises,
            health_map: HashMap::new(),
        }
    }

    pub fn instance() -> &'static RwLock<HealthCheck> {
        &HEALTH_CHECK
    }

    /// 检测状态是否能连接
    pub fn is_falldown(addr: &SocketAddr) -> bool {
        if let Ok(h) = HEALTH_CHECK.read() {
            if !h.health_map.contains_key(addr) {
                return false;
            }
            let value = h.health_map.get(&addr).unwrap();
            if Instant::now().duration_since(value.last_record) > value.fail_timeout {
                return false;
            }
            h.health_map[addr].status != 0
        } else {
            false
        }
    }

    /// 失败时调用
    pub fn add_falldown(addr: SocketAddr) {
        if let Ok(mut h) = HEALTH_CHECK.write() {
            if !h.health_map.contains_key(&addr) {
                let mut health = HealthRecord::new();
                health.fall_times = 1;
                h.health_map.insert(addr, health);
            } else {
                let max_fails = h.max_fails;
                let value = h.health_map.get_mut(&addr).unwrap();
                if Instant::now().duration_since(value.last_record) > value.fail_timeout {
                    value.clear_status();
                }
                value.last_record = Instant::now();
                value.fall_times += 1;

                if value.fall_times >= max_fails {
                    value.status = 1;
                }
            }
        }
    }

    /// 成功时调用
    pub fn add_riseup(addr: SocketAddr) {
        if let Ok(mut h) = HEALTH_CHECK.write() {
            if !h.health_map.contains_key(&addr) {
                let mut health = HealthRecord::new();
                health.rise_times = 1;
                h.health_map.insert(addr, health);
            } else {
                let min_rises = h.min_rises;
                let value = h.health_map.get_mut(&addr).unwrap();
                if Instant::now().duration_since(value.last_record) > value.fail_timeout {
                    value.clear_status();
                }
                value.last_record = Instant::now();
                value.rise_times += 1;

                if value.rise_times >= min_rises {
                    value.status = 0;
                }
            }
        }
    }

    pub async fn connect<A>(addr: &A) -> io::Result<TcpStream>
    where
        A: ToSocketAddrs,
    {
        let addrs = addr.to_socket_addrs()?;
        let mut last_err = None;

        for addr in addrs {
            if Self::is_falldown(&addr) {
                last_err = Some(io::Error::new(io::ErrorKind::Other, "health check falldown"));
            } else {
                match TcpStream::connect(&addr).await {
                    Ok(stream) => 
                    {
                        Self::add_riseup(addr);
                        return Ok(stream)
                    },
                    Err(e) => {
                        Self::add_falldown(addr);
                        last_err = Some(e)
                    },
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any address",
            )
        }))
    }
}
