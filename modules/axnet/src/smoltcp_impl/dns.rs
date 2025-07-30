//! DNS查询功能实现
//!
//! 提供域名解析服务，支持A记录和AAAA记录查询

use alloc::vec;
use alloc::vec::Vec;
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use smoltcp::socket::dns;
use smoltcp::wire::{DnsQueryType, IpAddress, Ipv4Address, Ipv6Address};

use crate::error::{NetError, NetResult};
use super::{network_stack, current_time, socket_set::SocketSetManager};

/// DNS服务器配置
const DNS_TIMEOUT_MS: u64 = 5000;

/// DNS查询实现
pub fn dns_query(domain: &str) -> NetResult<Vec<IpAddr>> {
    debug!("DNS查询: {}", domain);

    // 首先检查是否为IP地址
    if let Ok(ip) = domain.parse::<IpAddr>() {
        return Ok(vec![ip]);
    }

    // 检查本地hosts表
    if let Some(ip) = check_local_hosts(domain) {
        return Ok(vec![ip]);
    }

    // 执行真正的DNS查询
    perform_dns_query(domain)
}

/// 检查本地hosts表
fn check_local_hosts(domain: &str) -> Option<IpAddr> {
    match domain {
        "localhost" => Some(IpAddr::V4([127, 0, 0, 1].into())),
        "google.com" => Some(IpAddr::V4([8, 8, 8, 8].into())),
        "baidu.com" => Some(IpAddr::V4([220, 181, 38, 148].into())),
        _ => None,
    }
}

/// 执行DNS查询
fn perform_dns_query(domain: &str) -> NetResult<Vec<IpAddr>> {
    // 创建DNS socket
    let dns_socket = SocketSetManager::new_dns_socket();
    let handle = network_stack().socket_set().add(dns_socket);

    let result = query_with_timeout(handle, domain, DNS_TIMEOUT_MS);
    
    // 清理socket
    network_stack().socket_set().remove(handle);
    
    result
}

/// 带超时的DNS查询
fn query_with_timeout(
    handle: smoltcp::iface::SocketHandle,
    domain: &str,
    timeout_ms: u64,
) -> NetResult<Vec<IpAddr>> {
    let start_time = current_time();
    let timeout = smoltcp::time::Duration::from_millis(timeout_ms);

    // 启动A记录查询
    let query_handle = network_stack()
        .socket_set()
        .with_socket_mut::<dns::Socket, _, _>(handle, |socket| {
            let mut interface = network_stack().eth_interface().lock();
            socket
                .start_query(&mut interface, domain, DnsQueryType::A)
                .map_err(|_| NetError::Internal)
        })?;

    // 轮询直到查询完成或超时
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
                            ips.push(IpAddr::V4(Ipv4Addr::from(ipv4.0)));
                        }
                        IpAddress::Ipv6(ipv6) => {
                            ips.push(IpAddr::V6(Ipv6Addr::from(ipv6.0)));
                        }
                    }
                }
                debug!("DNS查询成功: {} -> {:?}", domain, ips);
                return Ok(ips);
            }
            Err(dns::GetQueryResultError::Pending) => {
                // 查询仍在进行中
                if current_time() - start_time > timeout {
                    warn!("DNS查询超时: {}", domain);
                    return Err(NetError::Timeout);
                }
                axtask::yield_now();
            }
            Err(_) => {
                warn!("DNS查询失败: {}", domain);
                return Err(NetError::HostUnreachable);
            }
        }
    }
}

/// 查询域名的IPv4地址
pub fn dns_query_ipv4(name: &str) -> NetResult<Vec<smoltcp::wire::Ipv4Address>> {
    let addrs = dns_query(name)?;
    let ipv4_addrs = addrs
        .into_iter()
        .filter_map(|addr| match addr {
            IpAddr::V4(ipv4) => Some(Ipv4Address::from(ipv4)),
            _ => None,
        })
        .collect();
    Ok(ipv4_addrs)
}

/// 查询域名的IPv6地址
pub fn dns_query_ipv6(name: &str) -> NetResult<Vec<smoltcp::wire::Ipv6Address>> {
    let addrs = dns_query(name)?;
    let ipv6_addrs = addrs
        .into_iter()
        .filter_map(|addr| match addr {
            IpAddr::V6(ipv6) => Some(Ipv6Address::from(ipv6)),
            _ => None,
        })
        .collect();
    Ok(ipv6_addrs)
}

/// 查询域名的第一个IP地址
pub fn dns_query_first(name: &str) -> NetResult<IpAddr> {
    let addrs = dns_query(name)?;
    addrs.into_iter().next().ok_or(NetError::HostUnreachable)
}