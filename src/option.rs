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
    io::{self, BufReader, Read},
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    process,
    sync::Arc,
    time::Duration,
};

use commander::Commander;
use rustls::{Certificate, ClientConfig, PrivateKey};

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use tokio::net::TcpListener;
use tokio_rustls::{rustls, TlsAcceptor};

use crate::{
    reverse::{HttpConfig, StreamConfig, UpstreamConfig},
    CenterClient, Flag, Helper, MappingConfig, OneHealth, ProxyError, ProxyResult,
};

use bitflags::bitflags;

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
    pub struct Mode: u8 {
        /// 未知类型, 单进程模型
        const NONE = 0x0;
        /// 仅客户端类型
        const CLIENT = 0x1;
        /// 仅服务端类型
        const SERVER = 0x2;
        /// 中转客户端及服务端
        const ALL = 0x3;
    }
}

impl Mode {
    pub fn is_none(&self) -> bool {
        self.bits() == 0
    }

    pub fn is_client(&self) -> bool {
        self.contains(Self::CLIENT)
    }

    pub fn is_server(&self) -> bool {
        self.contains(Self::SERVER)
    }
}

impl Serialize for Mode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(self.bits())
    }
}

impl<'a> Deserialize<'a> for Mode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let v = u8::deserialize(deserializer)?;
        Ok(Mode::from_bits(v).unwrap_or(Mode::NONE))
    }
}

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

    pub fn mode(self, mode: String) -> Builder {
        self.and_then(|mut proxy| {
            proxy.mode = mode;
            Ok(proxy)
        })
    }

    pub fn add_flag(self, flag: Flag) -> Builder {
        self.and_then(|mut proxy| {
            proxy.flag.set(flag, true);
            Ok(proxy)
        })
    }

    pub fn bind_addr(self, addr: SocketAddr) -> Builder {
        self.and_then(|mut proxy| {
            proxy.bind_addr = addr;
            Ok(proxy)
        })
    }

    pub fn server(self, addr: Option<SocketAddr>) -> Builder {
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

    pub fn center(self, center: bool) -> Builder {
        self.and_then(|mut proxy| {
            proxy.center = center;
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

fn default_bool_true() -> bool {
    true
}

/// 代理类, 一个代理类启动一种类型的代理
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    #[serde(default = "default_bind_addr")]
    pub(crate) bind_addr: SocketAddr,
    #[serde_as(as = "DisplayFromStr")]
    #[serde(default)]
    pub(crate) flag: Flag,
    #[serde(default)]
    pub(crate) mode: String,
    pub(crate) server: Option<SocketAddr>,
    /// 用于socks验证及中心服务器验证
    pub(crate) username: Option<String>,
    /// 用于socks验证及中心服务器验证
    pub(crate) password: Option<String>,
    pub(crate) udp_bind: Option<IpAddr>,

    pub(crate) map_http_bind: Option<SocketAddr>,
    pub(crate) map_https_bind: Option<SocketAddr>,
    pub(crate) map_tcp_bind: Option<SocketAddr>,
    pub(crate) map_cert: Option<String>,
    pub(crate) map_key: Option<String>,

    //// 是否启用协议转发
    #[serde(default = "default_bool_true")]
    pub(crate) center: bool,
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

/// 代理类, 一个代理类启动一种类型的代理
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigOption {
    /// HTTP反向代理,静态文件服相关
    #[serde(default)]
    pub(crate) proxy: Option<ProxyConfig>,
    /// HTTP反向代理,静态文件服相关
    #[serde(default)]
    pub(crate) http: Option<HttpConfig>,
    #[serde(default)]
    pub(crate) stream: Option<StreamConfig>,
    #[serde(default = "default_control_port")]
    pub(crate) control: SocketAddr,
    #[serde(default)]
    pub(crate) disable_stdout: bool,
    #[serde(default)]
    pub(crate) disable_control: bool,
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
        }
    }
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            flag: Flag::HTTP | Flag::HTTPS | Flag::SOCKS5,
            mode: "client".to_string(),
            bind_addr: default_bind_addr(),
            server: None,
            username: None,
            password: None,
            udp_bind: None,
            map_http_bind: None,
            map_https_bind: None,
            map_tcp_bind: None,
            map_cert: None,
            map_key: None,

            center: false,
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

    fn load_certs(path: &Option<String>) -> io::Result<Vec<Certificate>> {
        if let Some(path) = path {
            let file = File::open(path)?;
            let mut reader = BufReader::new(file);
            let certs = rustls_pemfile::certs(&mut reader)?;
            Ok(certs.into_iter().map(Certificate).collect())
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
            let certs = rustls_pemfile::certs(&mut buf)?;
            Ok(certs.into_iter().map(Certificate).collect())
        }
    }

    fn load_keys(path: &Option<String>) -> io::Result<PrivateKey> {
        let mut keys = if let Some(path) = path {
            let file = File::open(&path)?;
            let mut reader = BufReader::new(file);
            rustls_pemfile::rsa_private_keys(&mut reader)?
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
            rustls_pemfile::rsa_private_keys(&mut buf)?
        };

        match keys.len() {
            0 => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("No RSA private key found"),
            )),
            1 => Ok(PrivateKey(keys.remove(0))),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("More than one RSA private key found"),
            )),
        }
    }

    /// 获取服务端https的证书信息
    pub async fn get_map_tls_accept(&self) -> ProxyResult<TlsAcceptor> {
        if !self.tc {
            return Err(ProxyError::ProtNoSupport);
        }
        let certs = Self::load_certs(&self.map_cert)?;
        let key = Self::load_keys(&self.map_key)?;

        let config = rustls::ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

        let acceptor = TlsAcceptor::from(Arc::new(config));
        Ok(acceptor)
    }

    /// 获取服务端https的证书信息
    pub async fn get_tls_accept(&self) -> ProxyResult<TlsAcceptor> {
        if !self.tc {
            return Err(ProxyError::ProtNoSupport);
        }
        let certs = Self::load_certs(&self.cert)?;
        let key = Self::load_keys(&self.key)?;

        let config = rustls::ServerConfig::builder().with_safe_defaults();
        // 开始双向认证，需要客户端提供证书信息
        let config = if self.two_way_tls {
            let mut client_auth_roots = rustls::RootCertStore::empty();
            for root in &certs {
                client_auth_roots.add(&root).unwrap();
            }

            let client_auth = rustls::server::AllowAnyAuthenticatedClient::new(client_auth_roots);

            config
                .with_client_cert_verifier(client_auth.boxed())
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
    pub async fn get_tls_request(&self) -> ProxyResult<Arc<rustls::ClientConfig>> {
        if !self.ts {
            return Err(ProxyError::ProtNoSupport);
        }
        let certs = Self::load_certs(&self.cert)?;
        let mut root_cert_store = rustls::RootCertStore::empty();
        // 信任通用的签名商
        root_cert_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
            rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                ta.subject,
                ta.spki,
                ta.name_constraints,
            )
        }));
        for cert in &certs {
            let _ = root_cert_store.add(cert);
        }
        let config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_cert_store);

        if self.two_way_tls {
            let key = Self::load_keys(&self.key)?;
            Ok(Arc::new(config.with_client_auth_cert(certs, key).map_err(
                |err| io::Error::new(io::ErrorKind::InvalidInput, err),
            )?))
        } else {
            Ok(Arc::new(config.with_no_client_auth()))
        }
    }

    pub fn is_client(&self) -> bool {
        self.mode.eq_ignore_ascii_case("client")
    }

    pub fn is_server(&self) -> bool {
        self.mode.eq_ignore_ascii_case("server")
    }

    pub async fn bind(
        &self,
    ) -> ProxyResult<(
        Option<TlsAcceptor>,
        Option<Arc<ClientConfig>>,
        Option<TcpListener>,
        Option<CenterClient>,
    )> {
        let addr = self.bind_addr.clone();
        let proxy_accept = self.get_tls_accept().await.ok();
        let client = self.get_tls_request().await.ok();
        let mut center_client = None;
        if self.center {
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
                        log::error!("未能正确连上服务端:{:?}", self.server.unwrap());
                        process::exit(1);
                    }
                    Err(err) => {
                        log::error!(
                            "未能正确连上服务端:{:?}, 发生错误:{:?}",
                            self.server.unwrap(),
                            err
                        );
                        process::exit(1);
                    }
                }
                let _ = center.serve().await;
                center_client = Some(center);
            }
        }
        let center_listener = Some(Helper::bind(addr).await?);
        Ok((proxy_accept, client, center_listener, center_client))
    }

    pub async fn bind_map(
        &self,
    ) -> ProxyResult<(
        Option<TcpListener>,
        Option<TcpListener>,
        Option<TcpListener>,
        Option<TlsAcceptor>,
    )> {
        let mut http_listener = None;
        let mut https_listener = None;
        let mut tcp_listener = None;
        let mut map_accept = None;
        if let Some(ls) = &self.map_http_bind {
            http_listener = Some(Helper::bind(ls).await?);
        };
        if let Some(ls) = &self.map_https_bind {
            https_listener = Some(Helper::bind(ls).await?);
        };

        if https_listener.is_some() {
            let accept = self.get_map_tls_accept().await.ok();
            if accept.is_none() {
                let _ = https_listener.take();
            }
            map_accept = accept;
        };

        if let Some(ls) = &self.map_tcp_bind {
            tcp_listener = Some(Helper::bind(ls).await?);
        };
        Ok((http_listener, https_listener, tcp_listener, map_accept))
    }
}

impl ConfigOption {
    pub fn new_by_proxy(proxy: ProxyConfig) -> Self {
        let mut config = ConfigOption::default();
        config.proxy = Some(proxy);
        config.disable_control = true;
        config
    }
    pub fn parse_env() -> ProxyResult<ConfigOption> {
        let command = Commander::new()
            .version(&env!("CARGO_PKG_VERSION").to_string())
            .usage("-b 127.0.0.1:8090")
            .usage_desc("wmproxy -b 127.0.0.1:8090")
            .option_list(
                "-f, --flag [value]",
                "可兼容的方法, 如http https socks5",
                None,
            )
            .option_str(
                "-m, --mode value",
                "client.表示客户端,server 表示服务端,all表示服务端及客户端",
                None,
            )
            .option_str("-c, --config", "配置文件", None)
            .option_str("--http value", "内网穿透的http代理监听地址", None)
            .option_str("--https value", "内网穿透的https代理监听地址", None)
            .option_str("--tcp value", "内网穿透的tcp代理监听地址", None)
            // .option("--proxy value", "是否只接收来自代理的连接", Some(false))
            .option("--center value", "是否启用协议转发", Some(false))
            .option("--tc value", "接收客户端是否加密", Some(false))
            .option("--ts value", "连接服务端是否加密", Some(false))
            .option_str("--cert value", "证书的公钥", None)
            .option_str("--key value", "证书的私钥", None)
            .option_str("--domain value", "证书的域名", None)
            .option_str(
                "-b, --bind value",
                "监听地址及端口",
                Some("127.0.0.1:8090".to_string()),
            )
            .option_str("--user value", "auth的用户名", None)
            .option_str("-S value", "父级的监听端口地址,如127.0.0.1:8091", None)
            .option_str("--pass value", "auth的密码", None)
            .option_str(
                "--udp value",
                "udp的监听地址,如127.0.0.1,socks5的udp协议用",
                None,
            )
            .parse_env_or_exit();

        if let Some(config) = command.get_str("c") {
            let path = PathBuf::from(&config);
            let mut file = File::open(config)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            let extension = path.extension().unwrap().to_string_lossy().to_string();
            let mut option = match &*extension {
                "yaml" => serde_yaml::from_str::<ConfigOption>(&contents).map_err(|e| {
                    println!("parse error msg = {:?}", e);
                    io::Error::new(io::ErrorKind::Other, "parse yaml error")
                })?,
                "toml" => toml::from_str::<ConfigOption>(&contents).map_err(|e| {
                    println!("parse error msg = {:?}", e);
                    io::Error::new(io::ErrorKind::Other, "parse toml error")
                })?,
                _ => {
                    let e = io::Error::new(io::ErrorKind::Other, "unknow format error");
                    return Err(e.into());
                }
            };

            option.after_load_option()?;
            return Ok(option);
        }

        let listen_host = command.get_str("b").unwrap();
        let addr = listen_host.parse().unwrap();
        let mut builder = ProxyConfig::builder().bind_addr(addr);

        builder = builder.flag(Flag::HTTP | Flag::HTTPS | Flag::SOCKS5);
        builder = builder.mode(command.get_str("m").unwrap_or(String::new()));
        builder = builder.username(command.get_str("user"));
        builder = builder.password(command.get_str("pass"));
        builder = builder.tc(command.get("tc").unwrap_or(false));
        builder = builder.ts(command.get("ts").unwrap_or(false));
        builder = builder.center(command.get("center").unwrap_or(false));
        builder = builder.domain(command.get_str("domain"));
        builder = builder.cert(command.get_str("cert"));
        builder = builder.key(command.get_str("key"));
        if let Some(udp) = command.get_str("udp") {
            builder = builder.udp_bind(udp.parse::<IpAddr>().ok());
        };
        if let Some(http) = command.get_str("http") {
            builder = builder.map_http_bind(http.parse::<SocketAddr>().ok());
        };
        if let Some(https) = command.get_str("https") {
            builder = builder.map_https_bind(https.parse::<SocketAddr>().ok());
        };
        if let Some(tcp) = command.get_str("tcp") {
            builder = builder.map_tcp_bind(tcp.parse::<SocketAddr>().ok());
        };
        if let Some(s) = command.get_str("S") {
            builder = builder.server(s.parse::<SocketAddr>().ok());
        };

        log::debug!("启动默认信息，只开启代理信息");
        Ok(ConfigOption {
            proxy: Some(builder.inner?),
            http: None,
            stream: None,
            control: default_control_port(),
            disable_stdout: false,
            disable_control: false,
        })
    }

    pub fn is_empty_listen(&self) -> bool {
        if self.http.is_some() || self.stream.is_some() || self.proxy.is_some()  {
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
        println!("options = {:?}", self);
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

    pub fn get_health_check(&self) -> Vec<OneHealth> {
        let mut result = vec![];
        let mut already: HashSet<SocketAddr> = HashSet::new();
        if let Some(proxy) = &self.proxy {
            if let Some(server) = proxy.server {
                result.push(OneHealth::new(
                    server,
                    String::new(),
                    Duration::from_secs(5),
                ));
            }
        }

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
