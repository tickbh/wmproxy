// #![deny(warnings)]







use wmproxy::{Proxy, ProxyResult};

async fn run_main() -> ProxyResult<()> {
    let mut proxy = Proxy::parse_env()?;
    proxy.start_serve().await?;
    Ok(())
}

// #[forever_rs::main]
#[tokio::main]
async fn main() {
    let _  = run_main().await;
}
