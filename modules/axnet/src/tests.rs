//! Network module tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{NetError, NetResult};
    use core::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[test]
    fn test_error_conversion() {
        let net_err = NetError::ConnectionRefused;
        let ax_err: axerrno::AxError = net_err.into();
        assert_eq!(ax_err, axerrno::AxError::ConnectionRefused);
    }

    #[test]
    fn test_addr_utils() {
        use crate::stack::addr_utils::*;
        
        let std_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let smol_endpoint = from_std_socket_addr(std_addr);
        let converted_back = to_std_socket_addr(smol_endpoint);
        
        assert_eq!(std_addr, converted_back);
    }

    #[test]
    fn test_port_manager() {
        use crate::stack::PortManager;
        
        let manager = PortManager::new();
        let port1 = manager.next_ephemeral_port();
        let port2 = manager.next_ephemeral_port();
        
        assert_ne!(port1, port2);
        assert!(port1 >= 0xc000);
        assert!(port2 >= 0xc000);
    }
}