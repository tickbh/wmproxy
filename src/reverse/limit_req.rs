use wenmeng::{HeaderHelper, Middleware};

use async_trait::async_trait;

use wenmeng::{ProtError, ProtResult, RecvRequest, RecvResponse};

pub struct LimitReqZone {
    /// 空间名字
    name: String,
    /// 键值的匹配方式
    key: String,
    /// IP个数
    limit: usize,
}

pub struct LimitReq {
    zone: String,
    burst: usize,
}

pub struct LimitReqMiddleware {
    req: LimitReq,
}

#[async_trait]
impl Middleware for LimitReqMiddleware {
    async fn process_request(&mut self, request: &mut RecvRequest) -> ProtResult<()> {
        
        Ok(())
    }
    async fn process_response(
        &mut self,
        _request: &mut RecvRequest,
        response: &mut RecvResponse,
    ) -> ProtResult<()> {
        Ok(())
    }
}
