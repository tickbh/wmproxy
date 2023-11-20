use std::{hash::Hash, collections::HashMap};

use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc::{Receiver, Sender},
};
use webparse::{HeaderName, Method, Request, Response, Scheme, Url};
use wenmeng::{Client, HeaderHelper, ProtError, ProtResult, RecvStream};

use crate::{HealthCheck, FileServer, Helper};

use super::{ReverseHelper, UpstreamConfig, common::CommonConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationConfig {
    pub rule: String,
    pub file_server: Option<FileServer>,
    #[serde(default = "Vec::new")]
    pub headers: Vec<Vec<String>>,
    pub reverse_proxy: Option<String>,
    /// 请求方法
    pub method: Option<String>,
    pub server_name: Option<String>,

    pub root: Option<String>,
    #[serde(default = "Vec::new")]
    pub upstream: Vec<UpstreamConfig>,
    
    #[serde(flatten)]
    #[serde(default = "CommonConfig::new")]
    pub comm: CommonConfig,
}

impl Hash for LocationConfig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write(self.rule.as_bytes());
        if let Some(server_name) = &self.server_name {
            state.write(server_name.as_bytes());
        }
        if let Some(method) = &self.method {
            state.write(method.as_bytes());
        }
        state.finish();
    }
}

impl PartialEq for LocationConfig {
    fn eq(&self, other: &LocationConfig) -> bool {
        self.rule == other.rule && self.server_name == other.server_name && self.method == other.method
    }
}

impl Eq for LocationConfig {
    
}

impl LocationConfig {
    pub fn clone_only_hash(&self) -> LocationConfig {
        LocationConfig {
            rule: self.rule.clone(),
            method: self.method.clone(),
            server_name: self.server_name.clone(),
            file_server: None,
            headers: vec![],
            reverse_proxy: None,
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

    async fn deal_client<T>(
        req: &mut Request<RecvStream>,
        client: Client<T>,
    ) -> ProtResult<(
        Response<RecvStream>,
        Option<Sender<Request<RecvStream>>>,
        Option<Receiver<ProtResult<Response<RecvStream>>>>,
    )>
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        println!("处理客户端!!!!");
        let (mut recv, sender) = client.send2(req.replace_clone(RecvStream::empty())).await?;
        match recv.recv().await {
            Some(res) => Ok((res?, Some(sender), Some(recv))),
            None => Err(ProtError::Extension("already close by other")),
        }
    }

    pub async fn deal_reverse_proxy(
        &self,
        req: &mut Request<RecvStream>,
        reverse: String,
    ) -> ProtResult<(
        Response<RecvStream>,
        Option<Sender<Request<RecvStream>>>,
        Option<Receiver<ProtResult<Response<RecvStream>>>>,
    )> {
        let url = TryInto::<Url>::try_into(reverse.clone()).ok();
        if url.is_none() || url.as_ref().unwrap().domain.is_none() {
            return Err(ProtError::Extension("unknow data"));
        }
        let mut url = url.unwrap();
        let domain = url.domain.clone().unwrap();

        if let Ok(addr) = ReverseHelper::get_upstream_addr(&self.upstream, &*domain) {
            url.domain = Some(addr.ip().to_string());
            url.port = Some(addr.port());
        }
        if url.scheme == Scheme::None {
            url.scheme = req.scheme().clone();
        }
        req.headers_mut()
            .insert(HeaderName::HOST, url.domain.clone().unwrap());
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
            let client = Client::builder().timeout_layer(proxy_timeout).connect_by_stream(stream).await?;
            Self::deal_client(req, client).await?
        } else {
            let client = Client::builder().timeout_layer(proxy_timeout).connect_tls_by_stream(stream, url).await?;
            Self::deal_client(req, client).await?
        };
        HeaderHelper::rewrite_response(&mut res.0, &self.headers);
        Ok(res)
    }

    pub async fn deal_request(
        &self,
        req: &mut Request<RecvStream>,
    ) -> ProtResult<(
        Response<RecvStream>,
        Option<Sender<Request<RecvStream>>>,
        Option<Receiver<ProtResult<Response<RecvStream>>>>,
    )> {
        Helper::log_acess(&self.comm.log_format, &self.comm.access_log, &req);
        if let Some(file_server) = &self.file_server {
            let res = file_server.deal_request(req).await?;
            return Ok((res, None, None));
        }
        if let Some(reverse) = &self.reverse_proxy {
            println!("反向代理");
            return self.deal_reverse_proxy(req, reverse.clone()).await;
        }
        return Err(ProtError::Extension("unknow data"));
    }

    pub fn get_log_names(&self, names: &mut HashMap<String, String>)  {
        self.comm.get_log_names(names);
    }
}
