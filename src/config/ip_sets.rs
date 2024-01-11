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
// Created Date: 2023/12/22 11:34:48

use std::{net::IpAddr, str::FromStr, io, fmt::Display};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpGate {
    pub ip: IpAddr,
    pub gate: u8,
}

impl IpGate {
    pub fn contains(&self, ip: &IpAddr) -> bool {
        if self.gate == 0 {
            ip == &self.ip
        } else {
            match (&ip, &self.ip) {
                (IpAddr::V4(other), IpAddr::V4(my)) => {
                    let other = u32::from_be_bytes(other.octets()) >> (32u8 - self.gate);
                    let my = u32::from_be_bytes(my.octets()) >> (32u8 - self.gate);
                    other == my
                }
                _ => {
                    ip == &self.ip
                }
            }
        }
    }
}

impl FromStr for IpGate {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let vals = s.split("/").collect::<Vec<&str>>();
        let ip = vals[0].parse::<IpAddr>().map_err(|_| io::Error::new(io::ErrorKind::Other, "parse ip error"))?;
        let mut gate = 0;
        if vals.len() > 1 {
            gate = vals[1].parse::<u8>().map_err(|_| io::Error::new(io::ErrorKind::Other, "parse ip error"))?;
            if ip.is_ipv4() && gate > 32 {
                return Err(io::Error::new(io::ErrorKind::Other, "too big gate"));
            } else if ip.is_ipv6() && gate > 128 {
                return Err(io::Error::new(io::ErrorKind::Other, "too big gate"));
            }
        }
        Ok(IpGate {
            ip, gate
        })
    }
}

impl Display for IpGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.gate > 0 {
            f.write_fmt(format_args!("{}/{}", self.ip, self.gate))
        } else {
            f.write_fmt(format_args!("{}", self.ip))
        }
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpSets {
    pub ips: Vec<IpGate>,
}

impl IpSets {

    pub fn contains(&self, ip: &IpAddr) -> bool {
        for v in &self.ips {
            if v.contains(ip) {
                return true;
            }
        }
        false
    }
}


impl FromStr for IpSets {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let vals = s.split_whitespace().collect::<Vec<&str>>();
        let mut ips = vec![];
        for v in vals {
            ips.push(v.parse::<IpGate>()?);
        }
        Ok(IpSets { ips })
    }
}

impl Display for IpSets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for ip in &self.ips {
            ip.fmt(f)?;
            f.write_str(" ")?;
        }
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, IpAddr};
    use crate::IpSets;
    
    #[test]
    fn do_test() {
        let ips = "127.0.0.1 255.255.255.0/24".parse::<IpSets>().unwrap();
        let ip_local = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        assert_eq!(ips.ips[0].ip, ip_local);
        assert_eq!(ips.ips[1].ip, IpAddr::V4(Ipv4Addr::new(255, 255, 255, 0)));
        assert_eq!(ips.ips[1].gate, 24);
        assert!(ips.contains(&ip_local));
        assert!(ips.contains(&IpAddr::V4(Ipv4Addr::new(255, 255, 255, 128))));
    }
}


