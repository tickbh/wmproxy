use acme_lib::create_rsa_key;
use acme_lib::persist::MemoryPersist;
use acme_lib::{Directory, DirectoryUrl, Error};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use std::{
    fs::File,
    io::{self, BufReader},
    sync::Arc,
};

use lazy_static::lazy_static;
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    ServerConnection,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::{Accept, TlsAcceptor};

use crate::Helper;

lazy_static! {
    static ref CACHE_REQUEST: Mutex<HashMap<String, Instant>> = Mutex::new(HashMap::new());
}

#[derive(Clone)]
pub struct WrapTlsAccepter {
    pub last: Instant,
    pub domain: Option<String>,
    pub accepter: Option<TlsAcceptor>,
}

impl WrapTlsAccepter {
    fn load_certs(path: &Option<String>) -> io::Result<Vec<CertificateDer<'static>>> {
        if let Some(path) = path {
            match File::open(&path) {
                Ok(file) => {
                    let mut reader = BufReader::new(file);
                    let certs = rustls_pemfile::certs(&mut reader);
                    Ok(certs.into_iter().collect::<Result<Vec<_>, _>>()?)
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
                Ok(file) => {
                    {
                        let mut reader = BufReader::new(&file);
                        let mut keys = rustls_pemfile::pkcs8_private_keys(&mut reader)
                            .collect::<Result<Vec<_>, _>>()?;
                        if keys.len() == 1 {
                            return Ok(PrivateKeyDer::from(keys.remove(0)));
                        }
                    }
                    {
                        let mut reader = BufReader::new(&file);
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
            domain: Some(domain),
            accepter: None,
        };
        wrap.try_load_cert();
        wrap
    }

    pub fn try_load_cert(&mut self) -> bool {
        match Self::load_ssl(&self.get_cert_path(), &self.get_key_path()) {
            Ok(accepter) => {
                self.accepter = Some(accepter);
                true
            }
            Err(e) => {
                println!("load ssl error ={:?}", e);
                false
            }
        }
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
        if self.last.elapsed() > Duration::from_secs(5) {
            self.try_load_cert();
            self.last = Instant::now();
        }
    }

    pub fn is_wait_acme(&self) -> bool {
        self.accepter.is_none()
    }

    pub fn load_ssl(cert: &Option<String>, key: &Option<String>) -> io::Result<TlsAcceptor> {
        println!("cert = {:?}", cert);
        println!("key = {:?}", key);
        let one_key = Self::load_keys(&key)?;
        let one_cert = Self::load_certs(&cert)?;
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
        Ok(TlsAcceptor::from(Arc::new(config)))
    }

    pub fn new_cert(cert: &Option<String>, key: &Option<String>) -> io::Result<WrapTlsAccepter> {
        let config = Self::load_ssl(cert, key)?;
        Ok(WrapTlsAccepter {
            last: Instant::now(),
            domain: None,
            accepter: Some(config),
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
            Ok(a.accept_with(stream, f))
        } else {
            self.check_and_request_cert()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "load https error"))?;
            Err(io::Error::new(io::ErrorKind::Other, "try next https error"))
        }
    }

    fn check_and_request_cert(&self)  -> Result<(), Error> {
        if self.domain.is_none() {
            return Err(io::Error::new(io::ErrorKind::Other, "未知域名").into());
        }
        {
            let mut obj = CACHE_REQUEST
                .lock()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Fail get Lock"))?;
            if let Some(last) = obj.get(self.domain.as_ref().unwrap()) {
                if last.elapsed() < Duration::from_secs(30) {
                    return Err(io::Error::new(io::ErrorKind::Other, "等待上次请求结束").into());
                }
            }
            obj.insert(self.domain.clone().unwrap(), Instant::now());
        };

        let obj = self.clone();
        thread::spawn(move || {
            let _ = obj.request_cert();
        });
        Ok(())
    }

    fn request_cert(&self) -> Result<(), Error> {
        // Use DirectoryUrl::LetsEncrypStaging for dev/testing.
        let url = DirectoryUrl::LetsEncrypt;
        let path = Path::new(".well-known/acme-challenge");
        if !path.exists() {
            let _ = std::fs::create_dir_all(path);
        }

        // Save/load keys and certificates to current dir.
        let persist = MemoryPersist::new();

        // Create a directory entrypoint.
        let dir = Directory::from_url(persist, url)?;

        // Reads the private account key from persistence, or
        // creates a new one before accessing the API to establish
        // that it's there.
        let acc = dir.account("wmproxy@wmproxy.net")?;

        // Order a new TLS certificate for a domain.
        let mut ord_new = acc.new_order(&self.domain.clone().unwrap_or_default(), &[])?;

        let start = Instant::now();
        // If the ownership of the domain(s) have already been
        // authorized in a previous order, you might be able to
        // skip validation. The ACME API provider decides.
        let ord_csr = loop {
            // are we done?
            if let Some(ord_csr) = ord_new.confirm_validations() {
                break ord_csr;
            }

            if start.elapsed() > Duration::from_secs(30) {
                println!("获取证书超时");
                return Ok(());
            }

            // Get the possible authorizations (for a single domain
            // this will only be one element).
            let auths = ord_new.authorizations()?;

            // For HTTP, the challenge is a text file that needs to
            // be placed in your web server's root:
            //
            // /var/www/.well-known/acme-challenge/<token>
            //
            // The important thing is that it's accessible over the
            // web for the domain(s) you are trying to get a
            // certificate for:
            //
            // http://mydomain.io/.well-known/acme-challenge/<token>
            let chall = auths[0].http_challenge();

            // The token is the filename.
            let token = chall.http_token();
            let path = format!(".well-known/acme-challenge/{}", token);

            // The proof is the contents of the file
            let proof = chall.http_proof();

            Helper::write_to_file(&path, proof.as_bytes())?;

            // Here you must do "something" to place
            // the file/contents in the correct place.
            // update_my_web_server(&path, &proof);

            // After the file is accessible from the web, the calls
            // this to tell the ACME API to start checking the
            // existence of the proof.
            //
            // The order at ACME will change status to either
            // confirm ownership of the domain, or fail due to the
            // not finding the proof. To see the change, we poll
            // the API with 5000 milliseconds wait between.
            chall.validate(5000)?;

            // Update the state against the ACME API.
            ord_new.refresh()?;

            // return Ok(());
        };

        // Ownership is proven. Create a private key for
        // the certificate. These are provided for convenience, you
        // can provide your own keypair instead if you want.
        let pkey_pri = create_rsa_key(2048);

        // Submit the CSR. This causes the ACME provider to enter a
        // state of "processing" that must be polled until the
        // certificate is either issued or rejected. Again we poll
        // for the status change.
        let ord_cert = ord_csr.finalize_pkey(pkey_pri, 5000)?;

        // Now download the certificate. Also stores the cert in
        // the persistence.
        let cert = ord_cert.download_and_save_cert()?;
        println!(
            "cert = {}, key = {}",
            cert.certificate(),
            cert.private_key()
        );
        println!(
            "cert = {:?}, key = {:?}",
            &self.get_cert_path(),
            &self.get_cert_path()
        );
        Helper::write_to_file(
            &self.get_cert_path().unwrap(),
            cert.certificate().as_bytes(),
        )?;
        Helper::write_to_file(&self.get_key_path().unwrap(), cert.private_key().as_bytes())?;
        Ok(())
    }
}
