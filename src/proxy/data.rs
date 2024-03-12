use std::{io, sync::Mutex};

use lazy_static::lazy_static;

use crate::CenterServer;

pub struct ProxyData;

lazy_static!{
    static ref CAHCE_CENTER_SERVERS: Mutex<Vec<CenterServer>> = Mutex::new(vec![]);
}
impl ProxyData {

    pub fn cache_server(server: CenterServer) {
        let mut centers = CAHCE_CENTER_SERVERS.lock().unwrap();
        centers.push(server);
    }

    pub fn clear_close_servers() {
        let mut centers = CAHCE_CENTER_SERVERS.lock().unwrap();
        centers.retain(|s| !s.is_close());
    }

    pub fn get_servers() -> &'static Mutex<Vec<CenterServer>> {
        Self::clear_close_servers();
        &*CAHCE_CENTER_SERVERS
    }
}