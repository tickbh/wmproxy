// #![deny(warnings)]
#![deny(rust_2018_idioms)]

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use tokio::sync::mpsc::{channel, Sender};
    use webparse::Request;
    use wenmeng::Client;
    use wmproxy::{ConfigOption, WMCore, ProxyConfig, ProxyResult};

    static HTTP_URL: &str = "http://www.baidu.com";
    static HTTPS_URL: &str = "https://www.baidu.com";

    async fn run_proxy(
        username: Option<String>,
        password: Option<String>,
    ) -> ProxyResult<(SocketAddr, Sender<()>)> {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let proxy = ProxyConfig::builder()
            .bind_addr(addr)
            .username(username)
            .password(password)
            .into_value()
            .unwrap();
        let option = ConfigOption::new_by_proxy(proxy);
        let (sender_close, receiver_close) = channel::<()>(1);
        let mut proxy = WMCore::new(option);
        proxy.ready_serve().await.unwrap();
        let addr = proxy.center_listener.as_ref().unwrap().local_addr()?;
        tokio::spawn(async move {
            let _ = proxy.run_serve(receiver_close, None).await;
        });
        Ok((addr, sender_close))
    }

    async fn test_proxy(
        addr: SocketAddr,
        url: &str,
        method: &str,
        auth: Option<(String, String)>,
        is_failed: bool,
    ) {
        let req = Request::builder().method("GET").url(url).body("").unwrap();
        let proxy = if auth.is_some() {
            format!(
                "{}://{}:{}@{}",
                method,
                auth.as_ref().unwrap().0,
                auth.as_ref().unwrap().1,
                addr
            )
        } else {
            format!("{}://{}", method, addr)
        };
        let client = match Client::builder()
            .add_proxy(&proxy)
            .unwrap()
            .connect(url)
            .await
        {
            Ok(client) => {
                client
            }
            Err(_) => {
                if is_failed {
                    return;
                }
                assert!(false);
                unreachable!();
            }
        };
        let mut res = match client.send_now(req.into_type()).await {
            Ok(res) => {
                if is_failed {
                    println!("status {:?}", res.status());
                    println!("body {:?}", res.body());
                    assert!(res.status() != 200);
                }
                res
            }
            Err(_) => {
                if is_failed {
                    return;
                }
                assert!(false);
                unreachable!();
            }
        };
        res.body_mut().wait_all().await;
        let res = res.into_type::<String>();
        assert_eq!(res.status(), 200);
        assert!(unsafe { res.body().as_str().get_unchecked(0..15).contains("html") });
        assert!(!is_failed);
    }

    #[tokio::test]
    async fn test_no_auth() {
        let (addr, _sender) = run_proxy(None, None).await.unwrap();
        test_proxy(addr, HTTP_URL, "http", None, false).await;
        test_proxy(addr, HTTPS_URL, "http", None, false).await;
        test_proxy(addr, HTTP_URL, "socks5", None, false).await;
    }

    #[tokio::test]
    async fn test_auth() {
        let username = "wmproxy".to_string();
        let password = "wmproxy".to_string();
        let (addr, _sender) = run_proxy(Some(username.clone()), Some(password.clone()))
            .await
            .unwrap();
        test_proxy(addr, HTTP_URL, "http", None, true).await;
        test_proxy(addr, HTTPS_URL, "http", None, true).await;
        test_proxy(addr, HTTP_URL, "socks5", None, true).await;

        let auth = Some((username, password));
        test_proxy(addr, HTTP_URL, "http", auth.clone(), false).await;
        test_proxy(addr, HTTPS_URL, "http", auth.clone(), false).await;
        test_proxy(addr, HTTP_URL, "socks5", auth.clone(), false).await;
    }
}
