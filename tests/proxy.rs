// #![deny(warnings)]
#![deny(rust_2018_idioms)]

use std::convert::TryInto;
use std::future::Future;
use std::io::{self, Read, Write};
use std::net::TcpListener as StdTcpListener;
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::thread;
use std::time::Duration;


#[cfg(test)]
mod tests {
    macro_rules! create_proxy {
        () => {
            
        };
    }

    #[test]
    fn test_proxy() {
        
    }
}