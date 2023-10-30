use std::hash::Hash;

use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc::{Receiver, Sender},
};
use webparse::{HeaderName, Method, Request, Response, Scheme, Url};
use wenmeng::{Client, FileServer, HeaderHelper, ProtError, ProtResult, RecvStream};

use crate::HealthCheck;

use super::{ReverseHelper, UpstreamConfig};

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
        req: Request<RecvStream>,
        client: Client<T>,
    ) -> ProtResult<(
        Response<RecvStream>,
        Option<Sender<Request<RecvStream>>>,
        Option<Receiver<Response<RecvStream>>>,
    )>
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (mut recv, sender) = client.send2(req.into_type()).await?;
        let res = recv.recv().await.unwrap();
        Ok((res, Some(sender), Some(recv)))
    }

    pub async fn deal_reverse_proxy(
        &self,
        mut req: Request<RecvStream>,
        reverse: String,
    ) -> ProtResult<(
        Response<RecvStream>,
        Option<Sender<Request<RecvStream>>>,
        Option<Receiver<Response<RecvStream>>>,
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
        let stream = match url.get_connect_url() {
            Some(connect) => HealthCheck::connect(&connect).await?,
            None => {
                return Err(ProtError::Extension("get url error"));
            }
        };
        let mut res = if url.scheme.is_http() {
            let client = Client::builder().http2(false).connect_by_stream(stream).await?;
            Self::deal_client(req, client).await?
        } else {
            let client = Client::builder().http2(false).connect_tls_by_stream(stream, url).await?;
            Self::deal_client(req, client).await?
        };
        HeaderHelper::rewrite_response(&mut res.0, &self.headers);
        Ok(res)
    }

    pub async fn deal_request(
        &self,
        req: Request<RecvStream>,
    ) -> ProtResult<(
        Response<RecvStream>,
        Option<Sender<Request<RecvStream>>>,
        Option<Receiver<Response<RecvStream>>>,
    )> {
        if let Some(file_server) = &self.file_server {
            let res = file_server.deal_request(req).await?;
            return Ok((res, None, None));
        }
        if let Some(reverse) = &self.reverse_proxy {
            return self.deal_reverse_proxy(req, reverse.clone()).await;
        }
        return Err(ProtError::Extension("unknow data"));
    }
}
