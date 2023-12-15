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
// Created Date: 2023/10/23 09:44:07

use std::{
    net::SocketAddr,
    time::{Duration, Instant}, io,
};
use tokio::sync::mpsc::{Receiver, error::TryRecvError};
use webparse::{Request, Response};
use wenmeng::{Client, Body};

use crate::{ProxyResult, HealthCheck};

/// 单项健康检查
/// TODO HTTP检查应该可以配置请求方法及返回编码是否正确来判定是否为健康
#[derive(Debug, Clone)]
pub struct OneHealth {
    /// 主动检查地址
    pub addr: SocketAddr,
    /// 主动检查方法, 有http/https/tcp等
    pub method: String,
    /// 每次检查间隔
    pub interval: Duration,
    /// 最后一次记录时间
    pub last_record: Instant,
}

impl OneHealth {

    pub fn new(addr: SocketAddr, method: String, interval: Duration) -> Self {
        OneHealth { addr, method, interval, last_record: Instant::now() - interval }
    }
    
    pub async fn connect_http(&self) -> ProxyResult<Response<Body>> {
        let url = format!("{}://{}/", self.method, self.addr);
        let req = Request::builder().method("GET").url(url.clone()).body("").unwrap();

        let client = Client::builder()
            .connect(url).await?;
    
        let (mut recv, _sender) = client.send2(req.into_type()).await?;
        match recv.recv().await {
            Some(res) => {
                return Ok(res?);
            }
            _ => {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "").into());
            }
        }
    }
    pub async fn do_check(&self) -> ProxyResult<()> {
        // 防止短时间内健康检查的连接过多, 做一定的超时处理, 或者等上一条消息处理完毕
        if !HealthCheck::check_can_request(&self.addr, self.interval) {
            return Ok(())
        }
        if self.method.eq_ignore_ascii_case("http") {
            match tokio::time::timeout(self.interval + Duration::from_secs(3), self.connect_http()).await {
                Ok(r) => match r {
                    Ok(r) => {
                        if r.status().is_server_error() {
                            log::trace!("主动健康检查:HTTP:{}, 返回失败:{}", self.addr, r.status());
                            HealthCheck::add_fall_down(self.addr);
                        } else {
                            HealthCheck::add_rise_up(self.addr);
                        }
                    }
                    Err(e) => {
                        log::trace!("主动健康检查:HTTP:{}, 发生错误:{:?}", self.addr, e);
                        HealthCheck::add_fall_down(self.addr);
                    }
                },
                Err(e) => {
                    log::trace!("主动健康检查:HTTP:{}, 发生超时:{:?}", self.addr, e);
                    HealthCheck::add_fall_down(self.addr);
                },
            }
        } else {
            match tokio::time::timeout(Duration::from_secs(3), self.connect_http()).await {
                Ok(r) => {
                    match r {
                        Ok(_) => {
                            HealthCheck::add_rise_up(self.addr);
                        }
                        Err(e) => {
                            log::trace!("主动健康检查:TCP:{}, 发生错误:{:?}", self.addr, e);
                            HealthCheck::add_fall_down(self.addr);
                        }
                    }
                }
                Err(e) => {
                    log::trace!("主动健康检查:TCP:{}, 发生超时:{:?}", self.addr, e);
                    HealthCheck::add_fall_down(self.addr);
                }
            }
        }
        Ok(())
    }
}

/// 主动式健康检查
pub struct ActiveHealth {
    /// 所有的健康列表
    pub healths: Vec<OneHealth>,
    /// 接收健康列表，当配置变更时重新载入
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
