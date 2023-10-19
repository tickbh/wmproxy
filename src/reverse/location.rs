use std::sync::Weak;

use serde::{Serialize, Deserialize};
use tokio::io::{AsyncWrite, AsyncRead};
use webparse::{Request, Response, Url, HeaderName};
use wenmeng::{FileServer, RecvStream, ProtResult, ProtError, Client, HeaderHelper};

use super::ServerConfig;

fn default_headers() -> Vec<Vec<String>> {
    vec![]
}

fn default_null() -> *const ServerConfig {
    std::ptr::null()
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationConfig {
    pub rule: String,
    pub file_server: Option<FileServer>,
    #[serde(default = "default_headers")]
    pub headers: Vec<Vec<String>>,
    pub reverse_proxy: Option<String>,

    // #[serde(skip, default = "default_null")]
    // pub weak_server: *const ServerConfig,
}

impl LocationConfig {
    pub fn is_match_rule(&self, path: &String) -> bool {
        if let Some(_) = path.find(&self.rule) {
            return true;
        } else {
            false
        }
    }

    
    // async fn inner_operate(
    //     &mut self,
    //     mut req: Request<RecvStream>
    // ) -> ProtResult<Response<RecvStream>> {
    //     println!("receiver req = {:?}", req.url());
    //     // if let Some(f) = &mut value.file_server {
    //     //     f.deal_request(req).await
    //     // } else {
    //     if let Some(file_server) = &mut self.file_server {
    //         file_server.deal_request(req)
    //     }
    //     return Err(ProtError::Extension("unknow data"));
    //     // }
    // }
    async fn deal_client<T>(mut req: Request<RecvStream>, client: Client<T>) -> ProtResult<Response<RecvStream>> 
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + 'static, {
        let (mut recv, _sender) = client.send2(req.into_type()).await?;
        let mut res = recv.recv().await.unwrap();
        Ok(res)
    }

    pub async fn deal_reverse_proxy(&mut self, 
        mut req: Request<RecvStream>, reverse: String) -> ProtResult<Response<RecvStream>> {
        let url = TryInto::<Url>::try_into(reverse.clone()).ok();
        if url.is_none() {
            return Err(ProtError::Extension("unknow data"));
        }
        let url = url.unwrap();
        req.headers_mut().insert(HeaderName::HOST, url.domain.clone().unwrap());
        
        let mut res = if url.scheme.is_http() {
            let client = Client::builder().connect(url).await?;
            Self::deal_client(req, client).await?
        } else {
            let client = Client::builder().connect_tls(url).await?;
            Self::deal_client(req, client).await?
        };
        HeaderHelper::rewrite_response(&mut res, &self.headers);
        Ok(res)
    }
    
    pub async fn deal_request(
        server: &mut ServerConfig,
        location_index: usize,
        mut req: Request<RecvStream>,
    ) -> ProtResult<Response<RecvStream>> {
        let mut location = &mut server.location[location_index];
        println!("receiver req = {:?}", req.url());
        if let Some(file_server) = &mut location.file_server {
            if file_server.root.is_none() && server.root.is_some() {
                file_server.root = server.root.clone();
            }
            if file_server.prefix.is_empty() {
                file_server.set_prefix(location.rule.clone());
            }
            return file_server.deal_request(req).await
        }
        if let Some(reverse) = &location.reverse_proxy {
            return location.deal_reverse_proxy(req, reverse.clone()).await;
        }
        return Err(ProtError::Extension("unknow data"));
    }

}