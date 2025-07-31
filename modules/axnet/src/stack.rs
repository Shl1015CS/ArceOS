//! Network stack common definitions and utilities

use core::net::{IpAddr, SocketAddr as StdSocketAddr};
use smoltcp::wire::{IpAddress, IpEndpoint};

/// Socket address type alias
pub type SocketAddr = StdSocketAddr;

/// IP address conversion utilities
pub mod addr_utils {
    use super::*;
    use smoltcp::wire::{Ipv4Address, Ipv6Address};

    /// Convert standard library IpAddr to smoltcp IpAddress
    pub const fn from_std_ip(ip: IpAddr) -> IpAddress {
        match ip {
            IpAddr::V4(ipv4) => IpAddress::Ipv4(Ipv4Address(ipv4.octets())),
            IpAddr::V6(ipv6) => IpAddress::Ipv6(Ipv6Address(ipv6.octets())),
        }
    }

    /// Convert smoltcp IpAddress to standard library IpAddr
    pub const fn to_std_ip(ip: IpAddress) -> IpAddr {
        match ip {
            IpAddress::Ipv4(ipv4) => IpAddr::V4(unsafe { core::mem::transmute(ipv4.0) }),
            IpAddress::Ipv6(ipv6) => IpAddr::V6(unsafe { core::mem::transmute(ipv6.0) }),
        }
    }

    /// Convert standard library SocketAddr to smoltcp IpEndpoint
    pub const fn from_std_socket_addr(addr: SocketAddr) -> IpEndpoint {
        IpEndpoint {
            addr: from_std_ip(addr.ip()),
            port: addr.port(),
        }
    }

    /// Convert smoltcp IpEndpoint to standard library SocketAddr
    pub const fn to_std_socket_addr(endpoint: IpEndpoint) -> SocketAddr {
        SocketAddr::new(to_std_ip(endpoint.addr), endpoint.port)
    }

    /// Check if IP address is unspecified
    pub fn is_unspecified(ip: IpAddress) -> bool {
        match ip {
            IpAddress::Ipv4(ipv4) => ipv4.is_unspecified(),
            IpAddress::Ipv6(ipv6) => ipv6.is_unspecified(),
        }
    }

    /// Unspecified IP address constant
    pub const UNSPECIFIED_IP: IpAddress = IpAddress::v4(0, 0, 0, 0);
    
    /// Unspecified endpoint constant
    pub const UNSPECIFIED_ENDPOINT: IpEndpoint = IpEndpoint::new(UNSPECIFIED_IP, 0);
}

/// Port manager
pub struct PortManager {
    next_ephemeral: core::sync::atomic::AtomicU16,
}

impl PortManager {
    /// Create new port manager
    pub const fn new() -> Self {
        Self {
            next_ephemeral: core::sync::atomic::AtomicU16::new(0xc000),
        }
    }

    /// Get next ephemeral port
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