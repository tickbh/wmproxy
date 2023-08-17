#![deny(warnings)]

use std::io::Error;
use std::net::SocketAddr;

use tokio::{net::{TcpListener, TcpStream}, io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt}};
use commander::Commander;
use webparse::Buffer;

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
            let mut outbound;
            let mut buffer = Buffer::new();
            // In a loop, read data from the socket and write the data back.
            loop {
                let n = inbound
                    .read(buffer.get_write_array(1024))
                    .await
                    .expect("failed to read data from socket");

                buffer.add_write_len(n);
                buffer.uncommit();
                println!("n === {:?}", n);
                println!("value = {:?}", String::from_utf8_lossy(buffer.get_write_data()));
                
                let mut request = webparse::Request::new();
                let _result = request.parse_buffer(&mut buffer).expect("ok");
                match request.get_connect_url() {
                    Some(host) => {
                        println!("host !!= {}", host);
                        outbound = TcpStream::connect(host).await.unwrap();
                        break;
                    }
                    None => continue,
                }
            }

            buffer.set_start(0);
            outbound.write_all(buffer.get_write_data()).await.expect("");

            // let mut outbound = TcpStream::connect("www.baidu.com:80").await.unwrap();
            // println!("outbound = {:?}", outbound);
            let _ = copy_bidirectional(&mut inbound, &mut outbound)
                .await.map(|_r| {
                    // if let Err(e) = r {
                    //     println!("Failed to transfer; error={}", e);
                    // }
                });

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

// async fn proxy(client: HttpClient, req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
//     println!("req: {:?}", req);


//     if Method::CONNECT == req.method() {
//         // Received an HTTP request like:
//         // ```
//         // CONNECT www.domain.com:443 HTTP/1.1
//         // Host: www.domain.com:443
//         // Proxy-Connection: Keep-Alive
//         // ```
//         //
//         // When HTTP method is CONNECT we should return an empty body
//         // then we can eventually upgrade the connection and talk a new protocol.
//         //
//         // Note: only after client received an empty body with STATUS_OK can the
//         // connection be upgraded, so we can't return a response inside
//         // `on_upgrade` future.
//         if let Some(addr) = host_addr(req.uri()) {
//             tokio::task::spawn(async move {
//                 match hyper::upgrade::on(req).await {
//                     Ok(upgraded) => {
//                         if let Err(e) = tunnel(upgraded, addr).await {
//                             eprintln!("server io error: {}", e);
//                         };
//                     }
//                     Err(e) => eprintln!("upgrade error: {}", e),
//                 }
//             });
//             Ok(Response::new(Body::empty()))
//         } else {
//             eprintln!("CONNECT host is not socket addr: {:?}", req.uri());
//             let mut resp = Response::new(Body::from("CONNECT must be to a socket address"));
//             *resp.status_mut() = http::StatusCode::BAD_REQUEST;

//             Ok(resp)
//         }
//     } else {
//         println!("request!!!!!! {:?}", time::Instant::now());
//         let res = client.request(req).await;
//         println!("end!!!!!!!!!!!!! {:?}", time::Instant::now());
//         res
//     }
// }

// fn host_addr(uri: &http::Uri) -> Option<String> {
//     uri.authority().and_then(|auth| Some(auth.to_string()))
// }

// // Create a TCP connection to host:port, build a tunnel between the connection and
// // the upgraded connection
// async fn tunnel(mut upgraded: Upgraded, addr: String) -> std::io::Result<()> {
//     // Connect to remote server
//     let mut server = TcpStream::connect(addr).await?;

//     // Proxying data
//     let (from_client, from_server) =
//         tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;

//     // Print message when done
//     println!(
//         "client wrote {} bytes and received {} bytes",
//         from_client, from_server
//     );

//     Ok(())
// }
