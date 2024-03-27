#![deny(rust_2018_idioms)]

/// 关于代理相关
#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, thread};

    use tokio::sync::mpsc::{self, channel, Sender};
    use webparse::Request;
    use wenmeng::Client;
    use wmproxy::{core::ServiceTrait, ConfigOption, ProxyConfig, ProxyResult, WMCore};

    static HTTP_URL: &str = "http://www.baidu.com";
    static HTTPS_URL: &str = "https://www.baidu.com";

    // async fn run_server_proxy(
    //     proxy: ProxyConfig,
    // ) -> ProxyResult<(SocketAddr, Sender<()>)> {
    //     let option = ConfigOption::new_by_proxy(proxy);
    //     let addr = option.proxy.as_ref().unwrap().center_addr.unwrap().0;
    //     let (sender_close, _receiver_close) = channel::<()>(1);
    //     thread::spawn(move || {
    //         let _ = WMCore::run_main_opt(option).unwrap();
    //     });

    //     // let mut proxy = WMCore::new(option);
    //     // proxy.ready_serve().await.unwrap();
    //     // let addr = proxy.center_listener.as_ref().unwrap().local_addr()?;
    //     // tokio::spawn(async move {
    //     //     let _ = proxy.run_serve(receiver_close, None).await;
    //     // });
    //     Ok((addr, sender_close))
    // }

    
    async fn build_server_proxy(
        proxy: ProxyConfig,
    ) -> ProxyResult<(SocketAddr, Vec<Box<dyn ServiceTrait>>)> {
        let option = ConfigOption::new_by_proxy(proxy);
        let addr = option.proxy.as_ref().unwrap().center_addr.unwrap().0;
        let (sender_close, _receiver_close) = channel::<()>(1);
        let core = WMCore::new(option);
        let services = core.build_services()?;
        Ok((addr, services))
    }

    
    async fn run_server_proxy(
        proxy: ProxyConfig,
    ) -> ProxyResult<SocketAddr> {
        let services = build_server_proxy(proxy.clone()).await?;
        WMCore::run_main_service(ConfigOption::new_by_proxy(proxy), services.1).unwrap();
        // let option = ConfigOption::new_by_proxy(proxy);
        // let addr = option.proxy.as_ref().unwrap().center_addr.unwrap().0;
        // let (sender_close, _receiver_close) = channel::<()>(1);
        // let services = WMCore::build_services(option)?;
        Ok(services.0)
    }

    async fn build_proxy(
        proxy: ProxyConfig,
    ) -> ProxyResult<(SocketAddr, Vec<Box<dyn ServiceTrait>>)> {
        
        let option = ConfigOption::new_by_proxy(proxy);
        let addr = option.proxy.as_ref().unwrap().bind.unwrap().0;
        let (sender_close, _receiver_close) = channel::<()>(1);
        let core = WMCore::new(option);
        let services = core.build_services()?;

        // let option = ConfigOption::new_by_proxy(proxy);
        // let addr = option.proxy.as_ref().unwrap().bind.unwrap().0;
        // let (sender_close, _receiver_close) = channel::<()>(1);
        // thread::spawn(move || {
        //     let _ = WMCore::run_main_opt(option).unwrap();
        // });
        // let option = ConfigOption::new_by_proxy(proxy);
        // let (sender_close, receiver_close) = channel::<()>(1);
        // let mut proxy = WMCore::new(option);
        // proxy.ready_serve().await.unwrap();
        // let addr = proxy.client_listener.as_ref().unwrap().local_addr()?;
        // tokio::spawn(async move {
        //     let _ = proxy.run_serve(receiver_close, None).await;
        // });
        Ok((addr, services))
    }

    
    async fn run_proxy(
        proxy: ProxyConfig,
    ) -> ProxyResult<SocketAddr> {
        let services = build_proxy(proxy.clone()).await?;
        thread::spawn(move || {
                WMCore::run_main_service(ConfigOption::new_by_proxy(proxy), services.1).unwrap();
        });

        // let option = ConfigOption::new_by_proxy(proxy);
        // let addr = option.proxy.as_ref().unwrap().center_addr.unwrap().0;
        // let (sender_close, _receiver_close) = channel::<()>(1);
        // let services = WMCore::build_services(option)?;
        Ok(services.0)
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
        let addr = "127.0.0.1:54123".parse().unwrap();
        let proxy = ProxyConfig::builder()
            .bind(addr)
            .into_value()
            .unwrap();

        let addr = run_proxy(proxy).await.unwrap();
        test_proxy(addr, HTTP_URL, "http", None, false).await;
        test_proxy(addr, HTTPS_URL, "http", None, false).await;
        test_proxy(addr, HTTP_URL, "socks5", None, false).await;
    }

    
    #[tokio::test]
    async fn test_auth() {
        let addr = "127.0.0.1:54124".parse().unwrap();
        let username = "wmproxy".to_string();
        let password = "wmproxy".to_string();
        let proxy = ProxyConfig::builder()
            .bind(addr)
            .username(Some(username.clone()))
            .password(Some(password.clone()))
            .into_value()
            .unwrap();

        let addr = run_proxy(proxy)
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
        let center_addr = "127.0.0.1:54125".parse().unwrap();
        let bind_addr = "127.0.0.1:54126".parse().unwrap();
        let username = "wmproxy".to_string();
        let password = "wmproxy".to_string();

        let proxy = ProxyConfig::builder()
            .center_addr(center_addr)
            .username(Some(username.clone()))
            .password(Some(password.clone()))
            .into_value()
            .unwrap();

        let (server_addr, mut servivce1) = build_server_proxy(proxy)
            .await
            .unwrap();
        
        let proxy = ProxyConfig::builder()
            .bind(bind_addr)
            .username(Some(username.clone()))
            .password(Some(password.clone()))
            .server(Some(format!("{}", server_addr)))
            .into_value()
            .unwrap();
        let proxy_clone = proxy.clone();
        let (addr, mut servivce2) = build_proxy(proxy)
            .await
            .unwrap();

        servivce1.append(&mut servivce2);

        let (tx, rx) = mpsc::channel(1);
        thread::spawn(move || {
            let mut core = WMCore::new(ConfigOption::new_by_proxy(proxy_clone));
            core.server.add_services(servivce1);
            let _ = core.run_server_with_recv(rx);
        });

        test_proxy(addr, HTTP_URL, "http", None, true).await;
        test_proxy(addr, HTTPS_URL, "http", None, true).await;
        test_proxy(addr, HTTP_URL, "socks5", None, true).await;

        let auth = Some((username, password));
        test_proxy(addr, HTTP_URL, "http", auth.clone(), false).await;
        test_proxy(addr, HTTPS_URL, "http", auth.clone(), false).await;
        test_proxy(addr, HTTP_URL, "socks5", auth.clone(), false).await;
        let _  = tx.send(()).await;
    }

}
