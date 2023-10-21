use std::net::SocketAddr;

use wenmeng::{ProtResult, ProtError};

use super::UpstreamConfig;


pub struct ReverseHelper;

impl ReverseHelper {

    pub fn get_upstream_addr(upstream: &Vec<UpstreamConfig>, name: &str) -> ProtResult<SocketAddr> {
        for stream in upstream {
            if &stream.name == name {
                return stream.get_server_addr()
            }
        }
        return Err(ProtError::Extension(""));
    }
}