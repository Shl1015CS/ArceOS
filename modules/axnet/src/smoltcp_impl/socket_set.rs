//! Socket集合管理器
//!
//! 管理所有的TCP和UDP socket，提供统一的socket操作接口

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use axsync::Mutex;
use smoltcp::iface::{SocketHandle, SocketSet};
use smoltcp::socket::{tcp, udp};
use smoltcp::wire::{IpAddress, IpEndpoint};

use crate::error::{NetError, NetResult};

/// Socket集合管理器
pub struct SocketSetManager {
    socket_set: Mutex<SocketSet<'static>>,
    tcp_sockets: Mutex<BTreeMap<SocketHandle, TcpSocketInfo>>,
    udp_sockets: Mutex<BTreeMap<SocketHandle, UdpSocketInfo>>,
}

/// TCP Socket信息
#[derive(Debug, Clone)]
struct TcpSocketInfo {
    local_addr: Option<IpEndpoint>,
    peer_addr: Option<IpEndpoint>,
    state: TcpState,
}

/// UDP Socket信息
#[derive(Debug, Clone)]
struct UdpSocketInfo {
    local_addr: Option<IpEndpoint>,
    peer_addr: Option<IpEndpoint>,
}

/// TCP连接状态
#[derive(Debug, Clone, PartialEq)]
enum TcpState {
    Closed,
    Listening,
    Connecting,
    Connected,
}

impl SocketSetManager {
    /// 创建新的Socket集合管理器
    pub fn new() -> Self {
        Self {
            socket_set: Mutex::new(SocketSet::new(vec![])),
            tcp_sockets: Mutex::new(BTreeMap::new()),
            udp_sockets: Mutex::new(BTreeMap::new()),
        }
    }

    /// 创建新的TCP socket
    pub fn new_tcp_socket() -> tcp::Socket<'static> {
        let rx_buffer = tcp::SocketBuffer::new(vec![0; 65536]);
        let tx_buffer = tcp::SocketBuffer::new(vec![0; 65536]);
        tcp::Socket::new(rx_buffer, tx_buffer)
    }

    /// 创建新的UDP socket
    pub fn new_udp_socket() -> udp::Socket<'static> {
        let rx_buffer = udp::PacketBuffer::new(
            vec![udp::PacketMetadata::EMPTY; 16],
            vec![0; 65536],
        );
        let tx_buffer = udp::PacketBuffer::new(
            vec![udp::PacketMetadata::EMPTY; 16],
            vec![0; 65536],
        );
        udp::Socket::new(rx_buffer, tx_buffer)
    }

    /// 创建新的DNS socket
    pub fn new_dns_socket() -> smoltcp::socket::dns::Socket<'static> {
        let servers = [
            smoltcp::wire::IpAddress::v4(8, 8, 8, 8),
            smoltcp::wire::IpAddress::v4(8, 8, 4, 4),
        ];
        let queries = vec![];
        smoltcp::socket::dns::Socket::new(&servers, queries)
    }

    /// 添加socket到集合中
    pub fn add<T>(&self, socket: T) -> SocketHandle
    where
        T: smoltcp::socket::AnySocket<'static>,
    {
        let handle = self.socket_set.lock().add(socket);
        
        // 根据socket类型记录信息
        // 暂时简化实现，不区分socket类型
        // TODO: 实现正确的socket类型检测
        
        handle
    }

    /// 从集合中移除socket
    pub fn remove(&self, handle: SocketHandle) {
        self.socket_set.lock().remove(handle);
        self.tcp_sockets.lock().remove(&handle);
        self.udp_sockets.lock().remove(&handle);
    }

    /// 使用TCP socket执行操作（只读）
    pub fn with_socket<T, F, R>(&self, handle: SocketHandle, f: F) -> R
    where
        T: smoltcp::socket::AnySocket<'static>,
        F: FnOnce(&T) -> R,
    {
        let socket_set = self.socket_set.lock();
        let socket = socket_set.get::<T>(handle);
        f(socket)
    }

    /// 使用TCP socket执行操作（可变）
    pub fn with_socket_mut<T, F, R>(&self, handle: SocketHandle, f: F) -> R
    where
        T: smoltcp::socket::AnySocket<'static>,
        F: FnOnce(&mut T) -> R,
    {
        let mut socket_set = self.socket_set.lock();
        let socket = socket_set.get_mut::<T>(handle);
        f(socket)
    }

    /// 使用整个socket集合执行操作（只读）
    pub fn with_socket_set<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&SocketSet) -> R,
    {
        f(&*self.socket_set.lock())
    }

    /// 使用整个socket集合执行操作（可变）
    pub fn with_socket_set_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut SocketSet) -> R,
    {
        f(&mut *self.socket_set.lock())
    }

    /// 检查地址绑定冲突
    pub fn check_bind_conflict(&self, addr: IpAddress, port: u16) -> NetResult<()> {
        // 检查TCP socket冲突
        for info in self.tcp_sockets.lock().values() {
            if let Some(local_addr) = info.local_addr {
                if local_addr.port == port {
                    if local_addr.addr == addr || addr.is_unspecified() || local_addr.addr.is_unspecified() {
                        return Err(NetError::AddrInUse);
                    }
                }
            }
        }

        // 检查UDP socket冲突
        for info in self.udp_sockets.lock().values() {
            if let Some(local_addr) = info.local_addr {
                if local_addr.port == port {
                    if local_addr.addr == addr || addr.is_unspecified() || local_addr.addr.is_unspecified() {
                        return Err(NetError::AddrInUse);
                    }
                }
            }
        }

        Ok(())
    }

    /// 更新TCP socket信息
    pub fn update_tcp_socket_info(
        &self,
        handle: SocketHandle,
        local_addr: Option<IpEndpoint>,
        peer_addr: Option<IpEndpoint>,
        state: TcpState,
    ) {
        if let Some(info) = self.tcp_sockets.lock().get_mut(&handle) {
            if let Some(addr) = local_addr {
                info.local_addr = Some(addr);
            }
            if let Some(addr) = peer_addr {
                info.peer_addr = Some(addr);
            }
            info.state = state;
        }
    }

    /// 更新UDP socket信息
    pub fn update_udp_socket_info(
        &self,
        handle: SocketHandle,
        local_addr: Option<IpEndpoint>,
        peer_addr: Option<IpEndpoint>,
    ) {
        if let Some(info) = self.udp_sockets.lock().get_mut(&handle) {
            if let Some(addr) = local_addr {
                info.local_addr = Some(addr);
            }
            if let Some(addr) = peer_addr {
                info.peer_addr = Some(addr);
            }
        }
    }

    /// 获取TCP连接数量
    pub fn tcp_connection_count(&self) -> usize {
        self.tcp_sockets
            .lock()
            .values()
            .filter(|info| info.state == TcpState::Connected)
            .count()
    }

    /// 获取UDP socket数量
    pub fn udp_socket_count(&self) -> usize {
        self.udp_sockets.lock().len()
    }

    /// 获取所有socket数量
    pub fn total_socket_count(&self) -> usize {
        self.tcp_sockets.lock().len() + self.udp_sockets.lock().len()
    }

    /// 清理已关闭的socket
    pub fn cleanup_closed_sockets(&self) {
        let mut to_remove = Vec::new();
        
        // 检查TCP socket
        {
            let socket_set = self.socket_set.lock();
            for (&handle, info) in self.tcp_sockets.lock().iter() {
                let socket = socket_set.get::<tcp::Socket>(handle);
                if !socket.is_active() && info.state != TcpState::Listening {
                    to_remove.push(handle);
                }
            }
        }
        
        // 移除已关闭的socket
        for handle in to_remove {
            self.remove(handle);
        }
    }
}

impl Default for SocketSetManager {
    fn default() -> Self {
        Self::new()
    }
}