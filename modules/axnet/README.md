# ArceOS 网络模块 - 完整TCP/UDP实现

基于smoltcp的完整网络栈实现，为操作系统提供完整的网络访问能力。

## 特性

### 核心功能
- ✅ 完整的 TCP 支持（客户端和服务端）
- ✅ 完整的 UDP 支持（数据报和连接模式）
- ✅ DNS 查询功能
- ✅ IPv4 和 IPv6 支持
- ✅ 非阻塞 I/O 模式
- ✅ 地址重用 (SO_REUSEADDR)
- ✅ 回环接口支持

### 架构改进
- 🔧 清晰的模块化设计
- 🔧 改进的错误处理
- 🔧 更好的状态管理
- 🔧 优化的性能
- 🔧 增强的可维护性

## 模块结构

```
src/
├── lib.rs              # 模块入口和公共 API
├── error.rs            # 错误类型定义
├── stack.rs            # 网络栈通用定义
└── smoltcp_impl/       # smoltcp 实现
    ├── mod.rs          # 网络栈管理器
    ├── device.rs       # 设备适配器
    ├── interface.rs    # 网络接口管理
    ├── socket_set.rs   # Socket 集合管理
    ├── listen_table.rs # TCP 监听表
    ├── tcp.rs          # TCP Socket 实现
    ├── udp.rs          # UDP Socket 实现
    └── dns.rs          # DNS 查询实现
```

## API 使用示例

### TCP 客户端

```rust
use axnet::{TcpSocket, SocketAddr};

let socket = TcpSocket::new();
let addr = "127.0.0.1:8080".parse::<SocketAddr>()?;

// 连接到服务器
socket.connect(addr)?;

// 发送数据
socket.send(b"Hello, Server!")?;

// 接收数据
let mut buffer = [0u8; 1024];
let len = socket.recv(&mut buffer)?;

// 关闭连接
socket.shutdown()?;
```

### TCP 服务器

```rust
use axnet::{TcpSocket, SocketAddr};

let listener = TcpSocket::new();
let addr = "0.0.0.0:8080".parse::<SocketAddr>()?;

// 绑定和监听
listener.bind(addr)?;
listener.listen()?;

// 接受连接
let client = listener.accept()?;

// 处理客户端请求
let mut buffer = [0u8; 1024];
let len = client.recv(&mut buffer)?;
client.send(b"Hello, Client!")?;

client.shutdown()?;
```

### UDP Socket

```rust
use axnet::{UdpSocket, SocketAddr};

let socket = UdpSocket::new();
let addr = "0.0.0.0:8080".parse::<SocketAddr>()?;

// 绑定到本地地址
socket.bind(addr)?;

// 接收数据报
let mut buffer = [0u8; 1024];
let (len, peer_addr) = socket.recv_from(&mut buffer)?;

// 发送数据报
socket.send_to(b"Hello, UDP!", peer_addr)?;
```

### DNS 查询

```rust
use axnet::dns_query;

// 查询域名
let addresses = dns_query("example.com")?;
for addr in addresses {
    println!("解析到地址: {}", addr);
}
```

## 配置选项

### 环境变量
- `AX_IP`: 设置默认 IP 地址 (默认: 10.0.2.15)
- `AX_GW`: 设置默认网关 (默认: 10.0.2.2)

### Cargo Features
- `smoltcp`: 启用 smoltcp 网络栈 (默认启用)
- `ip`: 启用 IP 协议支持

## 性能优化

### 缓冲区大小
- TCP 接收缓冲区: 64KB
- TCP 发送缓冲区: 64KB  
- UDP 接收缓冲区: 64KB
- UDP 发送缓冲区: 64KB

### 连接管理
- TCP 监听队列大小: 512
- 临时端口范围: 49152-65535
- 支持端口重用

## 错误处理

模块提供了完整的错误类型定义：

```rust
pub enum NetError {
    ConnectionRefused,
    ConnectionReset,
    Timeout,
    AddrInUse,
    AddrNotAvailable,
    NetworkUnreachable,
    HostUnreachable,
    WouldBlock,
    InvalidInput,
    NotConnected,
    AlreadyConnected,
    BufferFull,
    BufferEmpty,
    Unsupported,
    Internal,
}
```

## 兼容性

重构后的模块保持了与原有 API 的兼容性，同时提供了更好的功能和性能。

## 测试

运行示例程序：

```bash
# TCP 服务器
cargo run --example tcp_server

# TCP 客户端  
cargo run --example tcp_client

# UDP 回显服务器
cargo run --example udp_echo
```

