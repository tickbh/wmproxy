use std::{io, sync::Arc};
use async_trait::async_trait;

use super::{server::ShutdownWatch, Stream};

#[async_trait]
pub trait AppTrait {
    async fn ready_init(
        &mut self,
    ) -> io::Result<()> {
        Ok(())
    }

    async fn process_new(
        self: &Arc<Self>,
        mut session: Stream,
        shutdown: &ShutdownWatch,
    ) -> Option<Stream>;

    fn cleanup(&self) {}
}