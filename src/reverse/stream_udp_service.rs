use std::{io, net::SocketAddr};

use async_trait::async_trait;
use futures::{future::select_all, FutureExt, StreamExt};

use crate::{core::{ServiceTrait, ShutdownWatch}, ProxyResult};

use super::{StreamConfig, StreamUdp};



pub struct StreamUdpService {
    stream: StreamConfig,
    stream_udp_listeners: Vec<StreamUdp>,
}

impl StreamUdpService {

    pub fn build_services(stream: StreamConfig) -> ProxyResult<Self> {
        Ok(Self {
            stream,
            stream_udp_listeners: vec![],
        })
    }

    pub fn is_valid(&self) -> bool {
        self.stream_udp_listeners.len() > 0
    }
    
    async fn multi_udp_listen_work(
        listens: &mut Vec<StreamUdp>,
    ) -> (io::Result<(Vec<u8>, SocketAddr)>, usize) {
        if !listens.is_empty() {
            let (data, index, _) =
                select_all(listens.iter_mut().map(|listener| listener.next().boxed())).await;
            if data.is_none() {
                return (
                    Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "read none data",
                    )),
                    index,
                );
            }
            (data.unwrap(), index)
        } else {
            let pend = std::future::pending();
            let () = pend.await;
            unreachable!()
        }
    }
}

#[async_trait]
impl ServiceTrait for StreamUdpService {
    async fn ready_service(&mut self) -> io::Result<()> {
        let stream_udp_listeners = self.stream.bind_udp_app().map_err(|e| io::Error::new(io::ErrorKind::Other, "bind udp error"))?;
        self.stream_udp_listeners = stream_udp_listeners;
        Ok(())
    }
    async fn start_service(&mut self, mut _shutdown: ShutdownWatch) {
        tokio::select! {
            (result, index) = Self::multi_udp_listen_work(&mut self.stream_udp_listeners) => {
                if let Ok((data, addr)) = result {
                    log::trace!("反向代理:{}收到客户端连接: {}->{:?}", "stream", addr, self.stream_udp_listeners[index].local_addr());

                    let udp = &mut self.stream_udp_listeners[index];
                    if let Err(e) = udp.process_data(data, addr).await {
                        log::info!("udp负载均衡的时候发生错误:{:?}", e);
                    }
                }
            }
        }

    }
    fn name(&self) -> &str {
        return "stream_udp_service";
    }
}