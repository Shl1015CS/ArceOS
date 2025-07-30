//! ArceOS 网络模块 - 基于smoltcp的完整TCP/UDP实现
//!
//! 提供完整的网络功能，包括TCP和UDP协议栈，让操作系统具备访问网络的能力。
//! 基于smoltcp网络栈实现，提供POSIX兼容的socket接口。
//!
//! # 核心功能
//!
//! - [`TcpSocket`]: 完整的TCP socket实现，支持客户端和服务端
//! - [`UdpSocket`]: 完整的UDP socket实现，支持数据报通信
//! - [`dns_query`]: DNS域名解析功能
//! - 网络接口管理和数据包处理
//! - 多播和广播支持
//!
//! # 特性
//!
//! - 完整的TCP状态机实现
//! - UDP数据报可靠传输
//! - 非阻塞I/O支持
//! - 地址重用和端口管理
//! - IPv4/IPv6双栈支持
//! - DNS解析和缓存

#![no_std]

extern crate alloc;
#[macro_use]
extern crate log;

mod error;
mod stack;

cfg_if::cfg_if! {
    if #[cfg(feature = "smoltcp")] {
        mod smoltcp_impl;
        use smoltcp_impl as net_impl;
    }
}

// 重新导出核心类型
pub use self::error::{NetError, NetResult};
pub use self::net_impl::{TcpSocket, UdpSocket};
pub use self::net_impl::{dns_query, poll_interfaces};
pub use self::stack::SocketAddr;

// 重新导出smoltcp类型以便上层使用
pub use smoltcp::time::Duration;
pub use smoltcp::wire::{
    IpAddress as IpAddr, 
    IpEndpoint as SocketEndpoint, 
    Ipv4Address as Ipv4Addr, 
    Ipv6Address as Ipv6Addr,
};

use axdriver::{prelude::*, AxDeviceContainer};

/// 初始化网络子系统
/// 
/// 通过网卡设备初始化完整的网络栈，包括：
/// - 以太网接口配置
/// - IP地址和路由设置
/// - Socket管理器初始化
/// - DNS服务配置
pub fn init_network(mut net_devs: AxDeviceContainer<AxNetDevice>) {
    info!("初始化网络子系统...");

    let dev = net_devs.take_one().expect("未找到网卡设备!");
    info!("  使用网卡设备: {:?}", dev.device_name());
    
    net_impl::init_network_stack(dev);
    info!("网络子系统初始化完成");
}

/// 获取网络栈统计信息
pub fn get_network_stats() -> NetResult<NetworkStats> {
    net_impl::get_network_stats()
}

/// 网络统计信息
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub tx_packets: u64,
    pub rx_packets: u64,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub tcp_connections: usize,
    pub udp_sockets: usize,
}

#[cfg(test)]
mod tests;