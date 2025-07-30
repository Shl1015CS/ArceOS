//! UDP 回显示例

#![no_std]
#![no_main]

extern crate alloc;
use axnet::{UdpSocket, NetResult};
use core::net::{IpAddr, Ipv4Addr, SocketAddr};

#[no_mangle]
fn main() -> NetResult<()> {
    // 创建 UDP socket
    let socket = UdpSocket::new();
    
    // 绑定到本地地址
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8080);
    socket.bind(bind_addr)?;
    log::info!("UDP 服务器监听在 {}", bind_addr);
    
    let mut buffer = [0u8; 1024];
    
    loop {
        // 接收数据
        let (received, client_addr) = socket.recv_from(&mut buffer)?;
        log::info!("从 {} 接收了 {} 字节: {}", 
                 client_addr, received,
                 core::str::from_utf8(&buffer[..received]).unwrap_or("<invalid utf8>"));
        
        // 回显数据
        let sent = socket.send_to(&buffer[..received], client_addr)?;
        log::info!("向 {} 发送了 {} 字节", client_addr, sent);
    }
}