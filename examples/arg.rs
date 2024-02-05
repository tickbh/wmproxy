use async_std::net;
use bpaf::{short, Bpaf, Parser};
use std::{fmt::Debug, path::PathBuf};
use console::{style, Style};
use local_ip_address::{list_afinet_netifas, local_ip};

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
#[allow(dead_code)]
struct Opts {
    /// 是否开始调试模式
    #[bpaf(short, long)]
    debug: bool,
    /// 这是一个注释,将被忽略
    #[bpaf(external(verbose))]
    verbose: usize,
    /// 设置速度, 拥有默认速度
    #[bpaf(argument("SPEED"), fallback(42.0), display_fallback)]
    speed: f64,
    /// 输出目录
    output: PathBuf,

    /// 将检测必须为正数
    #[bpaf(guard(positive, "must be positive"), fallback(1))]
    nb_cars: u32,
    files_to_process: Vec<PathBuf>,
}

fn verbose() -> impl Parser<usize> {
    // number of occurrences of the v/verbose flag capped at 3
    short('v')
        .long("verbose")
        .help("启动verbose模式\n根据输入的v的个数来判定等级\n可以 -v -v -v 或者 -vvv")
        .req_flag(())
        .many()
        .map(|xs| xs.len())
        .guard(|&x| x <= 3, "最多仅能输入三个v")
}

fn positive(input: &u32) -> bool {
    *input > 1
}

fn main() {
    println!("This is {} neat", style("quite").cyan());
    // Style::new().blink().fmt("https://www.baidu.com")

    println!("This is {} neat", Style::new().green().blink().apply_to("https://www.baidu.com"));

    let network_interfaces = list_afinet_netifas().unwrap();
    for net in network_interfaces {
        println!("This is my local IP address: {:?} {:?}", net.0, net.1);
    }
    let my_local_ip = local_ip().unwrap();
    println!("This is my local IP address: {:?}", my_local_ip);
    // println!("{:#?}", opts().run());
    
}