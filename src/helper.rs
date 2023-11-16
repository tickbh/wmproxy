use std::{io, net::ToSocketAddrs, collections::HashMap, cell::RefCell, sync::{Arc, Mutex}, str::FromStr};

use lazy_static::lazy_static;
use log::{LevelFilter, Record, Level, log_enabled};
use log4rs::{append::{console::ConsoleAppender, file::FileAppender}, encode::json::JsonEncoder, config::{Appender, Root, Logger}};
use socket2::{Socket, Domain, Type};
use tokio::{net::{TcpListener, UdpSocket}};
use webparse::{BinaryMut, Buf, http2::frame::read_u24, Request};
use wenmeng::RecvStream;
use crate::{ProxyResult, prot::{ProtFrame, ProtFrameHeader}, ConfigOption, ConfigLog, log::{PatternEncoder, writer::simple::SimpleWriter, Encode, ProxyRecord}};

thread_local! {
    static FORMAT_PATTERN_CACHE: RefCell<HashMap<&'static str, Arc<PatternEncoder>>> = RefCell::new(HashMap::new());
}

lazy_static! {
    static ref LOG4RS_HANDLE: Mutex<Option<log4rs::Handle>> = Mutex::new(None);
}


pub struct Helper;

impl Helper {
    pub fn decode_frame(read: &mut BinaryMut) -> ProxyResult<Option<ProtFrame>> {
        let data_len = read.remaining();
        if data_len < 8 {
            return Ok(None);
        }
        let mut copy = read.clone();
        let length = read_u24(&mut copy);
        let all_len = length as usize + ProtFrameHeader::FRAME_HEADER_BYTES;
        if all_len > data_len {
            return Ok(None);
        }
        read.advance(all_len);
        copy.mark_len(all_len - 3);
        let header = match ProtFrameHeader::parse_by_len(&mut copy, length) {
            Ok(v) => v,
            Err(err) => return Err(err),
        };

        match ProtFrame::parse(header, copy) {
            Ok(v) => return Ok(Some(v)),
            Err(err) => return Err(err),
        };
    }

    #[cfg(not(target_os = "windows"))]
    fn set_reuse_port(socket: &Socket, reuse: bool) -> io::Result<()> {
        socket.set_reuse_port(true)?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn set_reuse_port(_socket: &Socket, _sreuse: bool) -> io::Result<()> {
        Ok(())
    }
    
    /// 可端口复用的绑定方式，该端口可能被多个进程同时使用
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<TcpListener> {
        let addrs = addr.to_socket_addrs()?;
        let mut last_err = None;
        for addr in addrs {
            let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
            socket.set_nonblocking(true)?;
            let _ = socket.set_only_v6(false);
            socket.set_reuse_address(true)?;
            Self::set_reuse_port(&socket, true)?;
            socket.bind(&addr.into())?;
            match socket.listen(128) {
                Ok(_) => {
                    let listener: std::net::TcpListener = socket.into();
                    return TcpListener::from_std(listener);
                }
                Err(e) => {
                    log::info!("绑定端口地址失败，原因： {:?}", addr);
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any address",
            )
        }))
    }

    /// 可端口复用的绑定方式，该端口可能被多个进程同时使用
    pub async fn bind_upd<A: ToSocketAddrs>(addr: A) -> io::Result<UdpSocket> {
        let addrs = addr.to_socket_addrs()?;
        let last_err = None;
        for addr in addrs {
            let socket = Socket::new(Domain::IPV4, Type::DGRAM, None)?;
            socket.set_nonblocking(true)?;
            let _ = socket.set_only_v6(false);
            socket.set_reuse_address(true)?;
            Self::set_reuse_port(&socket, true)?;
            socket.bind(&addr.into())?;
            let listener: std::net::UdpSocket = socket.into();
            return UdpSocket::from_std(listener);
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any address",
            )
        }))
    }

    pub fn try_init_log(option: &ConfigOption) {
        let log_names = option.get_log_names();
        let mut log_config = log4rs::config::Config::builder();
        let mut root = Root::builder();
        for (name, path) in log_names {
            let (path, level) = {
                let vals: Vec<&str> = path.split(' ').collect();
                if vals.len() == 1 {
                    (path, Level::Info)
                } else {
                    (vals[0].to_string(), Level::from_str(vals[1]).ok().unwrap_or(Level::Info))
                }
            };
            let appender = FileAppender::builder().encoder(Box::new(log4rs::encode::pattern::PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} {m}{n}"))).build(path).unwrap();
            if name == "default" {
                root = root.appender(name.clone());
            }
            log_config = log_config.appender(Appender::builder().build(name.clone(), Box::new(appender)));
            log_config = log_config.logger(Logger::builder().appender(name.clone()).additive(false).build(name.clone(), level.to_level_filter()));
        }
        
        if !option.disable_stdout {
            let stdout: ConsoleAppender = ConsoleAppender::builder().build();
            log_config = log_config.appender(Appender::builder().build("stdout", Box::new(stdout)));
            root = root.appender("stdout");
        }

        let log_config = log_config.build(root.build(LevelFilter::Info)).unwrap();
        if LOG4RS_HANDLE.lock().unwrap().is_some() {
            LOG4RS_HANDLE.lock().unwrap().as_mut().unwrap().set_config(log_config);
        } else {
            let handle = log4rs::init_config(log_config).unwrap();
            *LOG4RS_HANDLE.lock().unwrap() = Some(handle);
        }
    }

    pub fn log_acess(log_formats: &HashMap<String, String>, access: &Option<ConfigLog>, req: &Request<RecvStream>) {
        if let Some(access) = access {
            if let Some(formats) = log_formats.get(&access.format) {
                log::error!(target: &access.name, "this is error");
                log::warn!(target: &access.name, "this is warn");
                log::info!(target: &access.name, "this is info");
                log::debug!(target: &access.name, "this is debug");
                log::trace!(target: &access.name, "this is trace");
                if log_enabled!(target: &access.name, access.level) {
                    let pw = FORMAT_PATTERN_CACHE.with(|m| {
                        if !m.borrow().contains_key(&**formats) {
                            let p = PatternEncoder::new(formats);
                            m.borrow_mut().insert(Box::leak(formats.clone().into_boxed_str()), Arc::new(p));
                        }
                        m.borrow()[&**formats].clone()
                    });
                    
                    let record = ProxyRecord::new_req(Record::builder().level(Level::Info).build(), req);
                    let mut buf = vec![];
                    pw.encode(
                        &mut SimpleWriter(&mut buf),
                        &record,
                    )
                    .unwrap();
                    match access.level {
                        Level::Error => log::error!(target: &access.name, "{}", String::from_utf8_lossy(&buf[..])),
                        Level::Warn => log::warn!(target: &access.name, "{}", String::from_utf8_lossy(&buf[..])),
                        Level::Info => log::info!(target: &access.name, "{}", String::from_utf8_lossy(&buf[..])),
                        Level::Debug => log::debug!(target: &access.name, "{}", String::from_utf8_lossy(&buf[..])),
                        Level::Trace => log::trace!(target: &access.name, "{}", String::from_utf8_lossy(&buf[..])),
                    };
                    
                }
            }
        }
    }

    // pub async fn udp_recv_from(socket: &UdpSocket, buf: &mut [u8]) -> io::Result<usize> {
    //     let (s, addr) = socket.recv_from(&mut buf).await?;
    //     unsafe {
    //         buf.advance_mut(size);
    //     }
    // }
}