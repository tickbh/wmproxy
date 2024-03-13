// Copyright 2022 - 2023 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Author: tickbh
// -----
// Created Date: 2023/09/25 10:42:02

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{self, BufReader},
    net::{IpAddr, SocketAddr},
    process,
    sync::Arc,
    time::Duration,
};

use bpaf::*;
use log::LevelFilter;
use rustls::{pki_types::{CertificateDer, PrivateKeyDer}, ClientConfig};

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use tokio::net::TcpListener;
use tokio_rustls::{rustls, TlsAcceptor};

use crate::{
    reverse::{HttpConfig, StreamConfig, UpstreamConfig},
    CenterClient, Flag, Helper, MappingConfig, OneHealth, ProxyError, ProxyResult, WrapAddr,
};

pub struct Builder {
    inner: ProxyResult<ProxyConfig>,
}

impl Builder {
    #[inline]
    pub fn new() -> Builder {
        Builder {
            inner: Ok(ProxyConfig::default()),
        }
    }

    pub fn flag(self, flag: Flag) -> Builder {
        self.and_then(|mut proxy| {
            proxy.flag = flag;
            Ok(proxy)
        })
    }

    // pub fn mode(self, mode: String) -> Builder {
    //     self.and_then(|mut proxy| {
    //         proxy.mode = mode;
    //         Ok(proxy)
    //     })
    // }

    pub fn add_flag(self, flag: Flag) -> Builder {
        self.and_then(|mut proxy| {
            proxy.flag.set(flag, true);
            Ok(proxy)
        })
    }

    pub fn bind(self, addr: SocketAddr) -> Builder {
        self.and_then(|mut proxy| {
            proxy.bind = Some(WrapAddr(addr));
            Ok(proxy)
        })
    }

    pub fn center_addr(self, addr: SocketAddr) -> Builder {
        self.and_then(|mut proxy| {
            proxy.center_addr = Some(WrapAddr(addr));
            Ok(proxy)
        })
    }

    pub fn server(self, addr: Option<String>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.server = addr;
            Ok(proxy)
        })
    }

    pub fn ts(self, is_tls: bool) -> Builder {
        self.and_then(|mut proxy| {
            proxy.ts = is_tls;
            Ok(proxy)
        })
    }

    pub fn tc(self, is_tls: bool) -> Builder {
        self.and_then(|mut proxy| {
            proxy.tc = is_tls;
            Ok(proxy)
        })
    }

    pub fn cert(self, cert: Option<String>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.cert = cert;
            Ok(proxy)
        })
    }

    pub fn key(self, key: Option<String>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.key = key;
            Ok(proxy)
        })
    }

    pub fn domain(self, domain: Option<String>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.domain = domain;
            Ok(proxy)
        })
    }

    pub fn username(self, username: Option<String>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.username = username;
            Ok(proxy)
        })
    }

    pub fn password(self, password: Option<String>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.password = password;
            Ok(proxy)
        })
    }

    pub fn udp_bind(self, udp_bind: Option<IpAddr>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.udp_bind = udp_bind;
            Ok(proxy)
        })
    }

    pub fn map_http_bind(self, map_http_bind: Option<SocketAddr>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.map_http_bind = map_http_bind;
            Ok(proxy)
        })
    }

    pub fn map_https_bind(self, map_https_bind: Option<SocketAddr>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.map_https_bind = map_https_bind;
            Ok(proxy)
        })
    }

    pub fn map_tcp_bind(self, map_tcp_bind: Option<SocketAddr>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.map_tcp_bind = map_tcp_bind;
            Ok(proxy)
        })
    }

    pub fn map_proxy_bind(self, map_proxy_bind: Option<SocketAddr>) -> Builder {
        self.and_then(|mut proxy| {
            proxy.map_proxy_bind = map_proxy_bind;
            Ok(proxy)
        })
    }

    pub fn mapping(self, mapping: MappingConfig) -> Builder {
        self.and_then(|mut proxy| {
            proxy.mappings.push(mapping);
            Ok(proxy)
        })
    }

    fn and_then<F>(self, func: F) -> Self
    where
        F: FnOnce(ProxyConfig) -> ProxyResult<ProxyConfig>,
    {
        Builder {
            inner: self.inner.and_then(func),
        }
    }

    pub fn into_value(self) -> ProxyResult<ProxyConfig> {
        self.inner
    }
}

fn default_bind_addr() -> SocketAddr {
    "127.0.0.1:8090".parse().unwrap()
}

/// 代理类, 一个代理类启动一种类型的代理
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, Bpaf)]
pub struct ProxyConfig {
    /// 代理id
    #[bpaf(fallback(0), display_fallback, short('s'), long)]
    #[serde(default)]
    pub(crate) server_id: u32,

    /// 代理绑定端口地址
    #[bpaf(short('b'), long)]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub(crate) bind: Option<WrapAddr>,

    /// 中心代理绑定端口地址
    #[bpaf(short('c'), long)]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub(crate) center_addr: Option<WrapAddr>,

    /// 代理种类, 如http https socks5
    #[bpaf(fallback(Flag::default()))]
    #[serde_as(as = "DisplayFromStr")]
    #[serde(default)]
    pub(crate) flag: Flag,

    /// 连接代理服务端地址
    #[bpaf(short('S'), long("server"))]
    pub(crate) server: Option<String>,
    /// 用于socks验证及中心服务器验证
    #[bpaf(long("user"))]
    pub(crate) username: Option<String>,
    /// 用于socks验证及中心服务器验证
    #[bpaf(long("pass"))]
    pub(crate) password: Option<String>,
    /// udp的绑定地址
    pub(crate) udp_bind: Option<IpAddr>,
    /// 内网http的映射地址
    pub(crate) map_http_bind: Option<SocketAddr>,
    /// 内网https的映射地址
    pub(crate) map_https_bind: Option<SocketAddr>,
    /// 内网tcp的映射地址
    pub(crate) map_tcp_bind: Option<SocketAddr>,
    /// 内网代理的映射地址
    pub(crate) map_proxy_bind: Option<SocketAddr>,
    /// 内网映射的证书cert
    pub(crate) map_cert: Option<String>,
    /// 内网映射的证书key
    pub(crate) map_key: Option<String>,

    /// 连接服务端是否启用tls
    #[serde(default)]
    pub(crate) ts: bool,
    /// 接收客户端是否启用tls
    #[serde(default)]
    pub(crate) tc: bool,
    /// 双向认证是否启用
    #[serde(default)]
    pub(crate) two_way_tls: bool,
    /// tls证书所用的域名
    pub(crate) domain: Option<String>,
    /// 公开的证书公钥文件
    pub(crate) cert: Option<String>,
    /// 隐私的证书私钥文件
    pub(crate) key: Option<String>,
    #[serde(default)]
    pub(crate) mappings: Vec<MappingConfig>,
}

pub fn default_control_port() -> SocketAddr {
    "127.0.0.1:8837".parse().unwrap()
}

pub fn default_pidfile() -> String {
    "wmproxy.pid".to_string()
}
#[serde_as]
/// 代理类, 一个代理类启动一种类型的代理
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigOption {
    /// HTTP反向代理,静态文件服相关
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    /// HTTP反向代理,静态文件服相关
    #[serde(default)]
    pub http: Option<HttpConfig>,
    #[serde(default)]
    pub stream: Option<StreamConfig>,
    #[serde(default = "default_control_port")]
    pub control: SocketAddr,
    #[serde(default)]
    pub disable_stdout: bool,
    #[serde(default)]
    pub disable_control: bool,
    #[serde(default="default_pidfile")]
    pub pidfile: String,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub default_level: Option<LevelFilter>,
}

impl Default for ConfigOption {
    fn default() -> Self {
        Self {
            proxy: Default::default(),
            http: Default::default(),
            stream: Default::default(),
            control: default_control_port(),
            disable_stdout: Default::default(),
            disable_control: Default::default(),
            default_level: None,
            pidfile: default_pidfile(),
        }
    }
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            server_id: 0,
            flag: Flag::HTTP | Flag::HTTPS | Flag::SOCKS5,
            // mode: "client".to_string(),
            bind: Some(WrapAddr(default_bind_addr())),
            center_addr: None,
            server: None,
            username: None,
            password: None,
            udp_bind: None,
            map_http_bind: None,
            map_https_bind: None,
            map_tcp_bind: None,
            map_proxy_bind: None,
            map_cert: None,
            map_key: None,

            ts: false,
            tc: false,
            two_way_tls: false,
            domain: None,
            cert: None,
            key: None,

            mappings: vec![],
        }
    }
}

impl ProxyConfig {
    pub fn builder() -> Builder {
        Builder::new()
    }

    fn load_certs(path: &Option<String>) -> io::Result<Vec<CertificateDer<'static>>> {
        if let Some(path) = path {
            let file = File::open(path)?;
            let mut reader = BufReader::new(file);
            let certs = rustls_pemfile::certs(&mut reader);
            Ok(certs.into_iter().collect::<Result<Vec<_>, _>>()?)
        } else {
            let cert = br"-----BEGIN CERTIFICATE-----
MIIF+zCCBOOgAwIBAgIQCkkcvmucB5JXt9JAehuNqTANBgkqhkiG9w0BAQsFADBu
MQswCQYDVQQGEwJVUzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3
d3cuZGlnaWNlcnQuY29tMS0wKwYDVQQDEyRFbmNyeXB0aW9uIEV2ZXJ5d2hlcmUg
RFYgVExTIENBIC0gRzIwHhcNMjMwOTIwMDAwMDAwWhcNMjQwOTIwMjM1OTU5WjAc
MRowGAYDVQQDExFzb2Z0LndtLXByb3h5LmNvbTCCASIwDQYJKoZIhvcNAQEBBQAD
ggEPADCCAQoCggEBAMO4HYTBMKeWpePVeG9w5MD1lGvwImsaOXZPPiVhE2iH1PC/
cOgYimSVkaPDMREn4mboPpJz/rZZZEkw0s3t7u2boGPcLocyA5JrEhz/SCr4CWhh
Kxn26NjTazqjGH7rkTIswLCHht0R8QqVW4n4Ikg9o9xttrucYF8fuJvWsCnWL/FC
TwGvoSa253bxXCJ2jpdhSBSS+BwYfdJHoB1j3+LRiT8/HFA0PSg2e3kjnYRtiSaH
8kJpu5l4MZJaBeYqp7/BzJlX7D7hlgdEnfEXBHFG8JfdpvrgrClzM1TDj0Vjt9Kc
Q2Ye+n4zaJ8/oCkJanXlZSfK8fLL20B+pmhuDYcCAwEAAaOCAuUwggLhMB8GA1Ud
IwQYMBaAFHjfkZBf7t6s9sV169VMVVPvJEq2MB0GA1UdDgQWBBSftBOi929JcrJy
D7WHstVroEbsLjAcBgNVHREEFTATghFzb2Z0LndtLXByb3h5LmNvbTA+BgNVHSAE
NzA1MDMGBmeBDAECATApMCcGCCsGAQUFBwIBFhtodHRwOi8vd3d3LmRpZ2ljZXJ0
LmNvbS9DUFMwDgYDVR0PAQH/BAQDAgWgMB0GA1UdJQQWMBQGCCsGAQUFBwMBBggr
BgEFBQcDAjCBgAYIKwYBBQUHAQEEdDByMCQGCCsGAQUFBzABhhhodHRwOi8vb2Nz
cC5kaWdpY2VydC5jb20wSgYIKwYBBQUHMAKGPmh0dHA6Ly9jYWNlcnRzLmRpZ2lj
ZXJ0LmNvbS9FbmNyeXB0aW9uRXZlcnl3aGVyZURWVExTQ0EtRzIuY3J0MAwGA1Ud
EwEB/wQCMAAwggF/BgorBgEEAdZ5AgQCBIIBbwSCAWsBaQB3AO7N0GTV2xrOxVy3
nbTNE6Iyh0Z8vOzew1FIWUZxH7WbAAABirCUmIAAAAQDAEgwRgIhAPPLhOxe+6gj
ywDOzmDW8dL8MUzb/StZQex5/KZ4zezOAiEA9WCnxbr3P/sJmTSqqtgPBQGIa/st
xg92qmSh9oOsXikAdgBIsONr2qZHNA/lagL6nTDrHFIBy1bdLIHZu7+rOdiEcwAA
AYqwlJiSAAAEAwBHMEUCIHarznxsuYuJ7ErhF+BfpjjLui/eyFBC24JhtqhdHJ1H
AiEAsfRfxg8x+F87dpbAP2VGQxJM0ycbVSk2W4pyX6/get4AdgDatr9rP7W2Ip+b
wrtca+hwkXFsu1GEhTS9pD0wSNf7qwAAAYqwlJhpAAAEAwBHMEUCIBckLfu2WU/4
CL9lT8SptA0WOGExlre6BIOilSU7DJd+AiEA0FhK1/Ar0pBLhW259HXqLOH64n9c
IOvFhW+vpwigZO4wDQYJKoZIhvcNAQELBQADggEBAEjZu3ogsPBZ1m6b4FdvXQ6x
l1DMulPFtjtNU56cFxa1J2C5X08OZililg7uvBUSaqt9TRHNq+SEeyE6YlVVwSbZ
7Jcc4aC/ZEk7m3qHJllQiGt3Br+H1Erhwr9fx0FAW8A7YPxj3QKpp1tjH/wbQR7i
KAGn9FipIhtW68gMBH9OR+2e2lcY24IUrTJ6l47jrK3aq1Izl0SSQqobdqX2hSyx
KSiBV2ZVIpKHuxCtgp4VuNacoVJ9aDHIRZ87UdCB82Trui7oao8B5D7DDl89RqQs
DawEK+lxC9RRlhv6thcVWle8oNX3r0FrfTDcmLm2NhWTOi894QLyDBj7pQ8hKXE=
-----END CERTIFICATE-----
-----BEGIN CERTIFICATE-----
MIIEqjCCA5KgAwIBAgIQDeD/te5iy2EQn2CMnO1e0zANBgkqhkiG9w0BAQsFADBh
MQswCQYDVQQGEwJVUzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3
d3cuZGlnaWNlcnQuY29tMSAwHgYDVQQDExdEaWdpQ2VydCBHbG9iYWwgUm9vdCBH
MjAeFw0xNzExMjcxMjQ2NDBaFw0yNzExMjcxMjQ2NDBaMG4xCzAJBgNVBAYTAlVT
MRUwEwYDVQQKEwxEaWdpQ2VydCBJbmMxGTAXBgNVBAsTEHd3dy5kaWdpY2VydC5j
b20xLTArBgNVBAMTJEVuY3J5cHRpb24gRXZlcnl3aGVyZSBEViBUTFMgQ0EgLSBH
MjCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAO8Uf46i/nr7pkgTDqnE
eSIfCFqvPnUq3aF1tMJ5hh9MnO6Lmt5UdHfBGwC9Si+XjK12cjZgxObsL6Rg1njv
NhAMJ4JunN0JGGRJGSevbJsA3sc68nbPQzuKp5Jc8vpryp2mts38pSCXorPR+sch
QisKA7OSQ1MjcFN0d7tbrceWFNbzgL2csJVQeogOBGSe/KZEIZw6gXLKeFe7mupn
NYJROi2iC11+HuF79iAttMc32Cv6UOxixY/3ZV+LzpLnklFq98XORgwkIJL1HuvP
ha8yvb+W6JislZJL+HLFtidoxmI7Qm3ZyIV66W533DsGFimFJkz3y0GeHWuSVMbI
lfsCAwEAAaOCAU8wggFLMB0GA1UdDgQWBBR435GQX+7erPbFdevVTFVT7yRKtjAf
BgNVHSMEGDAWgBROIlQgGJXm427mD/r6uRLtBhePOTAOBgNVHQ8BAf8EBAMCAYYw
HQYDVR0lBBYwFAYIKwYBBQUHAwEGCCsGAQUFBwMCMBIGA1UdEwEB/wQIMAYBAf8C
AQAwNAYIKwYBBQUHAQEEKDAmMCQGCCsGAQUFBzABhhhodHRwOi8vb2NzcC5kaWdp
Y2VydC5jb20wQgYDVR0fBDswOTA3oDWgM4YxaHR0cDovL2NybDMuZGlnaWNlcnQu
Y29tL0RpZ2lDZXJ0R2xvYmFsUm9vdEcyLmNybDBMBgNVHSAERTBDMDcGCWCGSAGG
/WwBAjAqMCgGCCsGAQUFBwIBFhxodHRwczovL3d3dy5kaWdpY2VydC5jb20vQ1BT
MAgGBmeBDAECATANBgkqhkiG9w0BAQsFAAOCAQEAoBs1eCLKakLtVRPFRjBIJ9LJ
L0s8ZWum8U8/1TMVkQMBn+CPb5xnCD0GSA6L/V0ZFrMNqBirrr5B241OesECvxIi
98bZ90h9+q/X5eMyOD35f8YTaEMpdnQCnawIwiHx06/0BfiTj+b/XQih+mqt3ZXe
xNCJqKexdiB2IWGSKcgahPacWkk/BAQFisKIFYEqHzV974S3FAz/8LIfD58xnsEN
GfzyIDkH3JrwYZ8caPTf6ZX9M1GrISN8HnWTtdNCH2xEajRa/h9ZBXjUyFKQrGk2
n2hcLrfZSbynEC/pSw/ET7H5nWwckjmAJ1l9fcnbqkU/pf6uMQmnfl0JQjJNSg==
-----END CERTIFICATE-----
            ";

            let cursor = io::Cursor::new(cert);
            let mut buf = BufReader::new(cursor);
            let certs = rustls_pemfile::certs(&mut buf);
            Ok(certs.into_iter().collect::<Result<Vec<_>, _>>()?)
        }
    }

    fn load_keys(path: &Option<String>) -> io::Result<PrivateKeyDer<'static>> {
        let mut keys = if let Some(path) = path {
            let file = File::open(&path)?;
            let mut reader = BufReader::new(file);
            rustls_pemfile::rsa_private_keys(&mut reader).collect::<Result<Vec<_>, _>>()?
        } else {
            let key = br"-----BEGIN RSA PRIVATE KEY-----
MIIEpQIBAAKCAQEAw7gdhMEwp5al49V4b3DkwPWUa/Aiaxo5dk8+JWETaIfU8L9w
6BiKZJWRo8MxESfiZug+knP+tllkSTDSze3u7ZugY9wuhzIDkmsSHP9IKvgJaGEr
Gfbo2NNrOqMYfuuRMizAsIeG3RHxCpVbifgiSD2j3G22u5xgXx+4m9awKdYv8UJP
Aa+hJrbndvFcInaOl2FIFJL4HBh90kegHWPf4tGJPz8cUDQ9KDZ7eSOdhG2JJofy
Qmm7mXgxkloF5iqnv8HMmVfsPuGWB0Sd8RcEcUbwl92m+uCsKXMzVMOPRWO30pxD
Zh76fjNonz+gKQlqdeVlJ8rx8svbQH6maG4NhwIDAQABAoIBAAx+3UJECU9cfWPh
6v9bxacSJsiLZiR7Yfl35ApZO8wmhpsGRfbzgNEc1tW4mRHff4NoHCftMvEWckzV
SqguIt0rB0meMvRGZlb7R1vwP9Mfzzie94mqq2wTIhtLcr1rscidIIJEwitWwWfB
ExKTHoaJNO8rrAknWeRzhIL36SW1M793EmsbVatRD+GhogcKcZBevQBcddnaZjSw
1WAjS9LcbbQ1tn8y61dYz6L/2MhvDoDOz5oeVB9SC2RPs7omYvQPUig1tYbFxuDu
ruhaVE3BdAKGayJX6Z5lnEA6N66R1JodVBYTESWlTyEenidGg02HsD000eKcdtr7
0M2BAzECgYEA/bpXxHxMIJN+gL6uerEOeNSXYndJy/Y1GHVvdZP2YpL1eS1EK7RD
UNITpG9J++ulCuUb0jiGgJO0SdvKtZbmEYzysj6rUfbxDUBVSazkzVJUXJXiGB7m
0suGjb3xoCH9BzYofPnKfykmlUHVEVdU7eMOu/IjhoCpiRelKAMmnrECgYEAxXjK
bVigxTaTNyDANFCid7S37VuwlEXFC6mwGPAxyZPB4fXZ8Hti/H4uZ5MYSSjzTgVh
8ZWqHJXFC18KvOiLezhUS5rKwvqi/FlnPL/fk/s4qt8Cv1yExxW243SCLWpNIq4Y
/NIhExA2MF4S+mKtoRuAZZagH4WcE8Qplc8lrbcCgYEA+d406M7vuXUHM4qVEUak
VeImY1XOWwpQJ5Ie/c+E6HaJP5iQdenEESeRKHJgjbL2idAuocwAyUasWcAV1NaS
I96Gc3q8BLAHm2ErnK6jdIALjFIeolpsPlMoYxYXifdu01dGcC0eejPwRzTZu4Yh
oVPmArjmu2KhktyyTMEtm0ECgYEAwSj4iaFKEd7ifehRWlsNsR5bU5h+z2q35kKj
+KDrcoxP+KGt/2gSWX1sEvB1rwqZhFYLim6lqbRuvELJlCO8XFmrSxEtCTB1wXYK
YAgnwO7abXobi+gKEVuSPEe5FoeG0EeQNa2toKIY/5Ll6Xog8Rifrb96/ZqKI2Oc
cefgqV0CgYEAy6E0iLoE/59Us8YlE5h+MhX1ktUSFVndkE0vHeLQxOCOmeHWUDhg
cR+nZ6DRmzKISbcN9/m8I7xNWwU2cglrYa4NCHguQSrTefhRoZAfl8BEOW1rJVGC
9D/+Pt3qW+t6dfM5doK7Eb9+nyGiNw/G0/VywTlxf48YKntBbPi8Uww=
-----END RSA PRIVATE KEY-----
            ";
            let cursor = io::Cursor::new(key);
            let mut buf = BufReader::new(cursor);
            rustls_pemfile::rsa_private_keys(&mut buf).collect::<Result<Vec<_>, _>>()?
        };

        match keys.len() {
            0 => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("No RSA private key found"),
            )),
            1 => Ok(PrivateKeyDer::from(keys.remove(0))),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("More than one RSA private key found"),
            )),
        }
    }

    /// 获取服务端https的证书信息
    pub fn get_map_tls_accept(&self) -> ProxyResult<TlsAcceptor> {
        // if !self.tc {
        //     return Err(ProxyError::ProtNoSupport);
        // }
        let certs = Self::load_certs(&self.map_cert)?;
        let key = Self::load_keys(&self.map_key)?;

        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

        // config.alpn_protocols.push("http/1.1".as_bytes().to_vec());
        // config.alpn_protocols.push("h2".as_bytes().to_vec());

        let acceptor = TlsAcceptor::from(Arc::new(config));
        Ok(acceptor)
    }

    /// 获取服务端https的证书信息
    pub fn get_tls_accept(&self) -> ProxyResult<TlsAcceptor> {
        if !self.tc {
            return Err(ProxyError::ProtNoSupport);
        }
        let certs = Self::load_certs(&self.cert)?;
        let key = Self::load_keys(&self.key)?;

        let config = rustls::ServerConfig::builder();
        // 开始双向认证，需要客户端提供证书信息
        let config = if self.two_way_tls {
            let mut client_auth_roots = rustls::RootCertStore::empty();
            for root in certs.clone().into_iter() {
                client_auth_roots.add(root).unwrap();
            }
            let client_auth =
                rustls::server::WebPkiClientVerifier::builder(client_auth_roots.into())
                    .build()
                    .map_err(|_| ProxyError::Extension("add cert error"))?;

            // let client_auth = rustls::server::AllowAnyAuthenticatedClient::new(client_auth_roots);

            config
                .with_client_cert_verifier(client_auth)
                .with_single_cert(certs, key)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?
        } else {
            config
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?
        };

        let acceptor = TlsAcceptor::from(Arc::new(config));
        Ok(acceptor)
    }

    /// 获取客户端https的Config配置
    pub fn get_tls_request(&self) -> ProxyResult<Arc<rustls::ClientConfig>> {
        if !self.ts {
            return Err(ProxyError::ProtNoSupport);
        }
        let certs = Self::load_certs(&self.cert)?;
        let mut root_cert_store = rustls::RootCertStore::empty();
        // 信任通用的签名商
        root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        for cert in certs.clone().into_iter() {
            let _ = root_cert_store.add(cert);
        }
        let config = rustls::ClientConfig::builder().with_root_certificates(root_cert_store);

        if self.two_way_tls {
            let key = Self::load_keys(&self.key)?;
            Ok(Arc::new(config.with_client_auth_cert(certs, key).map_err(
                |err| io::Error::new(io::ErrorKind::InvalidInput, err),
            )?))
        } else {
            Ok(Arc::new(config.with_no_client_auth()))
        }
    }

    // pub fn is_client(&self) -> bool {
    //     self.mode.eq_ignore_ascii_case("client")
    // }

    // pub fn is_server(&self) -> bool {
    //     self.mode.eq_ignore_ascii_case("server")
    // }

    
    pub async fn try_connect_center_client(
        &self,
    ) -> ProxyResult<(
        Option<Arc<ClientConfig>>,
        Option<CenterClient>,
    )> {
        let client = self.get_tls_request().ok();
        let mut center_client = None;
        if self.bind.is_some() {
            if let Some(server) = self.server.clone() {
                let mut center = CenterClient::new(
                    self.clone(),
                    server,
                    client.clone(),
                    self.domain.clone(),
                    self.mappings.clone(),
                );
                match center.connect().await {
                    Ok(true) => (),
                    Ok(false) => {
                        log::error!("未能正确连上服务端:{:?}", self.server.clone().unwrap());
                        process::exit(1);
                    }
                    Err(err) => {
                        log::error!(
                            "未能正确连上服务端:{:?}, 发生错误:{:?}",
                            self.server.clone().unwrap(),
                            err
                        );
                        process::exit(1);
                    }
                }
                let _ = center.serve().await;
                center_client = Some(center);
            }
        }
        Ok((
            client,
            center_client,
        ))
    }

    pub async fn bind(
        &self,
    ) -> ProxyResult<(
        Option<TlsAcceptor>,
        Option<Arc<ClientConfig>>,
        Option<TcpListener>,
        Option<TcpListener>,
        Option<CenterClient>,
    )> {
        let proxy_accept = self.get_tls_accept().ok();
        let client = self.get_tls_request().ok();
        let mut center_client = None;
        if self.bind.is_some() {
            if let Some(server) = self.server.clone() {
                let mut center = CenterClient::new(
                    self.clone(),
                    server,
                    client.clone(),
                    self.domain.clone(),
                    self.mappings.clone(),
                );
                match center.connect().await {
                    Ok(true) => (),
                    Ok(false) => {
                        log::error!("未能正确连上服务端:{:?}", self.server.clone().unwrap());
                        process::exit(1);
                    }
                    Err(err) => {
                        log::error!(
                            "未能正确连上服务端:{:?}, 发生错误:{:?}",
                            self.server.clone().unwrap(),
                            err
                        );
                        process::exit(1);
                    }
                }
                let _ = center.serve().await;
                center_client = Some(center);
            }
        }
        let client_listener = if let Some(bind) = self.bind {
            log::info!("绑定代理：{:?}，提供代理功能。", bind.0);
            Some(Helper::bind(bind.0).await?)
        } else {
            None
        };
        let center_listener = if let Some(center) = self.center_addr {
            log::info!("绑定代理：{:?}，提供中心代理功能。", center.0);
            Some(Helper::bind(center.0).await?)
        } else {
            None
        };
        Ok((
            proxy_accept,
            client,
            client_listener,
            center_listener,
            center_client,
        ))
    }

    pub async fn bind_map(
        &self,
    ) -> ProxyResult<(
        Option<TcpListener>,
        Option<TcpListener>,
        Option<TcpListener>,
        Option<TcpListener>,
        Option<TlsAcceptor>,
    )> {
        let mut http_listener = None;
        let mut https_listener = None;
        let mut tcp_listener = None;
        let mut proxy_listener = None;
        let mut map_accept = None;
        if let Some(ls) = &self.map_http_bind {
            log::info!("内网穿透，http绑定：{:?}，提供http内网功能。", ls);
            http_listener = Some(Helper::bind(ls).await?);
        };
        if let Some(ls) = &self.map_https_bind {
            log::info!("内网穿透，https绑定：{:?}，提供https内网功能。", ls);
            https_listener = Some(Helper::bind(ls).await?);
        };

        if https_listener.is_some() {
            let accept = self.get_map_tls_accept().ok();
            if accept.is_none() {
                let _ = https_listener.take();
            }
            map_accept = accept;
        };

        if let Some(ls) = &self.map_tcp_bind {
            log::info!("内网穿透，tcp绑定：{:?}，提供tcp内网功能。", ls);
            tcp_listener = Some(Helper::bind(ls).await?);
        };

        if let Some(ls) = &self.map_proxy_bind {
            log::info!("内网穿透，tcp绑定：{:?}，提供tcp内网功能。", ls);
            proxy_listener = Some(Helper::bind(ls).await?);
        };

        Ok((
            http_listener,
            https_listener,
            tcp_listener,
            proxy_listener,
            map_accept,
        ))
    }
}

impl ConfigOption {
    pub fn new_by_proxy(proxy: ProxyConfig) -> Self {
        let mut config = ConfigOption::default();
        config.proxy = Some(proxy);
        config.disable_control = true;
        config
    }

    pub fn is_empty_listen(&self) -> bool {
        if self.http.is_some() || self.stream.is_some() || self.proxy.is_some() {
            return false;
        }
        true
    }

    pub fn after_load_option(&mut self) -> ProxyResult<()> {
        if let Some(http) = &mut self.http {
            http.after_load_option()?;
        }
        if let Some(stream) = &mut self.stream {
            stream.copy_to_child();
        }
        Ok(())
    }

    fn try_add_upstream(
        result: &mut Vec<OneHealth>,
        already: &mut HashSet<SocketAddr>,
        configs: &Vec<UpstreamConfig>,
    ) {
        for up in configs {
            if up.bind == "udp" {
                continue;
            }
            for s in &up.server {
                if already.contains(&s.addr) {
                    continue;
                }
                already.insert(s.addr);
                result.push(OneHealth::new(
                    s.addr,
                    "http".to_string(),
                    Duration::from_secs(1),
                ));
            }
        }
    }

    /// 获取所有待健康检查的列表
    pub fn get_health_check(&self) -> Vec<OneHealth> {
        let mut result = vec![];
        let mut already: HashSet<SocketAddr> = HashSet::new();
        // if let Some(proxy) = &self.proxy {
        //     if let Some(server) = proxy.server {
        //         result.push(OneHealth::new(
        //             server,
        //             String::new(),
        //             Duration::from_secs(5),
        //         ));
        //     }
        // }

        if let Some(http) = &self.http {
            Self::try_add_upstream(&mut result, &mut already, &http.upstream);
            for s in &http.server {
                Self::try_add_upstream(&mut result, &mut already, &s.upstream);
            }
        }

        if let Some(stream) = &self.stream {
            Self::try_add_upstream(&mut result, &mut already, &stream.upstream);
            for s in &stream.server {
                Self::try_add_upstream(&mut result, &mut already, &s.upstream);
            }
        }

        result
    }

    pub fn get_log_names(&self) -> HashMap<String, String> {
        let mut names = HashMap::new();
        if let Some(http) = &self.http {
            http.get_log_names(&mut names);
        }
        names
    }
}
