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
// Created Date: 2023/10/18 02:31:52

use std::{collections::HashMap, hash::Hash, net::SocketAddr};

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use tokio::sync::mpsc::{Receiver, Sender};
use webparse::{HeaderName, Method, Request, Response, Scheme, Url};
use wenmeng::{Body, Client, ProtError, ProtResult};

use crate::{ConfigHeader, FileServer, HealthCheck, Helper};

use super::{common::CommonConfig, ReverseHelper, TryPathsConfig, UpstreamConfig};

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationConfig {
    pub rule: String,
    pub file_server: Option<FileServer>,

    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[serde(default = "Vec::new")]
    pub headers: Vec<ConfigHeader>,

    /// 请求方法
    pub method: Option<String>,
    pub up_name: Option<String>,

    #[serde(default)]
    pub is_ws: bool,

    pub root: Option<String>,
    #[serde(default = "Vec::new")]
    pub upstream: Vec<UpstreamConfig>,

    #[serde_as(as = "Option<DisplayFromStr>")]
    pub try_paths: Option<TryPathsConfig>,

    #[serde(flatten)]
    #[serde(default = "CommonConfig::new")]
    pub comm: CommonConfig,
}

impl Hash for LocationConfig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write(self.rule.as_bytes());
        if let Some(up_name) = &self.up_name {
            state.write(up_name.as_bytes());
        }
        if let Some(method) = &self.method {
            state.write(method.as_bytes());
        }
        state.finish();
    }
}

impl PartialEq for LocationConfig {
    fn eq(&self, other: &LocationConfig) -> bool {
        self.rule == other.rule && self.up_name == other.up_name && self.method == other.method
    }
}

impl Eq for LocationConfig {}

impl LocationConfig {
    pub fn new() -> Self {
        Self {
            rule: "/".to_string(),
            file_server: None,
            headers: vec![],
            method: None,
            up_name: None,
            is_ws: false,
            root: None,
            upstream: vec![],
            try_paths: None,
            comm: CommonConfig::new(),
        }
    }
    pub fn clone_only_hash(&self) -> LocationConfig {
        LocationConfig {
            rule: self.rule.clone(),
            method: self.method.clone(),
            up_name: self.up_name.clone(),
            is_ws: self.is_ws,
            file_server: None,
            headers: vec![],
            try_paths: None,
            root: None,
            upstream: vec![],
            comm: CommonConfig::new(),
        }
    }

    /// 当本地限制方法时,优先匹配方法,在进行路径的匹配
    pub fn is_match_rule(&self, path: &String, method: &Method) -> bool {
        if self.method.is_some()
            && !self
                .method
                .as_ref()
                .unwrap()
                .eq_ignore_ascii_case(method.as_str())
        {
            return false;
        }
        if let Some(_) = path.find(&self.rule) {
            return true;
        } else {
            false
        }
    }

    async fn deal_client(
        req: &mut Request<Body>,
        client: Client,
    ) -> ProtResult<(
        Response<Body>,
        Option<Sender<Request<Body>>>,
        Option<Receiver<ProtResult<Response<Body>>>>,
    )> {
        println!("处理客户端!!!!");
        let (mut recv, sender) = client.send2(req.replace_clone(Body::empty())).await?;
        match recv.recv().await {
            Some(res) => Ok((res?, Some(sender), Some(recv))),
            None => Err(ProtError::Extension("already close by other")),
        }
    }

    pub async fn deal_reverse_proxy(
        &self,
        req: &mut Request<Body>,
        url: &Url,
    ) -> ProtResult<(
        Response<Body>,
        Option<Sender<Request<Body>>>,
        Option<Receiver<ProtResult<Response<Body>>>>,
    )> {
        let mut url = url.clone();
        let domain = url.domain.clone().unwrap();

        if let Some(addr) = ReverseHelper::get_upstream_addr(&self.upstream, &*domain) {
            url.domain = Some(addr.ip().to_string());
            url.port = Some(addr.port());
        }
        if url.scheme == Scheme::None {
            url.scheme = req.scheme().clone();
        }
        if let Some(connect) = url.get_connect_url() {
            req.headers_mut().insert(HeaderName::HOST, connect.clone());
        }
        let proxy_timeout = self.comm.build_proxy_timeout();

        let mut connect_timeout = None;
        if proxy_timeout.is_some() {
            connect_timeout = proxy_timeout.as_ref().unwrap().connect_timeout.clone();
        }
        let stream = match url.get_connect_url() {
            Some(connect) => HealthCheck::connect_timeout(&connect, connect_timeout).await?,
            None => {
                return Err(ProtError::Extension("get url error"));
            }
        };
        let mut res = if url.scheme.is_http() {
            let client = Client::builder()
                .timeout_layer(proxy_timeout)
                .connect_by_stream(stream)
                .await?;
            Self::deal_client(req, client).await?
        } else {
            let client = Client::builder()
                .timeout_layer(proxy_timeout)
                .url(url.clone())?
                .connect_tls_by_stream(stream)
                .await?;
            Self::deal_client(req, client).await?
        };
        Helper::rewrite_response(&mut res.0, &self.headers);
        Ok(res)
    }

    pub async fn deal_request(
        &self,
        req: &mut Request<Body>,
    ) -> ProtResult<(
        Response<Body>,
        Option<Sender<Request<Body>>>,
        Option<Receiver<ProtResult<Response<Body>>>>,
    )> {
        Helper::log_acess(&self.comm.log_format, &self.comm.access_log, &req);
        if let Some(file_server) = &self.file_server {
            let res = file_server.deal_request(req).await?;
            return Ok((res, None, None));
        }
        if let Some(reverse) = &self.comm.proxy_url {
            return self.deal_reverse_proxy(req, reverse).await;
        }
        return Err(ProtError::Extension("unknow data"));
    }

    pub fn get_log_names(&self, names: &mut HashMap<String, String>) {
        self.comm.get_log_names(names);
    }

    pub fn get_upstream_addr(&self) -> Option<SocketAddr> {
        let mut name = String::new();
        if let Some(r) = &self.comm.proxy_url {
            name = r.domain.clone().unwrap_or(String::new());
        }
        for stream in &self.upstream {
            if stream.name == name {
                return stream.get_server_addr();
            } else if name == "" {
                return stream.get_server_addr();
            }
        }
        return None;
    }

    pub fn get_reverse_url(&self) -> ProtResult<(Url, String)> {
        if let Some(addr) = self.get_upstream_addr() {
            if let Some(r) = &self.comm.proxy_url {
                let mut url = r.clone();
                let domain = url.domain.clone().unwrap_or(String::new());
                url.domain = Some(format!("{}", addr.ip()));
                url.port = Some(addr.port());
                Ok((url, domain))
            } else {
                let url = Url::parse(format!("http://{}/", addr).into_bytes())?;
                let domain = format!("{}", addr.ip());
                Ok((url, domain))
            }
        } else {
            Err(ProtError::Extension("error"))
        }
    }
}
