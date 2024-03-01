use tokio::sync::watch;

mod services;

pub type ShutdownWatch = watch::Receiver<bool>;