use serde::{Serialize, Deserialize};
use webparse::{Request, Response, Url, HeaderName};
use wenmeng::{FileServer, RecvStream, ProtResult, ProtError, Client};

fn default_headers() -> Vec<Vec<String>> {
    vec![]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocationConfig {
    pub rule: String,
    pub file_server: Option<FileServer>,
    #[serde(default = "default_headers")]
    pub headers: Vec<Vec<String>>,
    pub reverse_proxy: Option<String>,
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
    
    pub async fn deal_request(
        &mut self,
        mut req: Request<RecvStream>
    ) -> ProtResult<Response<RecvStream>> {
        println!("receiver req = {:?}", req.url());
        // if let Some(f) = &mut value.file_server {
        //     f.deal_request(req).await
        // } else {
        if let Some(file_server) = &mut self.file_server {
            if file_server.prefix.is_empty() {
                file_server.set_prefix(self.rule.clone());
            }
            return file_server.deal_request(req).await
        }
        if let Some(reverse) = &self.reverse_proxy {
            let url = TryInto::<Url>::try_into(reverse.clone()).ok();
            if url.is_none() {
                return Err(ProtError::Extension("unknow data"));
            }
            let url = url.unwrap();
            req.headers_mut().insert(HeaderName::HOST, url.domain.clone().unwrap());
            println!("aaaaaaaaaaassss");
            let client = Client::builder().connect(url).await?;
            println!("bbbbbbbbbbbbbbbssss");
            
            let (mut recv, _sender) = client.send2(req.into_type()).await?;
            let mut res = recv.recv().await.unwrap();
            return Ok(res);
            // res.body_mut().wait_all().await;
            // println!("res = {}", res);
        }
        return Err(ProtError::Extension("unknow data"));
    }

}