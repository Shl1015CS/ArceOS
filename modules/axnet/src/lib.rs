//! ArceOS Network Module - Complete TCP/UDP implementation based on smoltcp
//!
//! Provides complete network functionality, including TCP and UDP protocol stacks, 
//! enabling the operating system to access networks.
//! Based on smoltcp network stack implementation, providing POSIX-compatible socket interface.
//!
//! # Core Features
//!
//! - [`TcpSocket`]: Complete TCP socket implementation, supporting client and server
//! - [`UdpSocket`]: Complete UDP socket implementation, supporting datagram communication
//! - [`dns_query`]: DNS domain name resolution functionality
//! - Network interface management and packet processing
//! - Multicast and broadcast support
//!
//! # Features
//!
//! - Complete TCP state machine implementation
//! - UDP datagram reliable transmission
//! - Non-blocking I/O support
//! - Address reuse and port management
//! - IPv4/IPv6 dual stack support
//! - DNS resolution and caching

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

/// Initialize network subsystem
/// 
/// Initialize complete network stack through network device, including:
/// - Ethernet interface configuration
/// - IP address and routing setup
/// - Socket manager initialization
/// - DNS service configuration
pub fn init_network(mut net_devs: AxDeviceContainer<AxNetDevice>) {
    info!("Initializing network subsystem...");

    let dev = net_devs.take_one().expect("Network device not found!");
    info!("  Using network device: {:?}", dev.device_name());
    
    net_impl::init_network_stack(dev);
    info!("Network subsystem initialization completed");
}

/// Get network stack statistics
pub fn get_network_stats() -> NetResult<NetworkStats> {
    net_impl::get_network_stats()
}

/// Network statistics
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