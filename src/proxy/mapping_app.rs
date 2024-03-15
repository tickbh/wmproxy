use std::{
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use async_trait::async_trait;

use crate::{
    core::{AppTrait, Listeners, Service, ShutdownWatch, Stream, WrapListener},
    proxy::data::ProxyData,
    CenterServer, CenterTrans, ProxyConfig, ProxyResult,
};

pub struct MappingApp {
    config: ProxyConfig,
}

impl MappingApp {
    pub fn new(config: ProxyConfig) -> Self {
        Self { config }
    }

    pub fn build_services(config: ProxyConfig) -> ProxyResult<Service<Self>> {
        let app = Self::new(config);
        let mut listeners = Listeners::new();
        if let Some(http) = &app.config.map_http_bind {
            let mut wrap = WrapListener::new(http).expect("ok");
            wrap.set_desc("http");
            listeners.add(wrap);
        }
        if let Some(http) = &app.config.map_https_bind {
            let mut wrap = WrapListener::new(http).expect("ok");
            let accepter = app.config.get_map_tls_accept()?;
            wrap.accepter = Some(crate::core::WrapTlsAccepter::with_accepter(accepter));
            wrap.set_desc("https");
            listeners.add(wrap);
        }
        if let Some(tcp) = &app.config.map_tcp_bind {
            let mut wrap = WrapListener::new(tcp).expect("ok");
            wrap.set_desc("tcp");
            listeners.add(wrap);
        }
        if let Some(proxy) = &app.config.map_proxy_bind {
            let mut wrap = WrapListener::new(proxy).expect("ok");
            wrap.set_desc("proxy");
            listeners.add(wrap);
        }
        Ok(Service::with_listeners(
            "mapping_app".to_string(),
            listeners,
            app,
        ))
    }
}

#[async_trait]
impl AppTrait for MappingApp {
    async fn process_new(
        self: &Arc<Self>,
        session: Stream,
        _shutdown: &ShutdownWatch,
    ) -> Option<Stream> {
        let mutex = ProxyData::get_servers();
        let servers = mutex.lock().unwrap();
        for server in &*servers {
            match session.desc() {
                "http" | "https" => {
                    let addr = session.client_addr();
                    let _ = server.server_new_http(session, addr);
                    break;
                }
                "tcp" => {
                    let _ = server.server_new_tcp(session);
                    break;
                }
                "proxy" => {
                    let _ = server.server_new_proxy(session);
                    break;
                }
                _ => {
                    break;
                }
            }
        }

        log::warn!("未发现任何tcp服务器，但收到tcp的内网穿透，请检查配置");
        None
    }

    async fn ready_init(&mut self) -> io::Result<()> {
        Ok(())
    }
}
