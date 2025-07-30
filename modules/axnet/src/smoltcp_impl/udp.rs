//! UDP Socket 实现

use core::sync::atomic::{AtomicBool, Ordering};
use axio::{PollState, Read, Write};
use axsync::RwLock;
use smoltcp::iface::SocketHandle;
use smoltcp::socket::udp::{self, BindError, SendError};
use smoltcp::wire::{IpEndpoint, IpListenEndpoint};

use crate::error::{NetError, NetResult};
use crate::stack::{SocketAddr, addr_utils::*};
use super::{network_stack, socket_set::SocketSetManager};

/// UDP Socket 实现
/// 
/// 提供 POSIX 风格的 UDP socket API，支持：
/// - 数据报传输：`send_to`, `recv_from`
/// - 连接模式：`connect`, `send`, `recv`
/// - 地址绑定：`bind`
/// - 非阻塞模式和地址重用
pub struct UdpSocket {
    handle: SocketHandle,
    local_addr: RwLock<Option<IpEndpoint>>,
    peer_addr: RwLock<Option<IpEndpoint>>,
    nonblock: AtomicBool,
    reuse_addr: AtomicBool,
}

impl UdpSocket {
    /// 创建新的 UDP socket
    pub fn new() -> Self {
        let socket = SocketSetManager::new_udp_socket();
        let handle = network_stack().socket_set().add(socket);
        
        Self {
            handle,
            local_addr: RwLock::new(None),
            peer_addr: RwLock::new(None),
            nonblock: AtomicBool::new(false),
            reuse_addr: AtomicBool::new(false),
        }
    }

    /// 获取本地地址和端口
    pub fn local_addr(&self) -> NetResult<SocketAddr> {
        self.local_addr
            .read()
            .as_ref()
            .map(|&addr| to_std_socket_addr(addr))
            .ok_or(NetError::NotConnected)
    }

    /// 获取远程地址和端口
    pub fn peer_addr(&self) -> NetResult<SocketAddr> {
        self.remote_endpoint().map(to_std_socket_addr)
    }

    /// 检查是否为非阻塞模式
    pub fn is_nonblocking(&self) -> bool {
        self.nonblock.load(Ordering::Acquire)
    }

    /// 设置非阻塞模式
    pub fn set_nonblocking(&self, nonblocking: bool) {
        self.nonblock.store(nonblocking, Ordering::Release);
    }

    /// 检查是否启用地址重用
    pub fn is_reuse_addr(&self) -> bool {
        self.reuse_addr.load(Ordering::Acquire)
    }

    /// 设置地址重用
    pub fn set_reuse_addr(&self, reuse_addr: bool) {
        self.reuse_addr.store(reuse_addr, Ordering::Release);
    }

    /// 设置 TTL (生存时间)
    pub fn set_ttl(&self, ttl: u8) {
        network_stack()
            .socket_set()
            .with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
                socket.set_hop_limit(Some(ttl))
            });
    }

    /// 绑定到本地地址
    pub fn bind(&self, mut local_addr: SocketAddr) -> NetResult<()> {
        let mut self_local_addr = self.local_addr.write();

        if self_local_addr.is_some() {
            return Err(NetError::AlreadyConnected);
        }

        if local_addr.port() == 0 {
            local_addr.set_port(self.get_ephemeral_port()?);
        }

        let local_endpoint = from_std_socket_addr(local_addr);
        let listen_endpoint = IpListenEndpoint {
            addr: if !is_unspecified(local_endpoint.addr) {
                Some(local_endpoint.addr)
            } else {
                None
            },
            port: local_endpoint.port,
        };

        // 检查地址冲突
        if !self.is_reuse_addr() {
            network_stack()
                .socket_set()
                .check_bind_conflict(local_endpoint.addr, local_endpoint.port)?;
        }

        network_stack()
            .socket_set()
            .with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
                socket.bind(listen_endpoint).map_err(|e| match e {
                    BindError::InvalidState => NetError::AlreadyConnected,
                    BindError::Unaddressable => NetError::InvalidInput,
                })
            })?;

        *self_local_addr = Some(local_endpoint);
        debug!("UDP socket {}: 绑定到 {}", self.handle, listen_endpoint);
        Ok(())
    }

    /// 连接到远程地址
    pub fn connect(&self, remote_addr: SocketAddr) -> NetResult<()> {
        let mut self_peer_addr = self.peer_addr.write();

        // 如果未绑定，自动绑定到未指定地址
        if self.local_addr.read().is_none() {
            self.bind(to_std_socket_addr(UNSPECIFIED_ENDPOINT))?;
        }

        *self_peer_addr = Some(from_std_socket_addr(remote_addr));
        debug!("UDP socket {}: 连接到 {}", self.handle, remote_addr);
        Ok(())
    }

    /// 发送数据到指定地址
    pub fn send_to(&self, buf: &[u8], remote_addr: SocketAddr) -> NetResult<usize> {
        if remote_addr.port() == 0 || remote_addr.ip().is_unspecified() {
            return Err(NetError::InvalidInput);
        }
        
        self.send_impl(buf, from_std_socket_addr(remote_addr))
    }

    /// 从 socket 接收数据，返回数据长度和发送方地址
    pub fn recv_from(&self, buf: &mut [u8]) -> NetResult<(usize, SocketAddr)> {
        self.recv_impl(|socket| {
            socket
                .recv_slice(buf)
                .map(|(len, meta)| (len, to_std_socket_addr(meta.endpoint)))
                .map_err(|_| NetError::Internal)
        })
    }

    /// 窥视数据（不移除）
    pub fn peek_from(&self, buf: &mut [u8]) -> NetResult<(usize, SocketAddr)> {
        self.recv_impl(|socket| {
            socket
                .peek_slice(buf)
                .map(|(len, meta)| (len, to_std_socket_addr(meta.endpoint)))
                .map_err(|_| NetError::Internal)
        })
    }

    /// 发送数据到已连接的远程地址
    pub fn send(&self, buf: &[u8]) -> NetResult<usize> {
        let remote_endpoint = self.remote_endpoint()?;
        self.send_impl(buf, remote_endpoint)
    }

    /// 从已连接的远程地址接收数据
    pub fn recv(&self, buf: &mut [u8]) -> NetResult<usize> {
        let remote_endpoint = self.remote_endpoint()?;
        self.recv_impl(|socket| {
            let (len, meta) = socket
                .recv_slice(buf)
                .map_err(|_| NetError::Internal)?;
            
            // 检查是否来自连接的远程地址
            if !is_unspecified(remote_endpoint.addr) 
                && remote_endpoint.addr != meta.endpoint.addr {
                return Err(NetError::WouldBlock);
            }
            if remote_endpoint.port != 0 && remote_endpoint.port != meta.endpoint.port {
                return Err(NetError::WouldBlock);
            }
            
            Ok(len)
        })
    }

    /// 关闭 socket
    pub fn shutdown(&self) -> NetResult<()> {
        network_stack().poll_interfaces();
        network_stack()
            .socket_set()
            .with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
                debug!("UDP socket {}: 关闭", self.handle);
                socket.close();
            });
        Ok(())
    }

    /// 轮询 socket 状态
    pub fn poll(&self) -> NetResult<PollState> {
        if self.local_addr.read().is_none() {
            return Ok(PollState {
                readable: false,
                writable: false,
            });
        }
        
        network_stack()
            .socket_set()
            .with_socket::<udp::Socket, _, _>(self.handle, |socket| {
                Ok(PollState {
                    readable: socket.can_recv(),
                    writable: socket.can_send(),
                })
            })
    }

    /// 使用 socket 执行操作（只读）
    pub fn with_socket<R>(&self, f: impl FnOnce(&udp::Socket) -> R) -> R {
        network_stack().socket_set().with_socket(self.handle, f)
    }

    /// 使用 socket 执行操作（可变）
    pub fn with_socket_mut<R>(&self, f: impl FnOnce(&mut udp::Socket) -> R) -> R {
        network_stack().socket_set().with_socket_mut(self.handle, f)
    }
}

// 私有方法实现
impl UdpSocket {
    fn remote_endpoint(&self) -> NetResult<IpEndpoint> {
        self.peer_addr
            .read()
            .as_ref()
            .copied()
            .ok_or(NetError::NotConnected)
    }

    fn get_ephemeral_port(&self) -> NetResult<u16> {
        use crate::stack::PortManager;
        static PORT_MANAGER: PortManager = PortManager::new();
        
        // UDP 端口分配比较简单，不需要检查监听表
        Ok(PORT_MANAGER.next_ephemeral_port())
    }

    fn send_impl(&self, buf: &[u8], remote_endpoint: IpEndpoint) -> NetResult<usize> {
        if self.local_addr.read().is_none() {
            return Err(NetError::NotConnected);
        }

        self.block_on(|| {
            network_stack()
                .socket_set()
                .with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
                    if !socket.is_open() {
                        Err(NetError::NotConnected)
                    } else if socket.can_send() {
                        socket
                            .send_slice(buf, remote_endpoint)
                            .map_err(|e| match e {
                                SendError::BufferFull => NetError::WouldBlock,
                                SendError::Unaddressable => NetError::HostUnreachable,
                            })?;
                        Ok(buf.len())
                    } else {
                        Err(NetError::WouldBlock)
                    }
                })
        })
    }

    fn recv_impl<F, T>(&self, mut op: F) -> NetResult<T>
    where
        F: FnMut(&mut udp::Socket) -> NetResult<T>,
    {
        if self.local_addr.read().is_none() {
            return Err(NetError::NotConnected);
        }

        self.block_on(|| {
            network_stack()
                .socket_set()
                .with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
                    if !socket.is_open() {
                        Err(NetError::NotConnected)
                    } else if socket.can_recv() {
                        op(socket)
                    } else {
                        Err(NetError::WouldBlock)
                    }
                })
        })
    }

    fn block_on<F, T>(&self, mut f: F) -> NetResult<T>
    where
        F: FnMut() -> NetResult<T>,
    {
        if self.is_nonblocking() {
            f()
        } else {
            loop {
                network_stack().poll_interfaces();
                match f() {
                    Ok(t) => return Ok(t),
                    Err(NetError::WouldBlock) => axtask::yield_now(),
                    Err(e) => return Err(e),
                }
            }
        }
    }
}

impl Read for UdpSocket {
    fn read(&mut self, buf: &mut [u8]) -> axerrno::AxResult<usize> {
        self.recv(buf).map_err(|e| e.into())
    }
}

impl Write for UdpSocket {
    fn write(&mut self, buf: &[u8]) -> axerrno::AxResult<usize> {
        self.send(buf).map_err(|e| e.into())
    }

    fn flush(&mut self) -> axerrno::AxResult {
        Ok(()) // UDP 无需刷新
    }
}

impl Drop for UdpSocket {
    fn drop(&mut self) {
        let _ = self.shutdown();
        network_stack().socket_set().remove(self.handle);
    }
}