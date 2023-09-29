// #![deny(warnings)]

use wmproxy::{ProxyOption, ProxyResult, Proxy};

async fn run_main() -> ProxyResult<()> {
    let option = ProxyOption::parse_env()?;
    let mut proxy = Proxy::new(option);
    proxy.start_serve().await?;
    Ok(())
}

// #[forever_rs::main]
#[tokio::main]
async fn main() {
    if let Err(e) = run_main().await {
        println!("运行wmproxy发生错误:{:?}", e);
    }
}
