// #![deny(warnings)]
#![deny(rust_2018_idioms)]

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr};

    use tokio::sync::mpsc::{channel, Sender};
    use webparse::Request;
    use wenmeng::Client;
    use wmproxy::{ConfigOption, ProxyResult, ProxyConfig, Proxy};
    
    static HTTP_URL: &str = "http://www.baidu.com";
    static HTTPS_URL: &str = "https://www.baidu.com";
    
    
    async fn run_proxy() -> ProxyResult<(SocketAddr, Sender<()>)> {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let proxy = ProxyConfig::builder().bind_addr(addr).into_value().unwrap();
        let option = ConfigOption::new_by_proxy(proxy);
        let (sender_close, receiver_close) = channel::<()>(1);
        let mut proxy = Proxy::new(option);
        proxy.ready_serve().await.unwrap();
        let addr = proxy.center_listener.as_ref().unwrap().local_addr()?;
        tokio::spawn(async move {
            let _ = proxy.run_serve(receiver_close, None).await;
        });
        Ok((addr, sender_close))
    }

    async fn test_proxy(addr: SocketAddr, url: &str, method: &str, auth: Option<(String, String)>) {

        let req = Request::builder().method("GET").url(url).body("").unwrap();
        let proxy = if auth.is_some() {
            format!("{}://{}:{}@{}", method, auth.as_ref().unwrap().0, auth.as_ref().unwrap().1, addr)
        } else {
            format!("{}://{}", method, addr)
        };
        let client = Client::builder()
            .add_proxy(&proxy).unwrap()
            .connect(url).await.unwrap();
        let mut res = client.send_now(req.into_type()).await.unwrap();
        res.body_mut().wait_all().await;
        let res = res.into_type::<String>();
        assert_eq!(res.status(), 200);
        assert!(unsafe {
            res.body().as_str().get_unchecked(0..15).contains("html")
        });
    }


    macro_rules! create_proxy {
        () => {
        };
    }


    #[tokio::test]
    async fn my_test1() {
        let (addr, _sender) = run_proxy().await.unwrap();
        test_proxy(addr, HTTP_URL, "http", None).await;
        test_proxy(addr, HTTPS_URL, "http", None).await;
        test_proxy(addr, HTTP_URL, "socks5", None).await;
    }
}
