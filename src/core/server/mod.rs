use std::{thread, time::Duration};

use log::{error, info, warn};
use tokio::{runtime::{Builder, Runtime}, sync::watch};

mod services;

pub type ShutdownWatch = watch::Receiver<bool>;
pub use services::{Service, ServiceTrait};

use crate::{ConfigOption, ProxyConfig};

pub struct Server {
    services: Vec<Box<dyn ServiceTrait>>,
    shutdown_watch: watch::Sender<bool>,
    shutdown_recv: ShutdownWatch,
    opt: Option<ConfigOption>,
}

impl Server {
    pub fn new(opt: Option<ConfigOption>) -> Self {
        let (tx, rx) = watch::channel(false);
        Self {
            opt,
            services: vec![],
            shutdown_watch: tx,
            shutdown_recv: rx,
        }
    }

    pub fn add_service(&mut self, service: impl ServiceTrait + 'static) {
        self.services.push(Box::new(service));
    }

    pub fn add_services(&mut self, services: Vec<Box<dyn ServiceTrait>>) {
        self.services.extend(services);
    }

    async fn main_loop(&self) -> bool {
        let future = std::future::pending();
        let () = future.await;
        false
    }

    fn create_runtime(name: &str, threads: usize) -> Runtime {
        Builder::new_multi_thread()
            .enable_all()
            .worker_threads(threads)
            .thread_name(name)
            .build()
            .unwrap()
    }

    fn run_service(
        mut service: Box<dyn ServiceTrait>,
        shutdown: ShutdownWatch,
        threads: usize,
    ) -> Runtime {
        let service_runtime = Self::create_runtime(service.name(), threads);
        service_runtime.handle().spawn(async move {
            println!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaax");
            if let Err(e) = service.ready_service().await {
                warn!("初始化服务时{}失败, 原因:{:?}", service.name(), e);
                println!("xxxxxxxxxxxxxxxxxxxxxxxx");
                return;
            }
            println!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
            service.start_service(shutdown).await;
            println!("aaaaaaaaabbbbbbbbbbbbbaaaaaaaaaaaaax");
            info!("service exited.")
        });
        service_runtime
    }

    pub fn run_loop(&mut self) {
        let mut runtimes: Vec<Runtime> = Vec::new();

        while let Some(service) = self.services.pop() {
            let runtime = Self::run_service(
                service,
                self.shutdown_recv.clone(),
                2,
            );
            runtimes.push(runtime);
        }
        
        let server_runtime = Self::create_runtime("Server", 1);
        let shutdown_type = server_runtime.handle().block_on(self.main_loop());


        let shutdown_timeout = Duration::from_secs(5);
        let shutdowns: Vec<_> = runtimes
            .into_iter()
            .map(|rt| {
                info!("Waiting for runtimes to exit!");
                thread::spawn(move || {
                    rt.shutdown_timeout(shutdown_timeout);
                    thread::sleep(shutdown_timeout)
                })
            })
            .collect();
        for shutdown in shutdowns {
            if let Err(e) = shutdown.join() {
                error!("Failed to shutdown runtime: {:?}", e);
            }
        }
        info!("All runtimes exited, exiting now");
        std::process::exit(0);
    }
}
