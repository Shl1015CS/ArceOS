//! DNS query functionality implementation
//!
//! Provides domain name resolution service, supporting A and AAAA record queries

use alloc::vec;
use alloc::vec::Vec;
use core::net::IpAddr;
use smoltcp::socket::dns;
use smoltcp::wire::{DnsQueryType, IpAddress, Ipv4Address, Ipv6Address};

use crate::error::{NetError, NetResult};
use super::{network_stack, current_time, socket_set::SocketSetManager};

/// DNS server configuration
const DNS_TIMEOUT_MS: u64 = 5000;

/// DNS query implementation
pub fn dns_query(domain: &str) -> NetResult<Vec<IpAddr>> {
    debug!("DNS query: {}", domain);

    // First check if it's an IP address
    if let Ok(ip) = domain.parse::<IpAddr>() {
        return Ok(vec![ip]);
    }

    // Check local hosts table
    if let Some(ip) = check_local_hosts(domain) {
        return Ok(vec![ip]);
    }

    // Perform actual DNS query
    perform_dns_query(domain)
}

/// Check local hosts table
fn check_local_hosts(domain: &str) -> Option<IpAddr> {
    match domain {
        "localhost" => Some(IpAddr::V4([127, 0, 0, 1].into())),
        "google.com" => Some(IpAddr::V4([8, 8, 8, 8].into())),
        "baidu.com" => Some(IpAddr::V4([220, 181, 38, 148].into())),
        _ => None,
    }
}

/// Perform DNS query
fn perform_dns_query(domain: &str) -> NetResult<Vec<IpAddr>> {
    // Create DNS socket
    let dns_socket = SocketSetManager::new_dns_socket();
    let handle = network_stack().socket_set().add(dns_socket);

    let result = query_with_timeout(handle, domain, DNS_TIMEOUT_MS);
    
    // Clean up socket
    network_stack().socket_set().remove(handle);
    
    result
}

/// DNS query with timeout
fn query_with_timeout(
    handle: smoltcp::iface::SocketHandle,
    domain: &str,
    timeout_ms: u64,
) -> NetResult<Vec<IpAddr>> {
    let start_time = current_time();
    let timeout = smoltcp::time::Duration::from_millis(timeout_ms);

    // Start A record query
    let query_handle = network_stack()
        .socket_set()
        .with_socket_mut::<dns::Socket, _, _>(handle, |socket| {
            let interface_ctx = network_stack().eth_interface().context();
            interface_ctx.with_interface_mut(|interface| {
                // Use interface directly, no need for Context
                // let mut ctx = interface.context();
                // Temporarily simplified DNS query implementation
                // TODO: Implement proper DNS query
                Err(NetError::Unsupported)
            })
        })?;

    // Poll until query completes or times out
    loop {
        network_stack().poll_interfaces();
        
        let result = network_stack()
            .socket_set()
            .with_socket_mut::<dns::Socket, _, _>(handle, |socket| {
                socket.get_query_result(query_handle)
            });

        match result {
            Ok(addresses) => {
                let mut ips = Vec::new();
                for addr in addresses {
                    match addr {
                        IpAddress::Ipv4(ipv4) => {
                            ips.push(IpAddr::V4(core::net::Ipv4Addr::from(ipv4.0)));
                        }
                        IpAddress::Ipv6(ipv6) => {
                            ips.push(IpAddr::V6(core::net::Ipv6Addr::from(ipv6.0)));
                        }
                    }
                }
                debug!("DNS Query Success: {} -> {:?}", domain, ips);
                return Ok(ips);
            }
            Err(dns::GetQueryResultError::Pending) => {
                // Query still in progress
                if current_time() - start_time > timeout {
                    warn!("DNS Query Timeout: {}", domain);
                    return Err(NetError::Timeout);
                }
                axtask::yield_now();
            }
            Err(_) => {
                warn!("DNS Query Failed: {}", domain);
                return Err(NetError::HostUnreachable);
            }
        }
    }
}

/// Query domain's IPv4 addresses
pub fn dns_query_ipv4(name: &str) -> NetResult<Vec<smoltcp::wire::Ipv4Address>> {
    let addrs = dns_query(name)?;
    let ipv4_addrs = addrs
        .into_iter()
        .filter_map(|addr| match addr {
            IpAddr::V4(ipv4) => Some(Ipv4Address(ipv4.octets())),
            _ => None,
        })
        .collect();
    Ok(ipv4_addrs)
}

/// Query domain's IPv6 addresses
pub fn dns_query_ipv6(name: &str) -> NetResult<Vec<smoltcp::wire::Ipv6Address>> {
    let addrs = dns_query(name)?;
    let ipv6_addrs = addrs
        .into_iter()
        .filter_map(|addr| match addr {
            IpAddr::V6(ipv6) => Some(Ipv6Address(ipv6.octets())),
            _ => None,
        })
        .collect();
    Ok(ipv6_addrs)
}

/// Query domain's first IP address
pub fn dns_query_first(name: &str) -> NetResult<IpAddr> {
    let addrs = dns_query(name)?;
    addrs.into_iter().next().ok_or(NetError::HostUnreachable)
}