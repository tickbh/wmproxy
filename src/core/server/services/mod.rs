use std::{error::Error, io, mem, sync::Arc};

use crate::core::{apps::AppTrait, listeners::{Listeners, WrapListener}, Stream};

pub use self::service::ServiceTrait;
use async_trait::async_trait;
use log::{debug, error, info};
use tokio::runtime::Handle;

use super::ShutdownWatch;

mod service;

pub struct Service<A> {
    name: String,
    listeners: Listeners,
    app_logic: Option<A>,
}

impl<A> Service<A> {
    pub fn new(name: String, app_logic: A) -> Self {
        Self {
            name,
            listeners: Listeners::new(),
            app_logic: Some(app_logic),
        }
    }

    pub fn with_listeners(name: String, listeners: Listeners, app_logic: A) -> Self {
        Service {
            name,
            listeners,
            app_logic: Some(app_logic),
        }
    }
}


impl<A: AppTrait + Send + Sync + 'static> Service<A> {
    pub async fn handle_event(event: Stream, app_logic: Arc<A>, shutdown: ShutdownWatch) {
        debug!("new event!");
        let mut reuse_event = app_logic.process_new(event, &shutdown).await;
        while let Some(event) = reuse_event {
            debug!("new reusable event!");
            reuse_event = app_logic.process_new(event, &shutdown).await;
        }
    }
    
    async fn run_wrap(
        app_logic: Arc<A>,
        mut listener: WrapListener,
        mut shutdown: ShutdownWatch,
    ) {
        loop {
            let new_io = tokio::select! {
                new_io = listener.accept() => {
                    new_io
                }
                shutdown_signal = shutdown.changed() => {
                    match shutdown_signal {
                        Ok(()) => {
                            if !*shutdown.borrow() {
                                // happen in the initial read
                                continue;
                            }
                            info!("Shutting down {}", listener.local_desc());
                            break;
                        }
                        Err(e) => {
                            error!("shutdown_signal error {e}");
                            break;
                        }
                    }
                }
            };

            match new_io {
                Ok(io) => {
                    let app = app_logic.clone();
                    let shutdown = shutdown.clone();
                    tokio::spawn(async move {
                            Self::handle_event(io, app, shutdown).await
                    });
                },
                Err(e) => {
                    if let Some(io_error) = e.raw_os_error() {
                        // too many open files
                        if io_error == 24 {
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        }
                    }
                },
            }


        }
    }
}

#[async_trait]
impl<A: AppTrait + Send + Sync + 'static> ServiceTrait for Service<A> {
    
    async fn ready_service(&mut self) -> io::Result<()> {
        for listener in &mut self.listeners.listener {
            listener.try_init().await?;
        }
        if self.app_logic.is_none() {
            return Err(io::Error::new(io::ErrorKind::Other, "Not found app_logic"));
        }
        self.app_logic.as_mut().unwrap().ready_init().await?;
        Ok(())
    }
    
    async fn start_service(&mut self, shutdown: ShutdownWatch) {
        let runtime = Handle::current();
        let wrap_listeners = mem::replace(&mut self.listeners.listener, vec![]);
        println!("cccccccccccccc");
        let app_logic = Arc::new(self.app_logic.take().unwrap());
        let handlers = wrap_listeners.into_iter().map(|endpoint| {
            let shutdown = shutdown.clone();
            let app_logic_clone = app_logic.clone();
            runtime.spawn(async move {
                Self::run_wrap(app_logic_clone, endpoint, shutdown).await;
            })
        });

        futures::future::join_all(handlers).await;
        println!("dddddddddddddddd");
        app_logic.cleanup();
    }

    fn name(&self) -> &str {
        &self.name
    }
}
