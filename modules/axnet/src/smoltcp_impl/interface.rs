//! Network interface implementation
//!
//! Provides Ethernet and loopback interface wrappers, managing network packet transmission and reception

use axdriver::prelude::*;
use axsync::Mutex;
use smoltcp::iface::{Config, Interface};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr};

use crate::error::{NetError, NetResult};
use super::device::{EthernetDevice, LoopbackDevice};
use super::socket_set::SocketSetManager;

/// Ethernet interface wrapper
pub struct EthernetInterface {
    name: &'static str,
    interface: Mutex<Interface>,
    device: Mutex<EthernetDevice>,
}

impl EthernetInterface {
    /// Create new Ethernet interface
    pub fn new(
        name: &'static str,
        net_dev: AxNetDevice,
        ethernet_addr: EthernetAddress,
    ) -> NetResult<Self> {
        let mut device = EthernetDevice::new(net_dev);
        let mut config = Config::new(HardwareAddress::Ethernet(ethernet_addr));
        config.random_seed = 0xA2CE_05A2_CE05_A2CE;

        let interface = Interface::new(config, &mut device, super::current_time());

        Ok(Self {
            name,
            interface: Mutex::new(interface),
            device: Mutex::new(device),
        })
    }

    /// Get interface name
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Get Ethernet address
    pub fn ethernet_address(&self) -> EthernetAddress {
        if let HardwareAddress::Ethernet(addr) = self.interface.lock().hardware_addr() {
            addr
        } else {
            EthernetAddress::default()
        }
    }

    /// Set IP address
    pub fn setup_ip_addr(&self, ip: IpAddress, prefix_len: u8) -> NetResult<()> {
        let cidr = IpCidr::new(ip, prefix_len);
        self.interface
            .lock()
            .update_ip_addrs(|ip_addrs| {
                ip_addrs.clear();
                let _ = ip_addrs.push(cidr);
            });
        Ok(())
    }

    /// Set gateway
    pub fn setup_gateway(&self, gateway: IpAddress) -> NetResult<()> {
        match gateway {
            IpAddress::Ipv4(ipv4) => {
                self.interface
                    .lock()
                    .routes_mut()
                    .add_default_ipv4_route(ipv4)
                    .map_err(|_| NetError::Internal)?;
            }
            IpAddress::Ipv6(ipv6) => {
                self.interface
                    .lock()
                    .routes_mut()
                    .add_default_ipv6_route(ipv6)
                    .map_err(|_| NetError::Internal)?;
            }
        }
        Ok(())
    }

    /// Poll interface, handle network packets
    pub fn poll(&self, timestamp: Instant, socket_set: &SocketSetManager) {
        let mut interface = self.interface.lock();
        let mut device = self.device.lock();
        
        socket_set.with_socket_set_mut(|sockets| {
            let _ = interface.poll(timestamp, &mut *device, sockets);
        });
    }

    /// Get interface context (for Socket operations)
    pub fn context(&self) -> InterfaceContext {
        InterfaceContext {
            interface: &self.interface,
        }
    }

    /// Check link status
    pub fn is_link_up(&self) -> bool {
        self.device.lock().is_link_up()
    }

    /// Get current IP address
    pub fn ip_addr(&self) -> Option<IpAddress> {
        self.interface
            .lock()
            .ip_addrs()
            .iter()
            .next()
            .map(|cidr| cidr.address())
    }

    /// Get gateway address
    pub fn gateway(&self) -> Option<IpAddress> {
        // Simplified implementation, return None for now
        // TODO: Implement proper gateway retrieval logic
        None
    }
}

/// Loopback interface wrapper
pub struct LoopbackInterface {
    interface: Mutex<Interface>,
    device: Mutex<LoopbackDevice>,
}

impl LoopbackInterface {
    /// Create new loopback interface
    pub fn new() -> NetResult<Self> {
        let mut device = LoopbackDevice::new();
        let config = Config::new(HardwareAddress::Ip);

        let mut interface = Interface::new(config, &mut device, super::current_time());

        // Set loopback addresses
        interface
            .update_ip_addrs(|ip_addrs| {
                ip_addrs.clear();
                let _ = ip_addrs.push(IpCidr::new(IpAddress::v4(127, 0, 0, 1), 8));
                let _ = ip_addrs.push(IpCidr::new(IpAddress::v6(0, 0, 0, 0, 0, 0, 0, 1), 128));
            });

        Ok(Self {
            interface: Mutex::new(interface),
            device: Mutex::new(device),
        })
    }

    /// Poll loopback interface
    pub fn poll(&self, timestamp: Instant, socket_set: &SocketSetManager) {
        let mut interface = self.interface.lock();
        let mut device = self.device.lock();
        
        socket_set.with_socket_set_mut(|sockets| {
            let _ = interface.poll(timestamp, &mut *device, sockets);
        });
    }

    /// Get interface context
    pub fn context(&self) -> InterfaceContext {
        InterfaceContext {
            interface: &self.interface,
        }
    }

    /// Join multicast group
    pub fn join_multicast_group(
        &self,
        _multicast_addr: IpAddress,
        _timestamp: Instant,
    ) -> NetResult<()> {
        // Simplified implementation, return success for now
        // TODO: Implement proper multicast group join logic
        Ok(())
    }
}

/// Interface context for Socket operations
pub struct InterfaceContext<'a> {
    interface: &'a Mutex<Interface>,
}

impl<'a> InterfaceContext<'a> {
    /// Get mutable reference to interface
    pub fn with_interface_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Interface) -> R,
    {
        f(&mut *self.interface.lock())
    }

    /// Get immutable reference to interface
    pub fn with_interface<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Interface) -> R,
    {
        f(&*self.interface.lock())
    }
}

impl<'a> core::ops::Deref for InterfaceContext<'a> {
    type Target = Mutex<Interface>;

    fn deref(&self) -> &Self::Target {
        self.interface
    }
}