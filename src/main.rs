// #![deny(warnings)]

use std::io::Error;
use std::net::SocketAddr;

use tokio::{net::{TcpListener, TcpStream}, io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt, ReadBuf}};
use commander::Commander;
use webparse::{BinaryMut, Method, WebError, BufMut, Buf};
use wmproxy::{Proxy, ProxyResult};

async fn run_main() -> ProxyResult<()> {
    let mut proxy = Proxy::parse_env()?;
    proxy.start_serve().await?;
    Ok(())
}

#[forever_rs::main]
#[tokio::main]
async fn main() {
    let _  = run_main().await;
}
