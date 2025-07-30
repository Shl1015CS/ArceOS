//! 基于smoltcp的完整网络栈实现
//!
//! 提供完整的TCP/UDP网络功能，包括：
//! - 网络接口管理（以太网、回环）
//! - Socket集合管理
//! - TCP连接管理和监听表
//! - DNS解析服务
//! - 网络统计和监控

mod device;
mod dns;
mod interface;
mod listen_table;
mod socket_set;
mod tcp;
mod udp;

use alloc::sync::Arc;
use axdriver::prelude::*;
use axhal::time::NANOS_PER_MICROS;
use axsync::Mutex;
use lazy_init::LazyInit;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, IpAddress};

use crate::error::{NetError, NetResult};
use crate::NetworkStats;

pub use self::dns::dns_query;
pub use self::tcp::TcpSocket;
pub use self::udp::UdpSocket;

// 网络配置常量
const DNS_SERVER: &str = "8.8.8.8";
const BACKUP_DNS_SERVER: &str = "8.8.4.4";

// 环境变量配置
macro_rules! env_or_default {
    ($key:literal, $default:literal) => {
        match option_env!($key) {
            Some(val) => val,
            None => $default,
        }
    };
}

const IP: &str = env_or_default!("AX_IP", "10.0.2.15");
const GATEWAY: &str = env_or_default!("AX_GW", "10.0.2.2");
const IP_PREFIX: u8 = 24;

// 全局网络栈实例
static NETWORK_STACK: LazyInit<Arc<NetworkStack>> = LazyInit::new();

/// 网络栈管理器
/// 
/// 管理整个网络栈的核心组件，包括：
/// - 网络接口（以太网、回环）
/// - Socket集合管理
/// - TCP监听表
/// - 网络统计信息
pub struct NetworkStack {
    eth_interface: interface::EthernetInterface,
    loopback_interface: interface::LoopbackInterface,
    socket_set: socket_set::SocketSetManager,
    listen_table: listen_table::ListenTable,
    stats: Mutex<NetworkStats>,
}

impl NetworkStack {
    /// 创建新的网络栈实例
    fn new(net_dev: AxNetDevice) -> NetResult<Self> {
        info!("创建网络栈...");

        // 创建以太网接口
        let ether_addr = EthernetAddress(net_dev.mac_address().0);
        let eth_interface = interface::EthernetInterface::new("eth0", net_dev, ether_addr)?;

        // 配置IP地址和网关
        let ip: IpAddress = IP.parse().map_err(|_| NetError::InvalidInput)?;
        let gateway: IpAddress = GATEWAY.parse().map_err(|_| NetError::InvalidInput)?;
        
        eth_interface.setup_ip_addr(ip, IP_PREFIX)?;
        eth_interface.setup_gateway(gateway)?;

        info!("网络接口配置:");
        info!("  接口名称: {}", eth_interface.name());
        info!("  MAC地址: {}", eth_interface.ethernet_address());
        info!("  IP地址: {}/{}", ip, IP_PREFIX);
        info!("  网关: {}", gateway);

        // 创建回环接口
        let loopback_interface = interface::LoopbackInterface::new()?;
        info!("回环接口已创建");

        // 创建Socket集合管理器
        let socket_set = socket_set::SocketSetManager::new();
        info!("Socket管理器已初始化");

        // 创建TCP监听表
        let listen_table = listen_table::ListenTable::new();
        info!("TCP监听表已初始化");

        // 初始化统计信息
        let stats = Mutex::new(NetworkStats {
            tx_packets: 0,
            rx_packets: 0,
            tx_bytes: 0,
            rx_bytes: 0,
            tcp_connections: 0,
            udp_sockets: 0,
        });

        Ok(Self {
            eth_interface,
            loopback_interface,
            socket_set,
            listen_table,
            stats,
        })
    }

    /// 轮询所有网络接口
    /// 
    /// 处理网络数据包的收发，更新Socket状态
    pub fn poll_interfaces(&self) {
        let timestamp = current_time();
        
        // 轮询以太网接口
        self.eth_interface.poll(timestamp, &self.socket_set);
        
        // 轮询回环接口
        self.loopback_interface.poll(timestamp, &self.socket_set);
        
        // 更新统计信息
        self.update_stats();
    }

    /// 更新网络统计信息
    fn update_stats(&self) {
        let mut stats = self.stats.lock();
        stats.tcp_connections = self.listen_table.active_connections();
        stats.udp_sockets = self.socket_set.udp_socket_count();
        // 其他统计信息由接口层更新
    }

    /// 获取网络统计信息
    pub fn get_stats(&self) -> NetworkStats {
        self.stats.lock().clone()
    }

    /// 获取Socket集合管理器
    pub fn socket_set(&self) -> &socket_set::SocketSetManager {
        &self.socket_set
    }

    /// 获取TCP监听表
    pub fn listen_table(&self) -> &listen_table::ListenTable {
        &self.listen_table
    }

    /// 获取以太网接口
    pub fn eth_interface(&self) -> &interface::EthernetInterface {
        &self.eth_interface
    }

    /// 获取回环接口
    pub fn loopback_interface(&self) -> &interface::LoopbackInterface {
        &self.loopback_interface
    }

    /// 检查网络连接状态
    pub fn is_link_up(&self) -> bool {
        self.eth_interface.is_link_up()
    }

    /// 获取本地IP地址
    pub fn local_ip(&self) -> Option<IpAddress> {
        self.eth_interface.ip_addr()
    }

    /// 获取网关地址
    pub fn gateway(&self) -> Option<IpAddress> {
        self.eth_interface.gateway()
    }
}

/// 获取当前时间戳
pub fn current_time() -> Instant {
    Instant::from_micros_const((axhal::time::current_time() / NANOS_PER_MICROS) as i64)
}

/// 初始化网络栈
pub fn init_network_stack(net_dev: AxNetDevice) {
    let stack = NetworkStack::new(net_dev).expect("网络栈初始化失败");
    NETWORK_STACK.init_by(Arc::new(stack));
    
    // 启动网络轮询任务
    spawn_network_poll_task();
}

/// 获取全局网络栈实例
pub fn network_stack() -> &'static Arc<NetworkStack> {
    NETWORK_STACK.get()
}

/// 轮询网络接口（公共接口）
pub fn poll_interfaces() {
    if let Some(stack) = NETWORK_STACK.try_get() {
        stack.poll_interfaces();
    }
}

/// 获取网络统计信息
pub fn get_network_stats() -> NetResult<NetworkStats> {
    if let Some(stack) = NETWORK_STACK.try_get() {
        Ok(stack.get_stats())
    } else {
        Err(NetError::Internal)
    }
}

/// 启动网络轮询任务
fn spawn_network_poll_task() {
    axtask::spawn(|| {
        info!("网络轮询任务已启动");
        loop {
            poll_interfaces();
            axtask::yield_now();
        }
    });
}

// 兼容性函数和工具
pub use crate::stack::addr_utils::{
    from_std_socket_addr as from_core_sockaddr, 
    to_std_socket_addr as into_core_sockaddr
};

/// 添加多播组成员（暂时使用回环接口）
pub fn add_membership(
    multicast_addr: smoltcp::wire::IpAddress, 
    _interface_addr: smoltcp::wire::IpAddress
) -> NetResult<()> {
    if let Some(stack) = NETWORK_STACK.try_get() {
        stack.loopback_interface().join_multicast_group(multicast_addr, current_time())
    } else {
        Err(NetError::Internal)
    }
}

/// 网络性能基准测试：发送带宽
pub fn bench_transmit() -> NetResult<u64> {
    info!("开始发送带宽基准测试");
    // TODO: 实现具体的发送带宽测试
    Ok(0)
}

/// 网络性能基准测试：接收带宽  
pub fn bench_receive() -> NetResult<u64> {
    info!("开始接收带宽基准测试");
    // TODO: 实现具体的接收带宽测试
    Ok(0)
}