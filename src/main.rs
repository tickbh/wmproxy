#![deny(warnings)]

use std::io::Error;
use std::net::SocketAddr;

use tokio::{net::{TcpListener, TcpStream}, io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt, ReadBuf}};
use commander::Commander;
use webparse::{BinaryMut, Method, WebError, BufMut, Buf};

async fn process(inbound: &mut TcpStream) -> Result<(), WebError> {
    let mut outbound;
    let mut request;
    let mut buffer = BinaryMut::new();
    loop {
        
        let size = {
            let mut buf = ReadBuf::uninit(buffer.chunk_mut());
            inbound.read_buf(&mut buf).await?;
            buf.filled().len()
        };

        if size == 0 {
            return Err(WebError::Extension("empty"));
        }
        unsafe {
            buffer.advance_mut(size);
        }
        println!("n === {:?}", size);
        
        request = webparse::Request::new();
        let _result = request.parse_buffer(&mut buffer.clone())?;
        match request.get_connect_url() {
            Some(host) => {
                // println!("host !!= {}", host);
                outbound = TcpStream::connect(host).await?;
                break;
            }
            None => continue,
        }
    }

    match request.method() {
        &Method::Connect => {
            println!("connect = {:?}", String::from_utf8_lossy(buffer.chunk()));
            inbound.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await?;
        }
        _ => {
            outbound.write_all(buffer.chunk()).await?;
        }
    }
    // println!("outbound = {:?}", outbound);
    let _ = copy_bidirectional(inbound, &mut outbound)
        .await?;
    Ok(())
}

async fn run_main() -> Result<(), Error> {
    let command = Commander::new()
    .version(&env!("CARGO_PKG_VERSION").to_string())
    .usage("-b 127.0.0.1 -p 8100")
    .usage_desc("use http proxy")
    .option_int("-p, --port [value]", "listen port", Some(8100))
    .option_str("-b, --bind [value]", "bind addr", Some("0.0.0.0".to_string()))
    .parse_env_or_exit()
    ;
    
    let listen_port: u16 = command.get_int("p").unwrap() as u16;
    let listen_host = command.get_str("b").unwrap();

    println!("listener bind {} {}", listen_host, listen_port);
    let addr: SocketAddr = match format!("{}:{}", listen_host, listen_port).parse() {
        Err(_) => SocketAddr::from(([127, 0, 0, 1], listen_port as u16)),
        Ok(addr) => addr
    };
    
    println!("listener bind {}", addr);
    let listener = TcpListener::bind(addr).await?;
    while let Ok((mut inbound, _)) = listener.accept().await {
        tokio::spawn(async move {
            
            match process(&mut inbound).await {
                Err(_) => {
                    let _ = inbound.write_all(b"HTTP/1.1 500 OK\r\n\r\n").await;
                }
                _ => {

                }
            }


            // let mut buf = vec![0; 1024];

            // // In a loop, read data from the socket and write the data back.
            // loop {
            //     let n = inbound
            //         .read(&mut buf)
            //         .await
            //         .expect("failed to read data from socket");

            //     println!("receive size = {:?}", &buf[..n]);
            //     if n == 0 {
            //         return;
            //     }

            //     inbound
            //         .write_all(&buf[0..n])
            //         .await
            //         .expect("failed to write data to socket");
            // }
            // let mut outbound = TcpStream::connect(server_addr.clone()).await?;
            // copy_bidirectional(&mut inbound, &mut outbound)
            //     .map(|r| {
            //         if let Err(e) = r {
            //             println!("Failed to transfer; error={}", e);
            //         }
            //     })
            //     .await
        });
    }
    // println!("Listening on http://{}", addr);

    // if let Err(e) = server.await {
    //     eprintln!("server error: {}", e);
    // }
    Ok(())
}

#[forever_rs::main]
#[tokio::main]
async fn main() {
    let _  = run_main().await;
}
