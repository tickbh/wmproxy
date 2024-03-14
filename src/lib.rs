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
// Created Date: 2023/09/15 11:37:02



mod flag;
mod wmcore;
mod proxy;
mod option;
mod error;
mod streams;
mod prot;
mod helper;
mod trans;
mod mapping;
mod check;
mod reverse;
mod control;
mod config;
mod plugins;
pub mod log;
mod data;
pub mod arg;
pub mod core;

pub use error::{ProxyResult, ProxyError};
pub use flag::Flag;
pub use option::{ProxyConfig, Builder, ConfigOption};
pub use wmcore::WMCore;
pub use proxy::http::ProxyHttp;
pub use proxy::socks5::ProxySocks5;
pub use proxy::ProxyApp;
pub use streams::*;
pub use helper::Helper;
pub use prot::{ProtFrame, ProtFrameHeader, ProtClose, ProtData, ProtCreate};
pub use mapping::*;
pub use check::*;
pub use control::*;
pub use config::*;
pub use plugins::*;

const INVALID_SOCKET_ADDR: std::net::SocketAddr = std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 9)), 8080);