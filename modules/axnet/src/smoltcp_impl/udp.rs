//! UDP Socket implementation

use core::sync::atomic::{AtomicBool, Ordering};
use axio::{PollState, Read, Write};
use spin::RwLock;
use smoltcp::iface::SocketHandle;
use smoltcp::socket::udp::{self, BindError, SendError};
use smoltcp::wire::{IpEndpoint, IpListenEndpoint};

use crate::error::{NetError, NetResult};
use crate::stack::{SocketAddr, addr_utils::*};
use super::{network_stack, socket_set::SocketSetManager};

/// UDP Socket implementation
/// 
/// Provides POSIX-style UDP socket API, supporting:
/// - Datagram transmission: `send_to`, `recv_from`
/// - Connected mode: `connect`, `send`, `recv`
/// - Address binding: `bind`
/// - Non-blocking mode and address reuse
pub struct UdpSocket {
    handle: SocketHandle,
    local_addr: RwLock<Option<IpEndpoint>>,
    peer_addr: RwLock<Option<IpEndpoint>>,
    nonblock: AtomicBool,
    reuse_addr: AtomicBool,
}

impl UdpSocket {
    /// Create new UDP socket
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

    /// Get local address and port
    pub fn local_addr(&self) -> NetResult<SocketAddr> {
        self.local_addr
            .read()
            .as_ref()
            .map(|&addr| to_std_socket_addr(addr))
            .ok_or(NetError::NotConnected)
    }

    /// Get remote address and port
    pub fn peer_addr(&self) -> NetResult<SocketAddr> {
        self.remote_endpoint().map(to_std_socket_addr)
    }

    /// Check if in non-blocking mode
    pub fn is_nonblocking(&self) -> bool {
        self.nonblock.load(Ordering::Acquire)
    }

    /// Set non-blocking mode
    pub fn set_nonblocking(&self, nonblocking: bool) {
        self.nonblock.store(nonblocking, Ordering::Release);
    }

    /// Check if address reuse is enabled
    pub fn is_reuse_addr(&self) -> bool {
        self.reuse_addr.load(Ordering::Acquire)
    }

    /// Set address reuse
    pub fn set_reuse_addr(&self, reuse_addr: bool) {
        self.reuse_addr.store(reuse_addr, Ordering::Release);
    }

    /// Set TTL (Time To Live)
    pub fn set_ttl(&self, ttl: u8) {
        network_stack()
            .socket_set()
            .with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
                socket.set_hop_limit(Some(ttl))
            });
    }

    /// Bind to local address
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

        // Check address conflict
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
        debug!("UDP socket {}: bound to {}", self.handle, listen_endpoint);
        Ok(())
    }

    /// Connect to remote address
    pub fn connect(&self, remote_addr: SocketAddr) -> NetResult<()> {
        let mut self_peer_addr = self.peer_addr.write();

        // If not bound, automatically bind to unspecified address
        if self.local_addr.read().is_none() {
            self.bind(to_std_socket_addr(UNSPECIFIED_ENDPOINT))?;
        }

        *self_peer_addr = Some(from_std_socket_addr(remote_addr));
        debug!("UDP socket {}: connected to {}", self.handle, remote_addr);
        Ok(())
    }

    /// Send data to specified address
    pub fn send_to(&self, buf: &[u8], remote_addr: SocketAddr) -> NetResult<usize> {
        if remote_addr.port() == 0 || remote_addr.ip().is_unspecified() {
            return Err(NetError::InvalidInput);
        }
        
        self.send_impl(buf, from_std_socket_addr(remote_addr))
    }

    /// Receive data from socket, return data length and sender address
    pub fn recv_from(&self, buf: &mut [u8]) -> NetResult<(usize, SocketAddr)> {
        self.recv_impl(|socket| {
            socket
                .recv_slice(buf)
                .map(|(len, meta)| (len, to_std_socket_addr(meta.endpoint)))
                .map_err(|_| NetError::Internal)
        })
    }

    /// Peek data (without removing)
    pub fn peek_from(&self, buf: &mut [u8]) -> NetResult<(usize, SocketAddr)> {
        self.recv_impl(|socket| {
            socket
                .peek_slice(buf)
                .map(|(len, meta)| (len, to_std_socket_addr(meta.endpoint)))
                .map_err(|_| NetError::Internal)
        })
    }

    /// Send data to connected remote address
    pub fn send(&self, buf: &[u8]) -> NetResult<usize> {
        let remote_endpoint = self.remote_endpoint()?;
        self.send_impl(buf, remote_endpoint)
    }

    /// Receive data from connected remote address
    pub fn recv(&self, buf: &mut [u8]) -> NetResult<usize> {
        let remote_endpoint = self.remote_endpoint()?;
        self.recv_impl(|socket| {
            let (len, meta) = socket
                .recv_slice(buf)
                .map_err(|_| NetError::Internal)?;
            
            // Check if from connected remote address
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

    /// Close socket
    pub fn shutdown(&self) -> NetResult<()> {
        network_stack().poll_interfaces();
        network_stack()
            .socket_set()
            .with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
                debug!("UDP socket {}: closing", self.handle);
                socket.close();
            });
        Ok(())
    }

    /// Poll socket status
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

    /// Execute operation with socket (read-only)
    pub fn with_socket<R>(&self, f: impl FnOnce(&udp::Socket) -> R) -> R {
        network_stack().socket_set().with_socket(self.handle, f)
    }

    /// Execute operation with socket (mutable)
    pub fn with_socket_mut<R>(&self, f: impl FnOnce(&mut udp::Socket) -> R) -> R {
        network_stack().socket_set().with_socket_mut(self.handle, f)
    }
}

// Private method implementations
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
        
        // UDP port allocation is simpler, no need to check listen table
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
        Ok(()) // UDP doesn't need flush
    }
}

impl Drop for UdpSocket {
    fn drop(&mut self) {
        let _ = self.shutdown();
        network_stack().socket_set().remove(self.handle);
    }
}