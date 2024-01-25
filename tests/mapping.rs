#![deny(rust_2018_idioms)]

/// 关于内网映射相关
#[cfg(test)]
mod tests {

    use async_trait::async_trait;
    use std::{
        error::Error,
        io::{self},
        net::SocketAddr,
    };
    use tokio::{
        net::{TcpListener, TcpStream},
        sync::mpsc::{channel, Sender},
    };
    use webparse::{BinaryMut, Buf, Request, Response, Version};
    use wmproxy::{ConfigHeader, ConfigOption, MappingConfig, ProxyConfig, WMCore};

    use wenmeng::{
        self, Body, Client, HttpTrait, ProtResult, RecvRequest, RecvResponse, Server,
    };

    static HELLO_WORLD: &str = "Hello, World!";
    struct Operate;

    #[async_trait]
    impl HttpTrait for Operate {
        async fn operate(&mut self, req: &mut RecvRequest) -> ProtResult<RecvResponse> {
            let builder = Response::builder().version(req.version().clone());
            let response = builder
                .body(Body::new_text(HELLO_WORLD.to_string()))
                .map_err(|_err| io::Error::new(io::ErrorKind::Other, ""))?;
            Ok(response)
        }
    }

    async fn process(stream: TcpStream, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        let mut server = Server::new(stream, Some(addr));
        server.set_callback_http(Box::new(Operate));
        let _ret = server.incoming().await;
        Ok(())
    }

    async fn run_server() -> ProtResult<SocketAddr> {
        env_logger::init();
        let addr = "127.0.0.1:0".to_string();
        let server = TcpListener::bind(&addr).await?;
        let addr = server.local_addr()?;
        println!("Listening on: {}", addr);
        tokio::spawn(async move {
            loop {
                if let Ok((stream, addr)) = server.accept().await {
                    tokio::spawn(async move {
                        if let Err(e) = process(stream, addr).await {
                            println!("failed to process connection; error = {}", e);
                        }
                    });
                }
            }
        });
        Ok(addr)
    }

    async fn run_mapping_server(
        proxy: ProxyConfig,
    ) -> ProtResult<(
        SocketAddr,
        Option<SocketAddr>,
        Option<SocketAddr>,
        Option<SocketAddr>,
        Option<SocketAddr>,
        Sender<()>,
    )> {
        let option = ConfigOption::new_by_proxy(proxy);
        let (sender_close, receiver_close) = channel::<()>(1);
        let mut proxy = WMCore::new(option);
        proxy.ready_serve().await.unwrap();
        let addr = proxy.center_listener.as_ref().unwrap().local_addr()?;
        let result = (
            addr,
            proxy
                .map_http_listener
                .as_ref()
                .map(|l| l.local_addr().unwrap()),
            proxy
                .map_https_listener
                .as_ref()
                .map(|l| l.local_addr().unwrap()),
            proxy
                .map_tcp_listener
                .as_ref()
                .map(|l| l.local_addr().unwrap()),
            proxy
                .map_proxy_listener
                .as_ref()
                .map(|l| l.local_addr().unwrap()),
            sender_close,
        );
        tokio::spawn(async move {
            let _ = proxy.run_serve(receiver_close, None).await;
        });
        Ok(result)
    }

    async fn run_mapping_client(proxy: ProxyConfig) -> ProtResult<Sender<()>> {
        let option = ConfigOption::new_by_proxy(proxy);
        let (sender_close, receiver_close) = channel::<()>(1);
        let mut proxy = WMCore::new(option);
        proxy.ready_serve().await.unwrap();
        tokio::spawn(async move {
            let _ = proxy.run_serve(receiver_close, None).await;
        });
        Ok(sender_close)
    }

    #[tokio::test]
    async fn run_test() {
        let local_server_addr = run_server().await.unwrap();
        let addr = "127.0.0.1:0".parse().unwrap();
        let proxy = ProxyConfig::builder()
            .center_addr(addr)
            .map_http_bind(Some(addr))
            .map_https_bind(Some(addr))
            .map_tcp_bind(Some(addr))
            .map_proxy_bind(Some(addr))
            .into_value()
            .unwrap();

        let (server_addr, http_addr, https_addr, tcp_addr, proxy_addr, _sender) =
            run_mapping_server(proxy).await.unwrap();
        let mut mapping = MappingConfig::new(
            "test".to_string(),
            "http".to_string(),
            "soft.wm-proxy.com".to_string(),
            vec![],
        );
        mapping.local_addr = Some(local_server_addr);

        let mut mapping_tcp = MappingConfig::new(
            "tcp".to_string(),
            "tcp".to_string(),
            "soft.wm-proxy.com".to_string(),
            vec![],
        );
        mapping_tcp.local_addr = Some(local_server_addr);

        let mut mapping_proxy = MappingConfig::new(
            "proxy".to_string(),
            "proxy".to_string(),
            "soft.wm-proxy.com1".to_string(),
            vec![ConfigHeader::new(
                wmproxy::HeaderOper::Add,
                false,
                "from_proxy".to_string(),
                "mapping".to_string(),
            )],
        );
        mapping_proxy.local_addr = Some(local_server_addr);

        let proxy = ProxyConfig::builder()
            .bind(addr)
            .server(Some(format!("{}", server_addr)))
            .mapping(mapping)
            .mapping(mapping_tcp)
            .mapping(mapping_proxy)
            .into_value()
            .unwrap();
        let _client_sender = run_mapping_client(proxy).await.unwrap();

        fn do_build_req(url: &str, method: &str, body: &Vec<u8>) -> Request<Body> {
            let body = BinaryMut::from(body.clone());
            Request::builder()
                .method(method)
                .url(&*url)
                .body(Body::new_binary(body))
                .unwrap()
        }
        {
            let url = &*format!("http://{}/", http_addr.unwrap());
            let build_url = &*format!("http://soft.wm-proxy.com/");

            let client = Client::builder()
                .http2_only(true)
                .url(&*url).unwrap()
                .connect()
                .await
                .unwrap();

            let mut res = client
                .send_now(do_build_req(build_url, "GET", &vec![]))
                .await
                .unwrap();
            let mut result = BinaryMut::new();
            res.body_mut().read_all(&mut result).await;

            assert_eq!(result.remaining(), HELLO_WORLD.as_bytes().len());
            assert_eq!(result.as_slice(), HELLO_WORLD.as_bytes());
            assert_eq!(res.version(), Version::Http2);
        }

        {
            let url = &*format!("https://{}/", https_addr.unwrap());
            let build_url = &*format!("https://soft.wm-proxy.com/");

            let client = Client::builder()
                .http2_only(true)
                .url(&*url).unwrap()
                .connect_with_domain("soft.wm-proxy.com")
                .await
                .unwrap();

            let mut res = client
                .send_now(do_build_req(build_url, "GET", &vec![]))
                .await
                .unwrap();
            let mut result = BinaryMut::new();
            res.body_mut().read_all(&mut result).await;

            println!("result = {:?}", result.chunk());
            assert_eq!(result.remaining(), HELLO_WORLD.as_bytes().len());
            assert_eq!(result.as_slice(), HELLO_WORLD.as_bytes());
            assert_eq!(res.version(), Version::Http2);
        }

        {
            let url = &*format!("http://{}/", tcp_addr.unwrap());

            let client = Client::builder()
                .http2_only(true)
                .url(&*url).unwrap()
                .connect()
                .await
                .unwrap();

            let mut res = client
                .send_now(do_build_req(url, "GET", &vec![]))
                .await
                .unwrap();
            let mut result = BinaryMut::new();
            res.body_mut().read_all(&mut result).await;

            assert_eq!(result.remaining(), HELLO_WORLD.as_bytes().len());
            assert_eq!(result.as_slice(), HELLO_WORLD.as_bytes());
            assert_eq!(res.version(), Version::Http2);
        }

        {
            let url = &*format!("http://{}/", local_server_addr);
            let client = Client::builder()
                // .http2(false)
                .http2_only(true)
                .add_proxy(&*format!("http://{}", proxy_addr.unwrap()))
                .unwrap()
                .url(&*url).unwrap()
                .connect()
                .await
                .unwrap();

            let mut res = client
                .send_now(do_build_req(url, "GET", &vec![]))
                .await
                .unwrap();
            let mut result = BinaryMut::new();
            res.body_mut().read_all(&mut result).await;
            // 测试头信息来确认是否来源于代理
            assert_eq!(res.headers().get_value(&"from_proxy"), &"mapping");
            assert_eq!(result.remaining(), HELLO_WORLD.as_bytes().len());
            assert_eq!(result.as_slice(), HELLO_WORLD.as_bytes());
            assert_eq!(res.version(), Version::Http2);
        }
    }
}
