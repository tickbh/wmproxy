// #![deny(warnings)]

use wmproxy::{ConfigOption, ProxyResult, ControlServer, Helper};
use log::{error, info, warn, LevelFilter};
use log4rs::{
    append::console::ConsoleAppender,
    config::{Appender, Root},
    encode::json::JsonEncoder,
};

async fn run_main() -> ProxyResult<()> {
    // let stdout: ConsoleAppender = ConsoleAppender::builder()
    // .encoder(Box::new(JsonEncoder::new()))
    // .build();
    // let log_config = log4rs::config::Config::builder()
    //     .appender(Appender::builder().build("stdout", Box::new(stdout)))
    //     .build(Root::builder().appender("stdout").build(LevelFilter::Info))
    //     .unwrap();
    // log4rs::init_config(log_config).unwrap();
    // let stdout: ConsoleAppender = ConsoleAppender::builder()
    // .encoder(Box::new(JsonEncoder::new()))
    // .build();
    // let log_config = log4rs::config::Config::builder()
    //     .appender(Appender::builder().build("stdout", Box::new(stdout)))
    //     .build(Root::builder().appender("stdout").build(LevelFilter::Info))
    //     .unwrap();
    // log4rs::init_config(log_config).unwrap();

    // env_logger::init();
    let option = ConfigOption::parse_env()?;
    Helper::try_init_log(&option);
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
        .thread_stack_size(10 * 1024 * 1024)
        .build()
        .unwrap();
    runtime.block_on(async {
        if let Err(e) = run_main().await {
            println!("运行wmproxy发生错误:{:?}", e);
        }
    })
}