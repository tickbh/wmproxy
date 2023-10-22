use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::{sync::mpsc::{Receiver, error::TryRecvError}, net::TcpStream};

use crate::{ProxyResult, HealthCheck};

use super::health;

#[derive(Debug, Clone)]
pub struct OneHealth {
    pub addr: SocketAddr,
    pub method: String,
    pub interval: Duration,
    pub last_record: Instant,
}

impl OneHealth {

    pub fn new(addr: SocketAddr, method: String, interval: Duration) -> Self {
        OneHealth { addr, method, interval, last_record: Instant::now() - interval }
    }
    pub async fn do_check(&self) {

        match TcpStream::connect(self.addr).await {
            Ok(_) => {
                HealthCheck::add_rise_up(self.addr);
            }
            Err(_) => {
                HealthCheck::add_fall_down(self.addr);
            }
        }
        // if self.method.eq_ignore_ascii_case("http") {

        // } else {
        // }
    }
}

/// 主动式健康检查
pub struct ActiveHealth {
    pub healths: Vec<OneHealth>,
    pub receiver: Receiver<Vec<OneHealth>>,
}

impl ActiveHealth {
    pub fn new(healths: Vec<OneHealth>, receiver: Receiver<Vec<OneHealth>>) -> Self {
        Self { healths, receiver }
    }

    pub async fn repeat_check(&mut self) -> ProxyResult<()> {
        loop {
            let recv = self.receiver.try_recv();
            match recv {
                Ok(value) => {
                    self.healths = value;
                }
                Err(TryRecvError::Disconnected) => {
                    break;
                }
                _ => {}
            }
            let now = Instant::now();
            for health in &mut self.healths {
                if now.duration_since(health.last_record) > health.interval {
                    health.last_record = now;
                    let h = health.clone();
                    tokio::spawn(async move {
                        let _ = h.do_check();
                    });
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }

    pub fn do_start(mut self) -> ProxyResult<()> {
        tokio::spawn(async move {
            let _ = self.repeat_check();
        });
        Ok(())
    }
}
