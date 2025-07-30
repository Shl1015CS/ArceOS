//! 网络接口实现
//!
//! 提供以太网和回环接口的封装，管理网络数据包的收发

use alloc::vec::Vec;
use axdriver::prelude::*;
use axsync::Mutex;
use smoltcp::iface::{Config, Interface, SocketSet};
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address};

use crate::error::{NetError, NetResult};
use super::device::{EthernetDevice, LoopbackDevice};
use super::socket_set::SocketSetManager;

/// 以太网接口封装
pub struct EthernetInterface {
    name: &'static str,
    interface: Mutex<Interface>,
    device: Mutex<EthernetDevice>,
}

impl EthernetInterface {
    /// 创建新的以太网接口
    pub fn new(
        name: &'static str,
        net_dev: AxNetDevice,
        ethernet_addr: EthernetAddress,
    ) -> NetResult<Self> {
        let device = EthernetDevice::new(net_dev);
        let mut config = Config::new(HardwareAddress::Ethernet(ethernet_addr));
        config.random_seed = 0xA2CE_05A2_CE05_A2CE;

        let interface = Interface::new(config, &mut device.clone(), super::current_time());

        Ok(Self {
            name,
            interface: Mutex::new(interface),
            device: Mutex::new(device),
        })
    }

    /// 获取接口名称
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// 获取以太网地址
    pub fn ethernet_address(&self) -> EthernetAddress {
        if let HardwareAddress::Ethernet(addr) = self.interface.lock().hardware_addr() {
            addr
        } else {
            EthernetAddress::default()
        }
    }

    /// 设置IP地址
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

    /// 设置网关
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

    /// 轮询接口，处理网络数据包
    pub fn poll(&self, timestamp: Instant, socket_set: &SocketSetManager) {
        let mut interface = self.interface.lock();
        let mut device = self.device.lock();
        
        socket_set.with_socket_set_mut(|sockets| {
            let _ = interface.poll(timestamp, &mut *device, sockets);
        });
    }

    /// 获取接口上下文（用于Socket操作）
    pub fn context(&self) -> InterfaceContext {
        InterfaceContext {
            interface: &self.interface,
        }
    }

    /// 检查链路状态
    pub fn is_link_up(&self) -> bool {
        self.device.lock().is_link_up()
    }

    /// 获取当前IP地址
    pub fn ip_addr(&self) -> Option<IpAddress> {
        self.interface
            .lock()
            .ip_addrs()
            .iter()
            .next()
            .map(|cidr| cidr.address())
    }

    /// 获取网关地址
    pub fn gateway(&self) -> Option<IpAddress> {
        // 简化实现，返回默认路由
        self.interface
            .lock()
            .routes()
            .iter()
            .find_map(|(_, route)| {
                if route.prefix_len == 0 {
                    Some(route.via_router)
                } else {
                    None
                }
            })
    }
}

/// 回环接口封装
pub struct LoopbackInterface {
    interface: Mutex<Interface>,
    device: Mutex<LoopbackDevice>,
}

impl LoopbackInterface {
    /// 创建新的回环接口
    pub fn new() -> NetResult<Self> {
        let device = LoopbackDevice::new();
        let config = Config::new(HardwareAddress::Ip);

        let mut interface = Interface::new(config, &mut device.clone(), super::current_time());

        // 设置回环地址
        interface
            .update_ip_addrs(|ip_addrs| {
                ip_addrs.clear();
                ip_addrs
                    .push(IpCidr::new(IpAddress::v4(127, 0, 0, 1), 8))
                    .map_err(|_| NetError::Internal)?;
                ip_addrs
                    .push(IpCidr::new(IpAddress::v6(0, 0, 0, 0, 0, 0, 0, 1), 128))
                    .map_err(|_| NetError::Internal)?;
                Ok(())
            })
            .map_err(|_| NetError::Internal)?;

        Ok(Self {
            interface: Mutex::new(interface),
            device: Mutex::new(device),
        })
    }

    /// 轮询回环接口
    pub fn poll(&self, timestamp: Instant, socket_set: &SocketSetManager) {
        let mut interface = self.interface.lock();
        let mut device = self.device.lock();
        
        socket_set.with_socket_set_mut(|sockets| {
            let _ = interface.poll(timestamp, &mut *device, sockets);
        });
    }

    /// 获取接口上下文
    pub fn context(&self) -> InterfaceContext {
        InterfaceContext {
            interface: &self.interface,
        }
    }

    /// 加入多播组
    pub fn join_multicast_group(
        &self,
        multicast_addr: IpAddress,
        _timestamp: Instant,
    ) -> NetResult<()> {
        match multicast_addr {
            IpAddress::Ipv4(addr) => {
                self.interface
                    .lock()
                    .join_multicast_group(addr, super::current_time())
                    .map_err(|_| NetError::Internal)
            }
            IpAddress::Ipv6(addr) => {
                self.interface
                    .lock()
                    .join_multicast_group(addr, super::current_time())
                    .map_err(|_| NetError::Internal)
            }
        }
    }
}

/// 接口上下文，用于Socket操作
pub struct InterfaceContext<'a> {
    interface: &'a Mutex<Interface>,
}

impl<'a> InterfaceContext<'a> {
    /// 获取接口的可变引用
    pub fn with_interface_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Interface) -> R,
    {
        f(&mut *self.interface.lock())
    }

    /// 获取接口的不可变引用
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