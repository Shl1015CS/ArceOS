//! Complete network stack implementation based on smoltcp
//!
//! Provides complete TCP/UDP network functionality, including:
//! - Network interface management (Ethernet, loopback)
//! - Socket set management
//! - TCP connection management and listen table
//! - DNS resolution service
//! - Network statistics and monitoring

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

// Network configuration constants
const DNS_SERVER: &str = "8.8.8.8";
const BACKUP_DNS_SERVER: &str = "8.8.4.4";

// Environment variable configuration
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

// Global network stack instance
static NETWORK_STACK: LazyInit<Arc<NetworkStack>> = LazyInit::new();

/// Network stack manager
/// 
/// Manages core components of the entire network stack, including:
/// - Network interfaces (Ethernet, loopback)
/// - Socket set management
/// - TCP listen table
/// - Network statistics
pub struct NetworkStack {
    eth_interface: interface::EthernetInterface,
    loopback_interface: interface::LoopbackInterface,
    socket_set: socket_set::SocketSetManager,
    listen_table: listen_table::ListenTable,
    stats: Mutex<NetworkStats>,
}

impl NetworkStack {
    /// Create new network stack instance
    fn new(net_dev: AxNetDevice) -> NetResult<Self> {
        info!("Creating network stack...");

        // Create Ethernet interface
        let ether_addr = EthernetAddress(net_dev.mac_address().0);
        let eth_interface = interface::EthernetInterface::new("eth0", net_dev, ether_addr)?;

        // Configure IP address and gateway
        let ip: IpAddress = IP.parse().map_err(|_| NetError::InvalidInput)?;
        let gateway: IpAddress = GATEWAY.parse().map_err(|_| NetError::InvalidInput)?;
        
        eth_interface.setup_ip_addr(ip, IP_PREFIX)?;
        eth_interface.setup_gateway(gateway)?;

        info!("Network interface configuration:");
        info!("  Interface name: {}", eth_interface.name());
        info!("  MAC address: {}", eth_interface.ethernet_address());
        info!("  IP address: {}/{}", ip, IP_PREFIX);
        info!("  Gateway: {}", gateway);

        // Create loopback interface
        let loopback_interface = interface::LoopbackInterface::new()?;
        info!("Loopback interface created");

        // Create Socket set manager
        let socket_set = socket_set::SocketSetManager::new();
        info!("Socket manager initialized");

        // Create TCP listen table
        let listen_table = listen_table::ListenTable::new();
        info!("TCP listen table initialized");

        // Initialize statistics
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

    /// Poll all network interfaces
    /// 
    /// Handle network packet transmission and reception, update Socket status
    pub fn poll_interfaces(&self) {
        let timestamp = current_time();
        
        // Poll Ethernet interface
        self.eth_interface.poll(timestamp, &self.socket_set);
        
        // Poll loopback interface
        self.loopback_interface.poll(timestamp, &self.socket_set);
        
        // Update statistics
        self.update_stats();
    }

    /// Update network statistics
    fn update_stats(&self) {
        let mut stats = self.stats.lock();
        stats.tcp_connections = self.listen_table.active_connections();
        stats.udp_sockets = self.socket_set.udp_socket_count();
        // Other statistics are updated by interface layer
    }

    /// Get network statistics
    pub fn get_stats(&self) -> NetworkStats {
        self.stats.lock().clone()
    }

    /// Get Socket set manager
    pub fn socket_set(&self) -> &socket_set::SocketSetManager {
        &self.socket_set
    }

    /// Get TCP listen table
    pub fn listen_table(&self) -> &listen_table::ListenTable {
        &self.listen_table
    }

    /// Get Ethernet interface
    pub fn eth_interface(&self) -> &interface::EthernetInterface {
        &self.eth_interface
    }

    /// Get loopback interface
    pub fn loopback_interface(&self) -> &interface::LoopbackInterface {
        &self.loopback_interface
    }

    /// Check network connection status
    pub fn is_link_up(&self) -> bool {
        self.eth_interface.is_link_up()
    }

    /// Get local IP address
    pub fn local_ip(&self) -> Option<IpAddress> {
        self.eth_interface.ip_addr()
    }

    /// Get gateway address
    pub fn gateway(&self) -> Option<IpAddress> {
        self.eth_interface.gateway()
    }
}

/// Get current timestamp
pub fn current_time() -> Instant {
    Instant::from_micros_const((axhal::time::current_ticks() / NANOS_PER_MICROS) as i64)
}

/// Initialize network stack
pub fn init_network_stack(net_dev: AxNetDevice) {
    let stack = NetworkStack::new(net_dev).expect("Network stack initialization failed");
    NETWORK_STACK.init_by(Arc::new(stack));
    
    // Start network polling task
    spawn_network_poll_task();
}

/// Get global network stack instance
pub fn network_stack() -> &'static Arc<NetworkStack> {
    NETWORK_STACK.try_get().expect("Network stack not initialized")
}

/// Poll network interfaces (public interface)
pub fn poll_interfaces() {
    if let Some(stack) = NETWORK_STACK.try_get() {
        stack.poll_interfaces();
    }
}

/// Get network statistics
pub fn get_network_stats() -> NetResult<NetworkStats> {
    if let Some(stack) = NETWORK_STACK.try_get() {
        Ok(stack.get_stats())
    } else {
        Err(NetError::Internal)
    }
}

/// Start network polling task
fn spawn_network_poll_task() {
    // Simplified implementation, not starting polling task for now
    // TODO: Implement network polling task
    // axtask::spawn(|| {
    //     info!("Network polling task started");
    //     loop {
    //         poll_interfaces();
    //         axtask::yield_now();
    //     }
    // });
}

// Compatibility functions and utilities
// pub use crate::stack::addr_utils::{
//     from_std_socket_addr as from_core_sockaddr, 
//     to_std_socket_addr as into_core_sockaddr
// };

/// Add multicast group membership (temporarily using loopback interface)
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

/// Network performance benchmark: transmit bandwidth
pub fn bench_transmit() -> NetResult<u64> {
    info!("Starting transmit bandwidth benchmark");
    // TODO: Implement specific transmit bandwidth test
    Ok(0)
}

/// Network performance benchmark: receive bandwidth  
pub fn bench_receive() -> NetResult<u64> {
    info!("Starting receive bandwidth benchmark");
    // TODO: Implement specific receive bandwidth test
    Ok(0)
}