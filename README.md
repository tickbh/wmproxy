# wmproxy
http/https/socks5 proxy by rust
一个同时支持http/https/socks5的代理


## 📦 Installation & 🏃 Usage

### Installation

```bash
cargo install wmproxy
```

OR

```bash
git clone https://github.com/tickbh/wmproxy
cd wmproxy
cargo install --path .
```

### Usage
默认端口为8090端口，默认监听地址为127.0.0.1
```bash
# 直接通用默认参数
wmproxy

# 设置账号密码
wmproxy -p 8090 -b 0.0.0.0 --user wmproxy --pass wmproxy

wmproxy --help
```



# 🚥 Roadmap

- [x] IPV6 Support
- [x] `SOCKS5` Authentication Methods
  - [x] `NOAUTH`
  - [x] `USERPASS`
- [ ] `SOCKS5` Commands
  - [x] `CONNECT`
  - [ ] `UDP ASSOCIATE`