//! 网络功能测试示例
//!
//! 测试TCP和UDP的基本功能

#![no_std]
#![no_main]

#[macro_use]
extern crate log;
extern crate alloc;
use alloc::vec::Vec;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};

use axnet::{TcpSocket, UdpSocket, dns_query};

#[no_mangle]
fn main() {
    info!("开始网络功能测试...");

    // 测试DNS查询
    test_dns();

    // 测试UDP功能
    test_udp();

    // 测试TCP功能
    test_tcp();

    info!("网络功能测试完成");
}

fn test_dns() {
    info!("\n=== DNS测试 ===");
    
    match dns_query("localhost") {
        Ok(ips) => info!("localhost解析结果: {:?}", ips),
        Err(e) => info!("DNS查询失败: {:?}", e),
    }
    
    match dns_query("google.com") {
        Ok(ips) => info!("google.com解析结果: {:?}", ips),
        Err(e) => info!("DNS查询失败: {:?}", e),
    }
}

fn test_udp() {
    info!("\n=== UDP测试 ===");
    
    // 创建UDP socket
    let socket = UdpSocket::new();
    
    // 绑定到本地地址
    let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    match socket.bind(local_addr) {
        Ok(_) => info!("UDP socket绑定成功: {}", local_addr),
        Err(e) => {
            info!("UDP socket绑定失败: {:?}", e);
            return;
        }
    }
    
    // 测试发送数据
    let test_data = b"Hello UDP!";
    let target_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);
    
    match socket.send_to(test_data, target_addr) {
        Ok(len) => info!("UDP发送成功: {} bytes", len),
        Err(e) => info!("UDP发送失败: {:?}", e),
    }
    
    info!("UDP测试完成");
}

fn test_tcp() {
    info!("\n=== TCP测试 ===");
    
    // 测试TCP客户端
    test_tcp_client();
    
    // 测试TCP服务端
    test_tcp_server();
}

fn test_tcp_client() {
    info!("--- TCP客户端测试 ---");
    
    let socket = TcpSocket::new();
    
    // 尝试连接到本地服务
    let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    
    match socket.connect(server_addr) {
        Ok(_) => {
            info!("TCP连接成功: {}", server_addr);
            
            // 发送数据
            let test_data = b"Hello TCP!";
            match socket.send(test_data) {
                Ok(len) => info!("TCP发送成功: {} bytes", len),
                Err(e) => info!("TCP发送失败: {:?}", e),
            }
            
            // 接收数据
            let mut buffer = [0u8; 1024];
            match socket.recv(&mut buffer) {
                Ok(len) => {
                    let received = &buffer[..len];
                    info!("TCP接收成功: {} bytes, 内容: {:?}", len, 
                             core::str::from_utf8(received).unwrap_or("<invalid utf8>"));
                }
                Err(e) => info!("TCP接收失败: {:?}", e),
            }
        }
        Err(e) => info!("TCP连接失败: {:?}", e),
    }
}

fn test_tcp_server() {
    info!("--- TCP服务端测试 ---");
    
    let socket = TcpSocket::new();
    
    // 绑定到本地地址
    let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    match socket.bind(local_addr) {
        Ok(_) => info!("TCP服务端绑定成功: {}", local_addr),
        Err(e) => {
            info!("TCP服务端绑定失败: {:?}", e);
            return;
        }
    }
    
    // 开始监听
    match socket.listen() {
        Ok(_) => info!("TCP服务端开始监听"),
        Err(e) => {
            info!("TCP服务端监听失败: {:?}", e);
            return;
        }
    }
    
    // 尝试接受连接（非阻塞测试）
    match socket.accept() {
        Ok(client_socket) => {
            info!("接受到新连接");
            
            // 处理客户端数据
            let mut buffer = [0u8; 1024];
            match client_socket.recv(&mut buffer) {
                Ok(len) => {
                    let received = &buffer[..len];
                    info!("从客户端接收: {} bytes, 内容: {:?}", len,
                             core::str::from_utf8(received).unwrap_or("<invalid utf8>"));
                    
                    // 回复客户端
                    let response = b"Hello from server!";
                    match client_socket.send(response) {
                        Ok(len) => info!("回复客户端: {} bytes", len),
                        Err(e) => info!("回复客户端失败: {:?}", e),
                    }
                }
                Err(e) => info!("从客户端接收失败: {:?}", e),
            }
        }
        Err(e) => info!("接受连接失败: {:?}", e),
    }
    
    info!("TCP服务端测试完成");
}