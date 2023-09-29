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

# 其它指令
wmproxy --help

#配置文件版启动
wmproxy -c config/client.yaml
```

##### 启动二级代理
1. 在本地启动代理
```bash
wmproxy -b 127.0.0.1 -p 8090 -S 127.0.0.1:8091 --ts
```
或者
```bash
wmproxy -c config/client.yaml
```
因为纯转发，所以在当前节点设置账号密码没有意义`-S`表示连接到的二级代理地址，**有该参数则表示是中转代理，否则是末端代理。**```--ts```表示连接父级代理的时候需要用加密的方式链接

2. 在远程启动代理
```bash
wmproxy --user proxy --pass proxy -b 0.0.0.0 -p 8091 --tc
```
或者
```bash
wmproxy -c config/server.yaml
```
```--tc```表示接收子级代理的时候需要用加密的方式链接，可以```--cert```指定证书的公钥，```--key```指定证书的私钥，```--domain```指定证书的域名，如果不指定，则默认用自带的证书参数
> 至此通过代理访问的，我们已经没有办法得到真正的请求地址，只能得到代理发起的请求

# 🚥 Roadmap
socks5

- [x] IPV6 Support
- [x] `SOCKS5` Authentication Methods
  - [x] `NOAUTH`
  - [x] `USERPASS`
- [x] `SOCKS5` Commands
  - [x] `CONNECT`
  - [x] `UDP ASSOCIATE`

http/https

- [x] IPV6 Support

内网穿透

- [x] Http Support
- [x] Https Support
- [x] Tcp Support
