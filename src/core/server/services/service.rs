use async_trait::async_trait;

use crate::core::server::ShutdownWatch;


#[async_trait]
pub trait ServiceTrait: Sync + Send {
    async fn start_service(&mut self, mut shutdown: ShutdownWatch);
    fn name(&self) -> &str;
}