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

use std::{
    fs::File,
    io::{self, Read},
    net::{IpAddr, Ipv4Addr, SocketAddr, AddrParseError},
    path::PathBuf,
    str::FromStr, fmt::Display,
};

use bpaf::*;
use log::LevelFilter;

use crate::{
    option::proxy_config,
    reverse::{HttpConfig, LocationConfig, ServerConfig},
    ConfigOption, FileServer, ProxyConfig, ProxyResult,
};

#[derive(Debug, Clone, Copy)]
pub struct WrapAddr(pub SocketAddr);

impl FromStr for WrapAddr {
    type Err = AddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with(":") {
            let addr = format!("127.0.0.1{s}").parse::<SocketAddr>()?;
            Ok(WrapAddr(addr))
        } else {
            let addr = s.parse::<SocketAddr>()?;
            Ok(WrapAddr(addr))
        }
    }
}

impl Display for WrapAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

#[derive(Debug, Clone, Bpaf)]
#[allow(dead_code)]
struct Shared {
    /// 输入控制台的监听地址
    #[bpaf(
        fallback(WrapAddr(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8837))),
        display_fallback
    )]
    pub(crate) control: WrapAddr,
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

#[derive(Debug, Clone, Bpaf)]
#[allow(dead_code)]
struct FileServerConfig {
    /// 根目录路径
    #[bpaf(short, long, fallback(String::new()))]
    pub(crate) root: String,
    #[bpaf(
        short,
        long,
        fallback(WrapAddr(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 80))),
        display_fallback
    )]
    pub(crate) listen: WrapAddr,
    #[bpaf(short, long)]
    pub(crate) domain: Option<String>,
    #[bpaf(short, long)]
    pub(crate) browse: bool,
}

#[derive(Debug, Clone)]
enum Command {
    Proxy(ProxyConfig),
    Config(Config),
    FileServer(FileServerConfig),
}

fn parse_command() -> impl Parser<(Command, Shared)> {
    let action = proxy_config().map(Command::Proxy);
    let action = construct!(action, shared()).to_options().command("proxy");
    let config = config().map(Command::Config);
    let config = construct!(config, shared()).to_options().command("config");
    let file_config = file_server_config().map(Command::FileServer);
    let file_config = construct!(file_config, shared())
        .to_options()
        .command("file-server");
    construct!([action, config, file_config])
}

pub fn parse_env() -> ProxyResult<ConfigOption> {
    let (command, shared) = parse_command().run();
    let mut option = ConfigOption::default();
    option.default_level = shared.default_level;
    option.disable_control = shared.disable_control;
    option.disable_stdout = shared.disable_stdout;
    option.control = shared.control.0;
    if shared.verbose {
        option.default_level = Some(LevelFilter::Trace);
    }
    match command {
        Command::Proxy(proxy) => {
            option.proxy = Some(proxy);
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
        Command::FileServer(file) => {
            let mut http = HttpConfig::new();
            let mut server = ServerConfig::new(file.listen.0);
            let mut location = LocationConfig::new();
            location.file_server = Some(FileServer::new(file.root, "".to_string()));
            server.location.push(location);
            http.server.push(server);
            option.http = Some(http);
            return Ok(option);
        }
    }
}
