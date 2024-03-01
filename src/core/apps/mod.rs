use std::sync::Arc;
use async_trait::async_trait;

use super::{server::ShutdownWatch, Stream};

#[async_trait]
pub trait AppTrait {
    async fn process_new(
        self: &Arc<Self>,
        mut session: Stream,
        shutdown: &ShutdownWatch,
    ) -> Option<Stream>;

    fn cleanup(&self) {}
}