//! TCP 服务器示例

#![no_std]
#![no_main]

extern crate alloc;
use axnet::{TcpSocket, NetResult};
use core::net::{IpAddr, Ipv4Addr, SocketAddr};

#[no_mangle]
fn main() -> NetResult<()> {
    // 创建 TCP socket
    let listener = TcpSocket::new();
    
    // 绑定到本地地址
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8080);
    listener.bind(bind_addr)?;
    
    // 开始监听
    listener.listen()?;
    log::info!("服务器监听在 {}", bind_addr);
    
    loop {
        // 接受新连接
        let client_socket = listener.accept()?;
        let client_addr = client_socket.peer_addr()?;
        log::info!("接受来自 {} 的连接", client_addr);
        
        // 处理客户端请求
        let mut buffer = [0u8; 1024];
        let received = client_socket.recv(&mut buffer)?;
        log::info!("接收了 {} 字节: {}", received,
                 core::str::from_utf8(&buffer[..received]).unwrap_or("<invalid utf8>"));
        
        // 发送响应
        let response = b"Hello, Client!";
        let sent = client_socket.send(response)?;
        log::info!("发送了 {} 字节", sent);
        
        // 关闭客户端连接
        client_socket.shutdown()?;
    }
}