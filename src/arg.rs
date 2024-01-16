// Copyright 2022 - 2024 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Author: tickbh
// -----
// Created Date: 2024/01/16 10:59:37

// use std::net::SocketAddr;

use std::{net::{SocketAddr, SocketAddrV4, Ipv4Addr, IpAddr}, path::PathBuf, fs::File, io::{Read, self}};

use bpaf::*;
use log::LevelFilter;

use crate::{ConfigOption, ProxyConfig, option::proxy_config, ProxyResult};

#[derive(Debug, Clone, Bpaf)]
#[allow(dead_code)]
struct Shared {
    /// 输入控制台的监听地址
    #[bpaf(fallback(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8837)), display_fallback)]
    pub(crate) control: SocketAddr,
    /// 禁用默认输出
    pub(crate) disable_stdout: bool,
    /// 禁用控制微端
    pub(crate) disable_control: bool,
    /// 是否显示更多日志
    pub(crate) verbose: bool,
    /// 设置默认等级
    pub(crate) default_level: Option<LevelFilter>,
}

#[derive(Debug, Clone, Bpaf)]
#[allow(dead_code)]
struct Config {
    /// 配置文件路径
    #[bpaf(short, long)]
    pub(crate) config: String,
}

#[derive(Debug, Clone)]
pub enum Command {
    Proxy(ProxyConfig),
    Config(Config),
}


fn parse_command() -> impl Parser<(Command, Shared)> {
    
    let action = proxy_config().map(Command::Proxy);
    let action = construct!(action, shared()).to_options().command("proxy");
    let config = config().map(Command::Config);
    let config = construct!(config, shared()).to_options().command("config");
    construct!([action, config])
}

pub fn parse_env() -> ProxyResult<ConfigOption> {
    let (command, shared) = parse_command().run();
    match command {
        Command::Proxy(proxy) => {
            let mut option = ConfigOption::default();
            option.default_level = shared.default_level;
            option.disable_control = shared.disable_control;
            option.disable_stdout = shared.disable_stdout;
            option.control = shared.control;
            option.proxy = Some(proxy);
            if shared.verbose {
                option.default_level = Some(LevelFilter::Trace);
            }
            option.after_load_option()?;
            return Ok(option);
        }
        Command::Config(config) => {
            let path = PathBuf::from(&config.config);
            let mut file = File::open(config.config)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            let extension = path.extension().unwrap().to_string_lossy().to_string();
            let mut option = match &*extension {
                "yaml" => serde_yaml::from_str::<ConfigOption>(&contents).map_err(|e| {
                    println!("parse error msg = {:?}", e);
                    io::Error::new(io::ErrorKind::Other, "parse yaml error")
                })?,
                "toml" => toml::from_str::<ConfigOption>(&contents).map_err(|e| {
                    println!("parse error msg = {:?}", e);
                    io::Error::new(io::ErrorKind::Other, "parse toml error")
                })?,
                _ => {
                    let e = io::Error::new(io::ErrorKind::Other, "unknow format error");
                    return Err(e.into());
                }
            };
            if shared.verbose {
                option.default_level = Some(LevelFilter::Trace);
            }
            option.after_load_option().unwrap();
            return Ok(option);
        }
    }
}

