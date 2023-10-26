// #![deny(warnings)]

use wmproxy::{ConfigOption, ProxyResult, ControlServer};

async fn run_main() -> ProxyResult<()> {
    env_logger::init();
    let option = ConfigOption::parse_env()?;
    let control = ControlServer::new(option);
    control.start_serve().await?;
    Ok(())
}

// #[forever_rs::main]
// #[tokio::main]
// async fn main() {
//     if let Err(e) = run_main().await {
//         println!("运行wmproxy发生错误:{:?}", e);
//     }
// }

fn main() {
    use tokio::runtime::Builder;
    let runtime = Builder::new_multi_thread()
        .enable_io()
        .worker_threads(4)
        .enable_time()
        .thread_name("wmproxy")
        .thread_stack_size(10 * 1024 * 1024 * 1024)
        .build()
        .unwrap();
    runtime.block_on(async {
        if let Err(e) = run_main().await {
            println!("运行wmproxy发生错误:{:?}", e);
        }
    })
}