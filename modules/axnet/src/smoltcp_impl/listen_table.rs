//! TCP listen table management
//!
//! Manages TCP server listening ports and connection queues

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;
use axsync::Mutex;
use smoltcp::iface::SocketHandle;
use smoltcp::socket::tcp;
use smoltcp::wire::{IpEndpoint, IpListenEndpoint};

use crate::error::{NetError, NetResult};
use super::network_stack;

/// TCP listen table
pub struct ListenTable {
    listeners: Mutex<BTreeMap<u16, ListenerInfo>>,
}

/// Listener information
struct ListenerInfo {
    endpoint: IpListenEndpoint,
    accept_queue: VecDeque<(SocketHandle, (IpEndpoint, IpEndpoint))>,
    backlog: usize,
}

impl ListenTable {
    /// Create new listen table
    pub fn new() -> Self {
        Self {
            listeners: Mutex::new(BTreeMap::new()),
        }
    }

    /// Start listening on specified endpoint
    pub fn listen(&self, endpoint: IpListenEndpoint) -> NetResult<()> {
        let mut listeners = self.listeners.lock();
        
        if listeners.contains_key(&endpoint.port) {
            return Err(NetError::AddrInUse);
        }

        listeners.insert(
            endpoint.port,
            ListenerInfo {
                endpoint,
                accept_queue: VecDeque::new(),
                backlog: 128, // Default backlog
            },
        );

        debug!("Started listening on port {}", endpoint.port);
        Ok(())
    }

    /// Stop listening on specified port
    pub fn unlisten(&self, port: u16) {
        let mut listeners = self.listeners.lock();
        if let Some(info) = listeners.remove(&port) {
            // Clean up sockets in accept queue
            for (handle, _) in info.accept_queue {
                network_stack().socket_set().remove(handle);
            }
            debug!("Stopped listening on port {}", port);
        }
    }

    /// Check if can listen on specified port
    pub fn can_listen(&self, port: u16) -> bool {
        !self.listeners.lock().contains_key(&port)
    }

    /// Check if there are pending connections to accept
    pub fn can_accept(&self, port: u16) -> NetResult<bool> {
        let listeners = self.listeners.lock();
        if let Some(info) = listeners.get(&port) {
            Ok(!info.accept_queue.is_empty())
        } else {
            Err(NetError::NotConnected)
        }
    }

    /// Accept new connection
    pub fn accept(&self, port: u16) -> NetResult<(SocketHandle, (IpEndpoint, IpEndpoint))> {
        let mut listeners = self.listeners.lock();
        if let Some(info) = listeners.get_mut(&port) {
            if let Some(connection) = info.accept_queue.pop_front() {
                debug!("Accepted connection: {:?}", connection.1);
                Ok(connection)
            } else {
                Err(NetError::WouldBlock)
            }
        } else {
            Err(NetError::NotConnected)
        }
    }

    /// Handle new connection request
    pub fn handle_new_connection(
        &self,
        local_port: u16,
        handle: SocketHandle,
        local_addr: IpEndpoint,
        peer_addr: IpEndpoint,
    ) -> NetResult<()> {
        let mut listeners = self.listeners.lock();
        if let Some(info) = listeners.get_mut(&local_port) {
            if info.accept_queue.len() < info.backlog {
                info.accept_queue.push_back((handle, (local_addr, peer_addr)));
                debug!("New connection added to queue: {} -> {}", peer_addr, local_addr);
                Ok(())
            } else {
                // Queue is full, reject connection
                network_stack().socket_set().remove(handle);
                Err(NetError::ConnectionRefused)
            }
        } else {
            // No listener, reject connection
            network_stack().socket_set().remove(handle);
            Err(NetError::ConnectionRefused)
        }
    }

    /// Set listener backlog
    pub fn set_backlog(&self, port: u16, backlog: usize) -> NetResult<()> {
        let mut listeners = self.listeners.lock();
        if let Some(info) = listeners.get_mut(&port) {
            info.backlog = backlog;
            Ok(())
        } else {
            Err(NetError::NotConnected)
        }
    }

    /// Get active connection count
    pub fn active_connections(&self) -> usize {
        self.listeners
            .lock()
            .values()
            .map(|info| info.accept_queue.len())
            .sum()
    }

    /// Get listening port list
    pub fn listening_ports(&self) -> Vec<u16> {
        self.listeners.lock().keys().copied().collect()
    }

    /// Clean up expired connections
    pub fn cleanup_expired_connections(&self) {
        let mut listeners = self.listeners.lock();
        for info in listeners.values_mut() {
            let mut to_remove = Vec::new();
            
            for (i, &(handle, _)) in info.accept_queue.iter().enumerate() {
                // Check if socket is still valid
                let is_valid = network_stack()
                    .socket_set()
                    .with_socket::<tcp::Socket, _, _>(handle, |socket| socket.is_active());
                
                if !is_valid {
                    to_remove.push(i);
                }
            }
            
            // Remove from back to front to avoid index changes
            for &i in to_remove.iter().rev() {
                if let Some((handle, _)) = info.accept_queue.remove(i) {
                    network_stack().socket_set().remove(handle);
                }
            }
        }
    }

    /// Get listener information for specified port
    pub fn get_listener_info(&self, port: u16) -> Option<ListenerStats> {
        let listeners = self.listeners.lock();
        listeners.get(&port).map(|info| ListenerStats {
            port,
            endpoint: info.endpoint,
            queue_len: info.accept_queue.len(),
            backlog: info.backlog,
        })
    }

    /// Get statistics for all listeners
    pub fn get_all_stats(&self) -> Vec<ListenerStats> {
        let listeners = self.listeners.lock();
        listeners
            .iter()
            .map(|(&port, info)| ListenerStats {
                port,
                endpoint: info.endpoint,
                queue_len: info.accept_queue.len(),
                backlog: info.backlog,
            })
            .collect()
    }
}

/// Listener statistics
#[derive(Debug, Clone)]
pub struct ListenerStats {
    pub port: u16,
    pub endpoint: IpListenEndpoint,
    pub queue_len: usize,
    pub backlog: usize,
}

impl Default for ListenTable {
    fn default() -> Self {
        Self::new()
    }
}