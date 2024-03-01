use std::{mem, sync::Arc};

use crate::core::{apps::AppTrait, listeners::{Listeners, WrapListener}};

use self::service::ServiceTrait;
use async_trait::async_trait;
use tokio::runtime::Handle;

use super::ShutdownWatch;

mod service;

pub struct Service<A> {
    name: String,
    listeners: Listeners,
    app_logic: Arc<A>,
}

impl<A> Service<A> {
    pub fn new(name: String, app_logic: Arc<A>) -> Self {
        Self {
            name,
            listeners: Listeners::new(),
            app_logic,
        }
    }

    pub fn with_listeners(name: String, listeners: Listeners, app_logic: Arc<A>) -> Self {
        Service {
            name,
            listeners,
            app_logic,
        }
    }
}


impl<A: AppTrait + Send + Sync + 'static> Service<A> {

    async fn run_wrap(
        app_logic: Arc<A>,
        mut stack: WrapListener,
        mut shutdown: ShutdownWatch,
    ) {
        // loop {
        //     let new_io = tokio::select! {

        //     }
        // }
    }
}

#[async_trait]
impl<A: AppTrait + Send + Sync + 'static> ServiceTrait for Service<A> {

    async fn start_service(&mut self, shutdown: ShutdownWatch) {
        let runtime = Handle::current();
        let wrap_listeners = mem::replace(&mut self.listeners.listener, vec![]);
        
        let handlers = wrap_listeners.into_iter().map(|endpoint| {
            let app_logic = self.app_logic.clone();
            let shutdown = shutdown.clone();
            runtime.spawn(async move {
                Self::run_wrap(app_logic, endpoint, shutdown).await;
            })
        });

        futures::future::join_all(handlers).await;
        self.app_logic.cleanup();
    }

    fn name(&self) -> &str {
        &self.name
    }
}
