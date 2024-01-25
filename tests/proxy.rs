#![deny(rust_2018_idioms)]

/// 关于代理相关
#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use tokio::sync::mpsc::{channel, Sender};
    use webparse::Request;
    use wenmeng::Client;
    use wmproxy::{ConfigOption, WMCore, ProxyConfig, ProxyResult};

    static HTTP_URL: &str = "http://www.baidu.com";
    static HTTPS_URL: &str = "https://www.baidu.com";

    async fn run_server_proxy(
        proxy: ProxyConfig,
    ) -> ProxyResult<(SocketAddr, Sender<()>)> {
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

    async fn run_proxy(
        proxy: ProxyConfig,
    ) -> ProxyResult<(SocketAddr, Sender<()>)> {
        let option = ConfigOption::new_by_proxy(proxy);
        let (sender_close, receiver_close) = channel::<()>(1);
        let mut proxy = WMCore::new(option);
        proxy.ready_serve().await.unwrap();
        let addr = proxy.client_listener.as_ref().unwrap().local_addr()?;
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
            .url(&*url).unwrap()
            .connect()
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
                    assert!(res.status() != 200);
                    return ;
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
        let addr = "127.0.0.1:0".parse().unwrap();
        let proxy = ProxyConfig::builder()
            .bind(addr)
            .into_value()
            .unwrap();

        let (addr, _sender) = run_proxy(proxy).await.unwrap();
        test_proxy(addr, HTTP_URL, "http", None, false).await;
        test_proxy(addr, HTTPS_URL, "http", None, false).await;
        test_proxy(addr, HTTP_URL, "socks5", None, false).await;
    }

    
    #[tokio::test]
    async fn test_auth() {
        let addr = "127.0.0.1:0".parse().unwrap();
        let username = "wmproxy".to_string();
        let password = "wmproxy".to_string();
        let proxy = ProxyConfig::builder()
            .bind(addr)
            .username(Some(username.clone()))
            .password(Some(password.clone()))
            .into_value()
            .unwrap();

        let (addr, _sender) = run_proxy(proxy)
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

    
    #[tokio::test]
    async fn test_client_server_auth() {
        let addr = "127.0.0.1:0".parse().unwrap();
        let username = "wmproxy".to_string();
        let password = "wmproxy".to_string();

        let proxy = ProxyConfig::builder()
            .center_addr(addr)
            .username(Some(username.clone()))
            .password(Some(password.clone()))
            .into_value()
            .unwrap();

        let (server_addr, _sender) = run_server_proxy(proxy)
            .await
            .unwrap();
        
        let proxy = ProxyConfig::builder()
            .bind(addr)
            .username(Some(username.clone()))
            .password(Some(password.clone()))
            .server(Some(format!("{}", server_addr)))
            .into_value()
            .unwrap();

        let (addr, _sender) = run_proxy(proxy)
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
