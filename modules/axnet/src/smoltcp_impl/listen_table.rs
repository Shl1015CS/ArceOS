//! TCP监听表管理
//!
//! 管理TCP服务端的监听端口和连接队列

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;
use axsync::Mutex;
use smoltcp::iface::SocketHandle;
use smoltcp::socket::tcp;
use smoltcp::wire::{IpEndpoint, IpListenEndpoint};

use crate::error::{NetError, NetResult};
use super::network_stack;

/// TCP监听表
pub struct ListenTable {
    listeners: Mutex<BTreeMap<u16, ListenerInfo>>,
}

/// 监听器信息
struct ListenerInfo {
    endpoint: IpListenEndpoint,
    accept_queue: VecDeque<(SocketHandle, (IpEndpoint, IpEndpoint))>,
    backlog: usize,
}

impl ListenTable {
    /// 创建新的监听表
    pub fn new() -> Self {
        Self {
            listeners: Mutex::new(BTreeMap::new()),
        }
    }

    /// 开始监听指定端点
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
                backlog: 128, // 默认backlog
            },
        );

        debug!("开始监听端口 {}", endpoint.port);
        Ok(())
    }

    /// 停止监听指定端口
    pub fn unlisten(&self, port: u16) {
        let mut listeners = self.listeners.lock();
        if let Some(info) = listeners.remove(&port) {
            // 清理accept队列中的socket
            for (handle, _) in info.accept_queue {
                network_stack().socket_set().remove(handle);
            }
            debug!("停止监听端口 {}", port);
        }
    }

    /// 检查是否可以监听指定端口
    pub fn can_listen(&self, port: u16) -> bool {
        !self.listeners.lock().contains_key(&port)
    }

    /// 检查是否有待接受的连接
    pub fn can_accept(&self, port: u16) -> NetResult<bool> {
        let listeners = self.listeners.lock();
        if let Some(info) = listeners.get(&port) {
            Ok(!info.accept_queue.is_empty())
        } else {
            Err(NetError::NotConnected)
        }
    }

    /// 接受新连接
    pub fn accept(&self, port: u16) -> NetResult<(SocketHandle, (IpEndpoint, IpEndpoint))> {
        let mut listeners = self.listeners.lock();
        if let Some(info) = listeners.get_mut(&port) {
            if let Some(connection) = info.accept_queue.pop_front() {
                debug!("接受连接: {:?}", connection.1);
                Ok(connection)
            } else {
                Err(NetError::WouldBlock)
            }
        } else {
            Err(NetError::NotConnected)
        }
    }

    /// 处理新的连接请求
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
                debug!("新连接加入队列: {} -> {}", peer_addr, local_addr);
                Ok(())
            } else {
                // 队列已满，拒绝连接
                network_stack().socket_set().remove(handle);
                Err(NetError::ConnectionRefused)
            }
        } else {
            // 没有监听器，拒绝连接
            network_stack().socket_set().remove(handle);
            Err(NetError::ConnectionRefused)
        }
    }

    /// 设置监听器的backlog
    pub fn set_backlog(&self, port: u16, backlog: usize) -> NetResult<()> {
        let mut listeners = self.listeners.lock();
        if let Some(info) = listeners.get_mut(&port) {
            info.backlog = backlog;
            Ok(())
        } else {
            Err(NetError::NotConnected)
        }
    }

    /// 获取活跃连接数
    pub fn active_connections(&self) -> usize {
        self.listeners
            .lock()
            .values()
            .map(|info| info.accept_queue.len())
            .sum()
    }

    /// 获取监听端口列表
    pub fn listening_ports(&self) -> Vec<u16> {
        self.listeners.lock().keys().copied().collect()
    }

    /// 清理过期的连接
    pub fn cleanup_expired_connections(&self) {
        let mut listeners = self.listeners.lock();
        for info in listeners.values_mut() {
            let mut to_remove = Vec::new();
            
            for (i, &(handle, _)) in info.accept_queue.iter().enumerate() {
                // 检查socket是否仍然有效
                let is_valid = network_stack()
                    .socket_set()
                    .with_socket::<tcp::Socket, _, _>(handle, |socket| socket.is_active());
                
                if !is_valid {
                    to_remove.push(i);
                }
            }
            
            // 从后往前移除，避免索引变化
            for &i in to_remove.iter().rev() {
                if let Some((handle, _)) = info.accept_queue.remove(i) {
                    network_stack().socket_set().remove(handle);
                }
            }
        }
    }

    /// 获取指定端口的监听信息
    pub fn get_listener_info(&self, port: u16) -> Option<ListenerStats> {
        let listeners = self.listeners.lock();
        listeners.get(&port).map(|info| ListenerStats {
            port,
            endpoint: info.endpoint,
            queue_len: info.accept_queue.len(),
            backlog: info.backlog,
        })
    }

    /// 获取所有监听器的统计信息
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

/// 监听器统计信息
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