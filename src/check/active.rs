use std::{
    net::SocketAddr,
    time::{Duration, Instant}, io,
};
use tokio::{sync::mpsc::{Receiver, error::TryRecvError}, net::TcpStream};
use webparse::{Request, Response};
use wenmeng::{Client, RecvStream};

use crate::{ProxyResult, HealthCheck, Proxy};

use super::health;

/// 单项健康检查
/// TODO HTTP检查应该可以配置请求方法及返回编码是否正确来判定是否为健康
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
    
    pub async fn connect_http(&self) -> ProxyResult<Response<RecvStream>> {
        let url = format!("{}://{}/", self.method, self.addr);
        let req = Request::builder().method("GET").url(url.clone()).body("").unwrap();

        let client = Client::builder()
            .connect(url).await?;
    
        let (mut recv, _sender) = client.send2(req.into_type()).await?;
        match recv.recv().await {
            Some(res) => {
                return Ok(res);
            }
            _ => {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "").into());
            }
        }
    }
    pub async fn do_check(&self) -> ProxyResult<()> {
        // 防止短时间内健康检查的连接过多, 做一定的超时处理, 或者等上一条消息处理完毕
        if !HealthCheck::check_can_request(&self.addr, Duration::from_micros(5)) {
            return Ok(())
        }
        if self.method.eq_ignore_ascii_case("http") {
            match self.connect_http().await {
                Ok(r) => {
                    if r.status().is_server_error() {
                        HealthCheck::add_fall_down(self.addr);
                    } else {
                        HealthCheck::add_rise_up(self.addr);
                    }
                }
                Err(_) => {
                    HealthCheck::add_fall_down(self.addr);
                }
            }
        } else {
            match TcpStream::connect(self.addr).await {
                Ok(_) => {
                    HealthCheck::add_rise_up(self.addr);
                }
                Err(_) => {
                    HealthCheck::add_fall_down(self.addr);
                }
            }
        }
        Ok(())
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
                        let _ = h.do_check().await;
                    });
                }
            }
            tokio::time::sleep(Duration::from_secs(if self.healths.is_empty() { 60 } else { 1 })).await;
        }
        Ok(())
    }

    pub fn do_start(mut self) -> ProxyResult<()> {
        tokio::spawn(async move {
            let _ = self.repeat_check().await;
        });
        Ok(())
    }
}
