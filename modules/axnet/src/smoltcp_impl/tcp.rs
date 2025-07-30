//! TCP Socket 实现

use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use core::cell::UnsafeCell;
use axio::{PollState, Read, Write};
use axsync::Mutex;
use axtask::yield_now;
use smoltcp::iface::SocketHandle;
use smoltcp::socket::tcp::{self, ConnectError, State};
use smoltcp::wire::{IpEndpoint, IpListenEndpoint};

use crate::error::{NetError, NetResult};
use crate::stack::{SocketAddr, addr_utils::*};
use super::{network_stack, current_time};

// TCP Socket 状态
const STATE_CLOSED: u8 = 0;
const STATE_BUSY: u8 = 1;
const STATE_CONNECTING: u8 = 2;
const STATE_CONNECTED: u8 = 3;
const STATE_LISTENING: u8 = 4;

/// TCP Socket 实现
/// 
/// 提供 POSIX 风格的 TCP socket API，支持：
/// - 客户端连接：`connect`
/// - 服务端监听：`bind`, `listen`, `accept`
/// - 数据传输：`send`, `recv`
/// - 非阻塞模式和地址重用
pub struct TcpSocket {
    state: AtomicU8,
    handle: UnsafeCell<Option<SocketHandle>>,
    local_addr: UnsafeCell<IpEndpoint>,
    peer_addr: UnsafeCell<IpEndpoint>,
    nonblock: AtomicBool,
    reuse_addr: AtomicBool,
}

unsafe impl Sync for TcpSocket {}

impl TcpSocket {
    /// 创建新的 TCP socket
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(STATE_CLOSED),
            handle: UnsafeCell::new(None),
            local_addr: UnsafeCell::new(UNSPECIFIED_ENDPOINT),
            peer_addr: UnsafeCell::new(UNSPECIFIED_ENDPOINT),
            nonblock: AtomicBool::new(false),
            reuse_addr: AtomicBool::new(false),
        }
    }

    /// 创建已连接的 TCP socket（内部使用）
    const fn new_connected(
        handle: SocketHandle,
        local_addr: IpEndpoint,
        peer_addr: IpEndpoint,
    ) -> Self {
        Self {
            state: AtomicU8::new(STATE_CONNECTED),
            handle: UnsafeCell::new(Some(handle)),
            local_addr: UnsafeCell::new(local_addr),
            peer_addr: UnsafeCell::new(peer_addr),
            nonblock: AtomicBool::new(false),
            reuse_addr: AtomicBool::new(false),
        }
    }

    /// 获取本地地址和端口
    pub fn local_addr(&self) -> NetResult<SocketAddr> {
        match self.get_state() {
            STATE_CONNECTED | STATE_LISTENING | STATE_CLOSED => {
                let local_addr = unsafe { self.local_addr.get().read() };
                if local_addr == UNSPECIFIED_ENDPOINT {
                    Err(NetError::NotConnected)
                } else {
                    Ok(to_std_socket_addr(local_addr))
                }
            }
            _ => Err(NetError::NotConnected),
        }
    }

    /// 获取远程地址和端口
    pub fn peer_addr(&self) -> NetResult<SocketAddr> {
        match self.get_state() {
            STATE_CONNECTED => {
                let peer_addr = unsafe { self.peer_addr.get().read() };
                Ok(to_std_socket_addr(peer_addr))
            }
            _ => Err(NetError::NotConnected),
        }
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

    /// 连接到远程地址
    pub fn connect(&self, remote_addr: SocketAddr) -> NetResult<()> {
        self.update_state(STATE_CLOSED, STATE_CONNECTING, || {
            let handle = unsafe { self.handle.get().read() }
                .unwrap_or_else(|| {
                    network_stack().socket_set().add(
                        super::socket_set::SocketSetManager::new_tcp_socket()
                    )
                });

            let remote_endpoint = from_std_socket_addr(remote_addr);
            let bound_endpoint = self.bound_endpoint()?;

            // 选择合适的接口
            let (local_endpoint, remote_endpoint) = if remote_endpoint.addr.as_bytes()[0] == 127 {
                let interface_ctx = network_stack().loopback_interface().context();
                interface_ctx.with_interface_mut(|interface| {
                    network_stack()
                        .socket_set()
                        .with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                            socket
                                .connect(interface, remote_endpoint, bound_endpoint)
                                .map_err(|e| match e {
                                    ConnectError::InvalidState => NetError::AlreadyConnected,
                                    ConnectError::Unaddressable => NetError::ConnectionRefused,
                                })?;
                            
                            Ok::<(IpEndpoint, IpEndpoint), NetError>((
                                socket.local_endpoint().unwrap(),
                                socket.remote_endpoint().unwrap(),
                            ))
                        })
                })?
            } else {
                let interface_ctx = network_stack().eth_interface().context();
                interface_ctx.with_interface_mut(|interface| {
                    network_stack()
                        .socket_set()
                        .with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                            socket
                                .connect(interface, remote_endpoint, bound_endpoint)
                                .map_err(|e| match e {
                                    ConnectError::InvalidState => NetError::AlreadyConnected,
                                    ConnectError::Unaddressable => NetError::ConnectionRefused,
                                })?;
                            
                            Ok::<(IpEndpoint, IpEndpoint), NetError>((
                                socket.local_endpoint().unwrap(),
                                socket.remote_endpoint().unwrap(),
                            ))
                        })
                })
            };

            unsafe {
                self.local_addr.get().write(local_endpoint);
                self.peer_addr.get().write(remote_endpoint);
                self.handle.get().write(Some(handle));
            }
            Ok(())
        })
        .unwrap_or_else(|_| Err(NetError::AlreadyConnected))?;

        // 让出 CPU 时间给服务端处理
        yield_now();

        if self.is_nonblocking() {
            Err(NetError::WouldBlock)
        } else {
            self.block_on(|| {
                let PollState { writable, .. } = self.poll_connect()?;
                if !writable {
                    Err(NetError::WouldBlock)
                } else if self.get_state() == STATE_CONNECTED {
                    Ok(())
                } else {
                    Err(NetError::ConnectionRefused)
                }
            })
        }
    }

    /// 绑定到本地地址
    pub fn bind(&self, mut local_addr: SocketAddr) -> NetResult<()> {
        self.update_state(STATE_CLOSED, STATE_CLOSED, || {
            if local_addr.port() == 0 {
                local_addr.set_port(self.get_ephemeral_port()?);
            }

            let old_addr = unsafe { self.local_addr.get().read() };
            if old_addr != UNSPECIFIED_ENDPOINT {
                return Err(NetError::AlreadyConnected);
            }

            let local_endpoint = from_std_socket_addr(local_addr);
            
            // 检查地址冲突
            if !self.is_reuse_addr() {
                network_stack()
                    .socket_set()
                    .check_bind_conflict(local_endpoint.addr, local_endpoint.port)?;
            }

            let handle = unsafe { self.handle.get().read() }
                .unwrap_or_else(|| {
                    network_stack().socket_set().add(
                        super::socket_set::SocketSetManager::new_tcp_socket()
                    )
                });

            let bound_endpoint = self.bound_endpoint_from_addr(local_endpoint)?;
            network_stack()
                .socket_set()
                .with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                    socket.set_bound_endpoint(bound_endpoint);
                });

            unsafe {
                self.local_addr.get().write(local_endpoint);
                self.handle.get().write(Some(handle));
            }

            Ok(())
        })
        .unwrap_or_else(|_| Err(NetError::AlreadyConnected))
    }

    /// 开始监听连接
    pub fn listen(&self) -> NetResult<()> {
        self.update_state(STATE_CLOSED, STATE_LISTENING, || {
            let bound_endpoint = self.bound_endpoint()?;
            
            unsafe {
                (*self.local_addr.get()).port = bound_endpoint.port;
            }
            
            network_stack().listen_table().listen(bound_endpoint)?;
            debug!("TCP socket 开始监听 {}", bound_endpoint);
            Ok(())
        })
        .unwrap_or(Ok(())) // 忽略重复监听
    }

    /// 接受新连接
    pub fn accept(&self) -> NetResult<TcpSocket> {
        if !self.is_listening() {
            return Err(NetError::InvalidInput);
        }

        let local_port = unsafe { self.local_addr.get().read().port };
        self.block_on(|| {
            let (handle, (local_addr, peer_addr)) = 
                network_stack().listen_table().accept(local_port)?;
            debug!("TCP socket 接受新连接 {}", peer_addr);
            Ok(TcpSocket::new_connected(handle, local_addr, peer_addr))
        })
    }

    /// 发送数据
    pub fn send(&self, buf: &[u8]) -> NetResult<usize> {
        if self.is_connecting() {
            return Err(NetError::WouldBlock);
        } else if !self.is_connected() {
            return Err(NetError::NotConnected);
        }

        let handle = unsafe { self.handle.get().read().unwrap() };
        self.block_on(|| {
            network_stack()
                .socket_set()
                .with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                    if !socket.is_active() || !socket.may_send() {
                        Err(NetError::ConnectionReset)
                    } else if socket.can_send() {
                        let len = socket
                            .send_slice(buf)
                            .map_err(|_| NetError::Internal)?;
                        Ok(len)
                    } else {
                        Err(NetError::WouldBlock)
                    }
                })
        })
    }

    /// 接收数据
    pub fn recv(&self, buf: &mut [u8]) -> NetResult<usize> {
        if self.is_connecting() {
            return Err(NetError::WouldBlock);
        } else if !self.is_connected() {
            return Err(NetError::NotConnected);
        }

        let handle = unsafe { self.handle.get().read().unwrap() };
        self.block_on(|| {
            network_stack()
                .socket_set()
                .with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                    if socket.recv_queue() > 0 {
                        let len = socket
                            .recv_slice(buf)
                            .map_err(|_| NetError::Internal)?;
                        Ok(len)
                    } else if !socket.is_active() {
                        Err(NetError::ConnectionRefused)
                    } else if !socket.may_recv() {
                        Ok(0) // 连接已关闭
                    } else {
                        Err(NetError::WouldBlock)
                    }
                })
        })
    }

    /// 关闭连接
    pub fn shutdown(&self) -> NetResult<()> {
        // 关闭流连接
        self.update_state(STATE_CONNECTED, STATE_CLOSED, || {
            let handle = unsafe { self.handle.get().read().unwrap() };
            network_stack()
                .socket_set()
                .with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                    debug!("TCP socket {}: 关闭连接", handle);
                    socket.close();
                });
            unsafe { 
                self.local_addr.get().write(UNSPECIFIED_ENDPOINT);
                self.peer_addr.get().write(UNSPECIFIED_ENDPOINT);
            }
            network_stack().poll_interfaces();
            Ok(())
        })
        .unwrap_or(Ok(()))?;

        // 关闭监听
        self.update_state(STATE_LISTENING, STATE_CLOSED, || {
            let local_port = unsafe { self.local_addr.get().read().port };
            unsafe { self.local_addr.get().write(UNSPECIFIED_ENDPOINT); }
            network_stack().listen_table().unlisten(local_port);
            network_stack().poll_interfaces();
            Ok(())
        })
        .unwrap_or(Ok(()))?;

        Ok(())
    }

    /// 轮询 socket 状态
    pub fn poll(&self) -> NetResult<PollState> {
        match self.get_state() {
            STATE_CONNECTING => self.poll_connect(),
            STATE_CONNECTED => self.poll_stream(),
            STATE_LISTENING => self.poll_listener(),
            _ => Ok(PollState {
                readable: false,
                writable: false,
            }),
        }
    }

    /// 设置 Nagle 算法
    pub fn set_nagle_enabled(&self, enabled: bool) -> NetResult<()> {
        let handle = unsafe { self.handle.get().read() };
        if let Some(handle) = handle {
            network_stack()
                .socket_set()
                .with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                    socket.set_nagle_enabled(enabled)
                });
            Ok(())
        } else {
            Err(NetError::NotConnected)
        }
    }

    /// 获取 Nagle 算法状态
    pub fn nagle_enabled(&self) -> bool {
        let handle = unsafe { self.handle.get().read() };
        match handle {
            Some(handle) => network_stack()
                .socket_set()
                .with_socket::<tcp::Socket, _, _>(handle, |socket| socket.nagle_enabled()),
            None => true, // 默认启用
        }
    }
}

// 私有方法实现
impl TcpSocket {
    fn get_state(&self) -> u8 {
        self.state.load(Ordering::Acquire)
    }

    fn set_state(&self, state: u8) {
        self.state.store(state, Ordering::Release);
    }

    fn update_state<F, T>(&self, expect: u8, new: u8, f: F) -> Result<NetResult<T>, u8>
    where
        F: FnOnce() -> NetResult<T>,
    {
        match self.state.compare_exchange(
            expect,
            STATE_BUSY,
            Ordering::Acquire,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                let res = f();
                if res.is_ok() {
                    self.set_state(new);
                } else {
                    self.set_state(expect);
                }
                Ok(res)
            }
            Err(old) => Err(old),
        }
    }

    fn is_connecting(&self) -> bool {
        self.get_state() == STATE_CONNECTING
    }

    fn is_connected(&self) -> bool {
        self.get_state() == STATE_CONNECTED
    }

    fn is_listening(&self) -> bool {
        self.get_state() == STATE_LISTENING
    }

    fn bound_endpoint(&self) -> NetResult<IpListenEndpoint> {
        let local_addr = unsafe { self.local_addr.get().read() };
        let port = if local_addr.port != 0 {
            local_addr.port
        } else {
            self.get_ephemeral_port()?
        };
        
        let addr = if !is_unspecified(local_addr.addr) {
            Some(local_addr.addr)
        } else {
            None
        };
        
        Ok(IpListenEndpoint { addr, port })
    }

    fn bound_endpoint_from_addr(&self, addr: IpEndpoint) -> NetResult<IpListenEndpoint> {
        let addr_opt = if !is_unspecified(addr.addr) {
            Some(addr.addr)
        } else {
            None
        };
        
        Ok(IpListenEndpoint {
            addr: addr_opt,
            port: addr.port,
        })
    }

    fn get_ephemeral_port(&self) -> NetResult<u16> {
        use crate::stack::PortManager;
        static PORT_MANAGER: PortManager = PortManager::new();
        
        for _ in 0..1000 { // 最多尝试 1000 次
            let port = PORT_MANAGER.next_ephemeral_port();
            if network_stack().listen_table().can_listen(port) {
                return Ok(port);
            }
        }
        Err(NetError::AddrInUse)
    }

    fn poll_connect(&self) -> NetResult<PollState> {
        let handle = unsafe { self.handle.get().read().unwrap() };
        let writable = network_stack()
            .socket_set()
            .with_socket::<tcp::Socket, _, _>(handle, |socket| {
                match socket.state() {
                    State::SynSent => false,
                    State::Established => {
                        self.set_state(STATE_CONNECTED);
                        debug!("TCP socket {}: 连接已建立到 {}", handle, socket.remote_endpoint().unwrap());
                        true
                    }
                    _ => {
                        unsafe {
                            self.local_addr.get().write(UNSPECIFIED_ENDPOINT);
                            self.peer_addr.get().write(UNSPECIFIED_ENDPOINT);
                        }
                        self.set_state(STATE_CLOSED);
                        true
                    }
                }
            });
        
        Ok(PollState {
            readable: false,
            writable,
        })
    }

    fn poll_stream(&self) -> NetResult<PollState> {
        let handle = unsafe { self.handle.get().read().unwrap() };
        network_stack()
            .socket_set()
            .with_socket::<tcp::Socket, _, _>(handle, |socket| {
                Ok(PollState {
                    readable: !socket.may_recv() || socket.can_recv(),
                    writable: !socket.may_send() || socket.can_send(),
                })
            })
    }

    fn poll_listener(&self) -> NetResult<PollState> {
        let local_addr = unsafe { self.local_addr.get().read() };
        Ok(PollState {
            readable: network_stack().listen_table().can_accept(local_addr.port)?,
            writable: false,
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
                    Err(NetError::WouldBlock) => yield_now(),
                    Err(e) => return Err(e),
                }
            }
        }
    }
}

impl Read for TcpSocket {
    fn read(&mut self, buf: &mut [u8]) -> axerrno::AxResult<usize> {
        self.recv(buf).map_err(|e| e.into())
    }
}

impl Write for TcpSocket {
    fn write(&mut self, buf: &[u8]) -> axerrno::AxResult<usize> {
        self.send(buf).map_err(|e| e.into())
    }

    fn flush(&mut self) -> axerrno::AxResult {
        Ok(()) // TCP 自动刷新
    }
}

impl Drop for TcpSocket {
    fn drop(&mut self) {
        let _ = self.shutdown();
        if let Some(handle) = unsafe { self.handle.get().read() } {
            network_stack().socket_set().remove(handle);
        }
    }
}