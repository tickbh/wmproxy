use std::{collections::HashMap, net::SocketAddr, sync::RwLock, time::Instant};

use lazy_static::lazy_static;

lazy_static! {
    static ref HEALTH_CHECK: RwLock<HealthCheck> = RwLock::new(HealthCheck::new(60, 3));
}

struct HealthRecord {
    last_record: Instant,
    fall_times: usize,
    rise_times: usize,
    status: u8,
}

impl HealthRecord {
    pub fn new() -> Self {
        Self {
            last_record: Instant::now(),
            fall_times: 0,
            rise_times: 0,
            status: 0,
        }
    }
}

pub struct HealthCheck {
    /// 健康检查的重置时间, 失败超过该时间会重新检查, 统一单位秒
    fail_timeout: usize,
    /// 最大失败次数, 一定时间内超过该次数认为不可访问
    max_fails: usize,

    health_map: HashMap<SocketAddr, HealthRecord>,
}

impl HealthCheck {
    pub fn new(fail_timeout: usize, max_fails: usize) -> Self {
        Self {
            fail_timeout,
            max_fails,

            health_map: HashMap::new(),
        }
    }

    pub fn instance() -> &'static RwLock<HealthCheck> {
        &HEALTH_CHECK
    }

    pub fn is_falldown(addr: &SocketAddr) -> bool {
        if let Ok(h) = HEALTH_CHECK.read() {
            if !h.health_map.contains_key(addr) {
                return false;
            }
            h.health_map[addr].status != 1
        } else {
            false
        }
    }

    pub fn add_falldown(addr: SocketAddr) {
        if let Ok(mut h) = HEALTH_CHECK.write() {
            if !h.health_map.contains_key(&addr) {
                h.health_map.insert(addr, HealthRecord::new());
            } else {
                let max_fails = h.max_fails;
                let entry = h.health_map.get_mut(&addr).unwrap();
                entry.last_record = Instant::now();
                entry.fall_times += 1;

                if entry.fall_times >= max_fails {
                    entry.status = 1;
                }
            }
        }
    }
}
