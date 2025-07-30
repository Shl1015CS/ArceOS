//! TCP 客户端示例

#![no_std]
#![no_main]

extern crate alloc;
use axnet::{TcpSocket, NetResult};
use core::net::{IpAddr, Ipv4Addr, SocketAddr};

#[no_mangle]
fn main() -> NetResult<()> {
    // 创建 TCP socket
    let socket = TcpSocket::new();
    
    // 连接到服务器
    let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    socket.connect(server_addr)?;
    
    // 发送数据
    let message = b"Hello, Server!";
    let sent = socket.send(message)?;
    log::info!("发送了 {} 字节", sent);
    
    // 接收响应
    let mut buffer = [0u8; 1024];
    let received = socket.recv(&mut buffer)?;
    log::info!("接收了 {} 字节: {}", received, 
             core::str::from_utf8(&buffer[..received]).unwrap_or("<invalid utf8>"));
    
    // 关闭连接
    socket.shutdown()?;
    
    Ok(())
}