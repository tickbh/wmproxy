# wmproxy
一个同时支持http/https/socks5的代理，通过简单配置即可获取加密通道的内网穿透功能，支持http/https/tcp内网穿透

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
配置文件如下:
```
# 连接服务端地址
server: 127.0.0.1:8091
# 连接服务端是否加密
ts: true

# 内网映射配置的数组
mappings:
  #将localhost的域名转发到本地的127.0.0.1:8080
  - name: web
    mode: http
    local_addr: 127.0.0.1:8080
    domain: localhost
    headers:
      - [proxy, +, x-forward-for, $client_ip]
      - [proxy, +, from, $url]
      - [+, key, value]
      - [-, etag]
      - [+, last-modified, aaaa]

  #将tcp的流量无条件转到127.0.0.1:8080
  - name: tcp
    mode: tcp
    local_addr: 127.0.0.1:8080
    domain: 
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
配置文件如下:
```
#绑定的ip地址
bind_addr: 127.0.0.1:8091

#代理支持的功能，1为http，2为https，4为socks5
flag: 7

#内网映射http绑定地址
map_http_bind: 127.0.0.1:8001
#内网映射tcp绑定地址
map_tcp_bind: 127.0.0.1:8002
#内网映射https绑定地址
map_https_bind: 127.0.0.1:8003
#内网映射的公钥证书，为空则是默认证书
map_cert: 
#内网映射的私钥证书，为空则是默认证书
map_key:

#接收客户端是为是加密客户端
tc: true
#当前服务模式，server为服务端，client为客户端
mode: server
```

```--tc```表示接收子级代理的时候需要用加密的方式链接，可以```--cert```指定证书的公钥，```--key```指定证书的私钥，```--domain```指定证书的域名，如果不指定，则默认用自带的证书参数
> 至此通过代理访问的，我们已经没有办法得到真正的请求地址，只能得到代理发起的请求

### 代理间的认证
> 目前提供了两种认证双式, ```two_way_tls```开启客户端的证书认证信息, 另一种服务端配置了```username```和```password```即需要客户端配置相同的用户密码才能启用认证信息, 两种方式可叠加使用或者单独使用。
> 配置添加如下:
```yaml
two_way_tls: true
username: wmproxy
password: wmproxy
```

# 🚥 Roadmap
### socks5

- [x] IPV6 Support
- [x] `SOCKS5` Authentication Methods
  - [x] `NOAUTH`
  - [x] `USERPASS`
- [x] `SOCKS5` Commands
  - [x] `CONNECT`
  - [x] `UDP ASSOCIATE`

### http/https

- [x] IPV6 Support

### 内网穿透

- [x] Http Support
- [x] Https Support
- [x] Tcp Support

#### 支持功能
- 修改 HTTP 请求 Header
- 支持 HTTP2 的内网穿透