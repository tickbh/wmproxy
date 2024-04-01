use std::{sync::{Arc, Mutex}, thread, time::Duration};

use log::{error, info, warn};
use tokio::{runtime::{Builder, Runtime}, sync::{mpsc::{channel, Receiver}, watch}};

mod services;

pub type ShutdownWatch = watch::Receiver<bool>;
pub use services::{Service, ServiceTrait};

use crate::{ConfigOption, ProxyConfig};

pub struct Server {
    services: Vec<Box<dyn ServiceTrait>>,
    pub shutdown_watch: Arc<Mutex<watch::Sender<bool>>>,
    pub shutdown_recv: ShutdownWatch,
    opt: ConfigOption,
}

impl Server {
    pub fn new(opt: ConfigOption) -> Self {
        let (tx, rx) = watch::channel(false);
        Self {
            opt,
            services: vec![],
            shutdown_watch: Arc::new(Mutex::new(tx)),
            shutdown_recv: rx,
        }
    }

    pub fn add_service(&mut self, service: impl ServiceTrait + 'static) {
        self.services.push(Box::new(service));
    }

    pub fn add_services(&mut self, services: Vec<Box<dyn ServiceTrait>>) {
        self.services.extend(services);
    }

    async fn main_loop(&self, receiver: Option<Receiver<()>>) -> bool {
        // let (tx, mut rx) = channel(1);
        // let _tx1 = tx.clone();

        // let _ = ctrlc::set_handler(move || {
        //     let tx = tx.clone();
        //     thread::spawn(move || {
        //         let _ = tx.blocking_send(());
        //     });
        // });
        
        // println!("Waiting for Ctrl-C...");

        async fn try_receiver_close(mut receiver: Option<Receiver<()>>) {
            if receiver.is_some() {
                receiver.as_mut().unwrap().recv().await;
            } else {
                let pending = std::future::pending::<()>();
                pending.await
            }
        }
        
        let mut recv = self.shutdown_recv.clone();
        tokio::select! {
            _ = recv.changed() => {
                info!("Got it! Exiting...");
            }
            _ = try_receiver_close(receiver) => {
                info!("Got it! Exiting...");
                return false;
            }
        }
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
        mut service: Box<dyn ServiceTrait + 'static>,
        shutdown: ShutdownWatch,
        threads: usize,
    ) -> Runtime {
        let service_runtime = Self::create_runtime(service.name(), threads);
        service_runtime.handle().spawn(async move {
            if let Err(e) = service.ready_service().await {
                warn!("初始化服务时{}失败, 原因:{:?}", service.name(), e);
                return;
            }
            service.start_service(shutdown).await;
            info!("service exited.")
        });
        service_runtime
    }
    pub fn run_loop(&mut self) {
        self.run_loop_with_recv(None)
    }

    pub fn run_loop_with_recv(&mut self, receiver: Option<Receiver<()>>) {
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
        let shutdown_type = server_runtime.handle().block_on(self.main_loop(receiver));

        let shutdown_timeout = Duration::from_secs(0);
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
        // std::process::exit(0);
    }
}
