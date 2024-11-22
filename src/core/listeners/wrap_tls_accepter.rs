#[cfg(feature = "acme-lib")]
use acme_lib::{create_rsa_key, persist::MemoryPersist, Directory, DirectoryUrl, Error};
use chrono::{DateTime, Utc};
use rustls::crypto::ring::sign;
use rustls::server::ResolvesServerCertUsingSni;
use rustls::sign::CertifiedKey;
use std::collections::HashMap;
use std::io::Read;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::{
    fs::File,
    io::{self, BufReader},
    sync::Arc,
};
use x509_certificate::X509Certificate;

use lazy_static::lazy_static;
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    ServerConnection,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::{Accept, TlsAcceptor};

lazy_static! {
    // 请求时间做记录,防止短时间内重复请求
    static ref CACHE_REQUEST: Mutex<HashMap<String, Instant>> = Mutex::new(HashMap::new());

    // 成功加载时间记录,以方便将过期的数据做更新
    static ref SUCCESS_CERT: Mutex<HashMap<String, Instant>> = Mutex::new(HashMap::new());

    // 初始申请新证书延时
    static ref REQUEST_NEW_DELAY: Duration = Duration::from_secs(30);

    // 过期证书更新延时
    static ref REQUEST_UPDATE_DELAY: Duration = Duration::from_secs(300);
}

/// 包装tls accepter, 用于适应acme及自有证书两种
#[derive(Clone)]
pub struct WrapTlsAccepter {
    /// 最后请求的时间
    pub last: Instant,
    /// 最后成功加载证书的时间
    pub last_success: Instant,
    /// 域名
    pub domain: Option<String>,
    pub accepter: Option<TlsAcceptor>,
    /// 证书的过期时间,将加载证书的时候同步读取
    pub expired: Option<DateTime<Utc>>,
    pub is_acme: bool,
}

impl WrapTlsAccepter {
    pub fn with_accepter(accepter: TlsAcceptor) -> Self {
        Self {
            last: Instant::now(),
            last_success: Instant::now(),
            domain: None,
            accepter: Some(accepter),
            expired: None,
            is_acme: false,
        }
    }
    
    fn load_certs(
        path: &Option<String>,
    ) -> io::Result<(DateTime<Utc>, Vec<CertificateDer<'static>>)> {
        if let Some(path) = path {
            match File::open(&path) {
                Ok(mut file) => {
                    let mut content = String::new();
                    file.read_to_string(&mut content)?;
                    let mut reader = BufReader::new(content.as_bytes());
                    let certs = rustls_pemfile::certs(&mut reader);
                    let cert = X509Certificate::from_pem(content.as_bytes())
                        .map_err(|_| io::Error::new(io::ErrorKind::Other, "cert error"))?;
                    Ok((
                        cert.validity_not_after(),
                        certs.into_iter().collect::<Result<Vec<_>, _>>()?,
                    ))
                }
                Err(e) => {
                    log::warn!("加载公钥{}出错，错误内容:{:?}", path, e);
                    return Err(e);
                }
            }
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "unknow certs"))
        }
    }

    fn load_keys(path: &Option<String>) -> io::Result<PrivateKeyDer<'static>> {
        if let Some(path) = path {
            match File::open(&path) {
                Ok(mut file) => {
                    let mut content = String::new();
                    file.read_to_string(&mut content)?;
                    {
                        let mut reader = BufReader::new(content.as_bytes());
                        let mut keys = rustls_pemfile::pkcs8_private_keys(&mut reader)
                            .collect::<Result<Vec<_>, _>>()?;
                        if keys.len() == 1 {
                            return Ok(PrivateKeyDer::from(keys.remove(0)));
                        }
                    }
                    {
                        let mut reader = BufReader::new(content.as_bytes());
                        let mut keys = rustls_pemfile::rsa_private_keys(&mut reader)
                            .collect::<Result<Vec<_>, _>>()?;
                        if keys.len() == 1 {
                            return Ok(PrivateKeyDer::from(keys.remove(0)));
                        }
                    }
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("No pkcs8 or rsa private key found"),
                    ));
                }
                Err(e) => {
                    log::warn!("加载私钥{}出错，错误内容:{:?}", path, e);
                    return Err(e);
                }
            }
        } else {
            return Err(io::Error::new(io::ErrorKind::Other, "unknow keys"));
        };
    }

    pub fn new(domain: String) -> WrapTlsAccepter {
        let mut wrap = WrapTlsAccepter {
            last: Instant::now(),
            last_success: Instant::now(),
            domain: Some(domain),
            accepter: None,
            expired: None,
            is_acme: true,
        };
        wrap.try_load_cert();
        wrap
    }

    pub fn try_load_cert(&mut self) -> bool {
        let succ = match Self::load_ssl(&self.get_cert_path(), &self.get_key_path()) {
            Ok((expired, accepter)) => {
                self.accepter = Some(accepter);
                self.expired = Some(expired);
                true
            }
            Err(e) => {
                println!("load ssl error ={:?}", e);
                false
            }
        };
        if succ {
            self.last_success = Instant::now();
        }
        succ
    }

    pub fn get_cert_path(&self) -> Option<String> {
        if let Some(domain) = &self.domain {
            Some(format!(".well-known/{}.pem", domain))
        } else {
            None
        }
    }

    pub fn get_key_path(&self) -> Option<String> {
        if let Some(domain) = &self.domain {
            Some(format!(".well-known/{}.key", domain))
        } else {
            None
        }
    }

    pub fn update_last(&mut self) {
        if self.accepter.is_none() {
            if self.last.elapsed() > Duration::from_secs(5) {
                self.try_load_cert();
                self.last = Instant::now();
            }
        } else {
            if self.domain.is_none() {
                return;
            }
            let map = SUCCESS_CERT.lock().unwrap();
            let doamin = &self.domain.clone().unwrap();
            if !map.contains_key(doamin) {
                return;
            }
            if self.last_success < map[doamin] && self.last < map[doamin] {
                self.try_load_cert();
                self.last = map[doamin];
            }
        }
    }

    pub fn is_wait_acme(&self) -> bool {
        self.accepter.is_none()
    }

    pub fn load_ssl(
        cert: &Option<String>,
        key: &Option<String>,
    ) -> io::Result<(DateTime<Utc>, TlsAcceptor)> {
        let (expired, one_cert) = Self::load_certs(&cert)?;
        let one_key = Self::load_keys(&key)?;
        let config = rustls::ServerConfig::builder();
        let mut config = config
            .with_no_client_auth()
            .with_single_cert(one_cert, one_key)
            .map_err(|e| {
                log::warn!("添加证书时失败:{:?}", e);
                io::Error::new(io::ErrorKind::Other, "key error")
            })?;
        config.alpn_protocols.push("h2".as_bytes().to_vec());
        config.alpn_protocols.push("http/1.1".as_bytes().to_vec());
        Ok((expired, TlsAcceptor::from(Arc::new(config))))
    }

    pub fn new_multi(infos: Vec<(String, String, String)>) -> io::Result<WrapTlsAccepter> {
        let config = rustls::ServerConfig::builder();
        let mut resolve = ResolvesServerCertUsingSni::new();
        for (domain, cert, key) in infos {
            let (_expired, one_cert) = Self::load_certs(&Some(cert))?;
            let key = sign::any_supported_type(&Self::load_keys(&Some(key))?)
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "unvaild key"))?;
            let ck = CertifiedKey::new(one_cert, key);

            resolve.add(&domain, ck).map_err(|e| {
                println!("{:?}", e);
                io::Error::new(io::ErrorKind::Other, "unvaild key")
            })?;
        }

        let config = config
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(resolve));
        let accepter = TlsAcceptor::from(Arc::new(config));
        Ok(WrapTlsAccepter {
            last: Instant::now(),
            last_success: Instant::now(),
            domain: None,
            accepter: Some(accepter),
            expired: None,
            is_acme: false,
        })
    }

    pub fn new_cert(cert: &Option<String>, key: &Option<String>) -> io::Result<WrapTlsAccepter> {
        let (expired, config) = Self::load_ssl(cert, key)?;
        Ok(WrapTlsAccepter {
            last: Instant::now(),
            last_success: Instant::now(),
            domain: None,
            accepter: Some(config),
            expired: Some(expired),
            is_acme: false,
        })
    }

    #[inline]
    pub fn accept<IO>(&self, stream: IO) -> io::Result<Accept<IO>>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        self.accept_with(stream, |_| ())
    }

    pub fn accept_with<IO, F>(&self, stream: IO, f: F) -> io::Result<Accept<IO>>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
        F: FnOnce(&mut ServerConnection),
    {
        if let Some(a) = &self.accepter {
            if self.is_acme && self.is_tls_will_expired() {
                let _ = self.check_and_request_cert();
            }
            Ok(a.accept_with(stream, f))
        } else {
            self.check_and_request_cert()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "load https error"))?;
            Err(io::Error::new(io::ErrorKind::Other, "try next https error"))
        }
    }

    fn is_tls_will_expired(&self) -> bool {
        if let Some(expire) = &self.expired {
            let now = Utc::now();
            if now.timestamp() > expire.timestamp() - 86400 {
                return true;
            }
        }
        false
    }

    #[cfg(feature = "acme-lib")]
    fn get_delay_time(&self) -> Duration {
        if self.accepter.is_some() {
            *REQUEST_UPDATE_DELAY
        } else {
            *REQUEST_NEW_DELAY
        }
    }

    #[cfg(not(feature = "acme-lib"))]
    fn check_and_request_cert(&self) -> Result<(), io::Error> {
        Ok(())
    }

    #[cfg(feature = "acme-lib")]
    fn check_and_request_cert(&self) -> Result<(), Error> {
        if self.domain.is_none() {
            return Err(io::Error::new(io::ErrorKind::Other, "未知域名").into());
        }
        {
            let mut map = CACHE_REQUEST
                .lock()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Fail get Lock"))?;
            if let Some(last) = map.get(self.domain.as_ref().unwrap()) {
                if last.elapsed() < self.get_delay_time() {
                    return Err(io::Error::new(io::ErrorKind::Other, "等待上次请求结束").into());
                }
            }
            map.insert(self.domain.clone().unwrap(), Instant::now());
        };

        let obj = self.clone();
        std::thread::spawn(move || {
            let _ = obj.request_cert();
        });
        Ok(())
    }

    #[cfg(feature = "acme-lib")]
    fn request_cert(&self) -> Result<(), Error> {
        // 使用let's encrypt签发证书
        let url = DirectoryUrl::LetsEncrypt;
        let path = std::path::Path::new(".well-known/acme-challenge");
        if !path.exists() {
            let _ = std::fs::create_dir_all(path);
        }

        // 使用内存的存储结构，存储自己做处理
        let persist = MemoryPersist::new();

        // 创建目录节点
        let dir = Directory::from_url(persist, url)?;

        // 设置请求的email信息
        let acc = dir.account("wmproxy@wmproxy.net")?;

        // 请求签发的域名
        let mut ord_new = acc.new_order(&self.domain.clone().unwrap_or_default(), &[])?;

        let start = Instant::now();
        // 以下域名的鉴权，需要等待let's encrypt确认信息
        let ord_csr = loop {
            // 成功签发，跳出循环
            if let Some(ord_csr) = ord_new.confirm_validations() {
                break ord_csr;
            }

            // 超时30秒，认为失败了
            if start.elapsed() > self.get_delay_time() {
                println!("获取证书超时");
                return Ok(());
            }

            // 获取鉴权方式
            let auths = ord_new.authorizations()?;

            // 以下是HTTP的请求方法，本质上是请求token的url，然后返回正确的值
            // 此处我们用的是临时服务器
            //
            // /var/www/.well-known/acme-challenge/<token>
            //
            // http://mydomain.io/.well-known/acme-challenge/<token>
            let chall = auths[0].http_challenge();

            // 将token存储在目录下
            let token = chall.http_token();
            let path = format!(".well-known/acme-challenge/{}", token);

            // 获取token的内容
            let proof = chall.http_proof();

            crate::Helper::write_to_file(&path, proof.as_bytes())?;

            // 等待acme检测时间，以ms计
            chall.validate(5000)?;

            // 再尝试刷新acme请求
            ord_new.refresh()?;
        };

        // 创建rsa的密钥对
        let pkey_pri = create_rsa_key(2048);

        // 提交CSR获取最终的签名
        let ord_cert = ord_csr.finalize_pkey(pkey_pri, 5000)?;

        // 下载签名及证书，此时下载下来的为pkcs#8证书格式
        let cert = ord_cert.download_and_save_cert()?;
        crate::Helper::write_to_file(
            &self.get_cert_path().unwrap(),
            cert.certificate().as_bytes(),
        )?;
        crate::Helper::write_to_file(&self.get_key_path().unwrap(), cert.private_key().as_bytes())?;
        let mut map = SUCCESS_CERT.lock().unwrap();
        let doamin = self.domain.clone().unwrap();
        map.insert(doamin, Instant::now());
        Ok(())
    }
}
