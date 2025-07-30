//! 网络栈通用定义和工具

use core::net::{IpAddr, SocketAddr as StdSocketAddr};
use smoltcp::wire::{IpAddress, IpEndpoint};

/// Socket 地址类型别名
pub type SocketAddr = StdSocketAddr;

/// IP 地址转换工具
pub mod addr_utils {
    use super::*;
    use smoltcp::wire::{Ipv4Address, Ipv6Address};

    /// 将标准库 IpAddr 转换为 smoltcp IpAddress
    pub const fn from_std_ip(ip: IpAddr) -> IpAddress {
        match ip {
            IpAddr::V4(ipv4) => IpAddress::Ipv4(Ipv4Address(ipv4.octets())),
            IpAddr::V6(ipv6) => IpAddress::Ipv6(Ipv6Address(ipv6.octets())),
        }
    }

    /// 将 smoltcp IpAddress 转换为标准库 IpAddr
    pub const fn to_std_ip(ip: IpAddress) -> IpAddr {
        match ip {
            IpAddress::Ipv4(ipv4) => IpAddr::V4(unsafe { core::mem::transmute(ipv4.0) }),
            IpAddress::Ipv6(ipv6) => IpAddr::V6(unsafe { core::mem::transmute(ipv6.0) }),
        }
    }

    /// 将标准库 SocketAddr 转换为 smoltcp IpEndpoint
    pub const fn from_std_socket_addr(addr: SocketAddr) -> IpEndpoint {
        IpEndpoint {
            addr: from_std_ip(addr.ip()),
            port: addr.port(),
        }
    }

    /// 将 smoltcp IpEndpoint 转换为标准库 SocketAddr
    pub const fn to_std_socket_addr(endpoint: IpEndpoint) -> SocketAddr {
        SocketAddr::new(to_std_ip(endpoint.addr), endpoint.port)
    }

    /// 检查 IP 地址是否为未指定地址
    pub fn is_unspecified(ip: IpAddress) -> bool {
        match ip {
            IpAddress::Ipv4(ipv4) => ipv4.is_unspecified(),
            IpAddress::Ipv6(ipv6) => ipv6.is_unspecified(),
        }
    }

    /// 未指定的 IP 地址常量
    pub const UNSPECIFIED_IP: IpAddress = IpAddress::v4(0, 0, 0, 0);
    
    /// 未指定的端点常量
    pub const UNSPECIFIED_ENDPOINT: IpEndpoint = IpEndpoint::new(UNSPECIFIED_IP, 0);
}

/// 端口管理器
pub struct PortManager {
    next_ephemeral: core::sync::atomic::AtomicU16,
}

impl PortManager {
    /// 创建新的端口管理器
    pub const fn new() -> Self {
        Self {
            next_ephemeral: core::sync::atomic::AtomicU16::new(0xc000),
        }
    }

    /// 获取下一个临时端口
    pub fn next_ephemeral_port(&self) -> u16 {
        use core::sync::atomic::Ordering;
        
        const PORT_START: u16 = 0xc000;
        const PORT_END: u16 = 0xffff;
        
        let current = self.next_ephemeral.load(Ordering::Relaxed);
        let next = if current >= PORT_END { PORT_START } else { current + 1 };
        self.next_ephemeral.store(next, Ordering::Relaxed);
        current
    }
}