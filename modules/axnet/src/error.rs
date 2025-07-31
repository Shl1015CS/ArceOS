//! Network error type definitions

use axerrno::{AxError, LinuxError};

/// Network operation result type
pub type NetResult<T> = Result<T, NetError>;

/// Network error type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetError {
    /// Connection refused
    ConnectionRefused,
    /// Connection reset
    ConnectionReset,
    /// Connection timeout
    Timeout,
    /// Address already in use
    AddrInUse,
    /// Address not available
    AddrNotAvailable,
    /// Network unreachable
    NetworkUnreachable,
    /// Host unreachable
    HostUnreachable,
    /// Operation would block
    WouldBlock,
    /// Invalid input
    InvalidInput,
    /// Not connected
    NotConnected,
    /// Already connected
    AlreadyConnected,
    /// Buffer full
    BufferFull,
    /// Buffer empty
    BufferEmpty,
    /// Unsupported operation
    Unsupported,
    /// Internal error
    Internal,
}

impl From<NetError> for AxError {
    fn from(err: NetError) -> Self {
        match err {
            NetError::ConnectionRefused => AxError::ConnectionRefused,
            NetError::ConnectionReset => AxError::ConnectionReset,
            NetError::Timeout => AxError::WouldBlock, // Use WouldBlock instead of Timeout
            NetError::AddrInUse => AxError::AddrInUse,
            NetError::AddrNotAvailable => AxError::BadAddress,
            NetError::NetworkUnreachable => AxError::BadAddress,
            NetError::HostUnreachable => AxError::BadAddress,
            NetError::WouldBlock => AxError::WouldBlock,
            NetError::InvalidInput => AxError::InvalidInput,
            NetError::NotConnected => AxError::NotConnected,
            NetError::AlreadyConnected => AxError::AlreadyExists,
            NetError::BufferFull => AxError::StorageFull,
            NetError::BufferEmpty => AxError::UnexpectedEof,
            NetError::Unsupported => AxError::Unsupported,
            NetError::Internal => AxError::BadState,
        }
    }
}

impl From<AxError> for NetError {
    fn from(err: AxError) -> Self {
        match err {
            AxError::ConnectionRefused => NetError::ConnectionRefused,
            AxError::ConnectionReset => NetError::ConnectionReset,
            AxError::WouldBlock => NetError::WouldBlock,
            AxError::AddrInUse => NetError::AddrInUse,
            AxError::BadAddress => NetError::AddrNotAvailable,
            AxError::InvalidInput => NetError::InvalidInput,
            AxError::NotConnected => NetError::NotConnected,
            AxError::AlreadyExists => NetError::AlreadyConnected,
            AxError::StorageFull => NetError::BufferFull,
            AxError::UnexpectedEof => NetError::BufferEmpty,
            AxError::Unsupported => NetError::Unsupported,
            _ => NetError::Internal,
        }
    }
}

impl From<NetError> for LinuxError {
    fn from(err: NetError) -> Self {
        match err {
            NetError::ConnectionRefused => LinuxError::ECONNREFUSED,
            NetError::ConnectionReset => LinuxError::ECONNRESET,
            NetError::Timeout => LinuxError::ETIMEDOUT,
            NetError::AddrInUse => LinuxError::EADDRINUSE,
            NetError::AddrNotAvailable => LinuxError::EADDRNOTAVAIL,
            NetError::NetworkUnreachable => LinuxError::ENETUNREACH,
            NetError::HostUnreachable => LinuxError::EHOSTUNREACH,
            NetError::WouldBlock => LinuxError::EAGAIN,
            NetError::InvalidInput => LinuxError::EINVAL,
            NetError::NotConnected => LinuxError::ENOTCONN,
            NetError::AlreadyConnected => LinuxError::EISCONN,
            NetError::BufferFull => LinuxError::ENOBUFS,
            NetError::BufferEmpty => LinuxError::ENODATA,
            NetError::Unsupported => LinuxError::EOPNOTSUPP,
            NetError::Internal => LinuxError::EIO,
        }
    }
}

impl From<LinuxError> for NetError {
    fn from(err: LinuxError) -> Self {
        match err {
            LinuxError::ECONNREFUSED => NetError::ConnectionRefused,
            LinuxError::ECONNRESET => NetError::ConnectionReset,
            LinuxError::ETIMEDOUT => NetError::Timeout,
            LinuxError::EADDRINUSE => NetError::AddrInUse,
            LinuxError::EADDRNOTAVAIL => NetError::AddrNotAvailable,
            LinuxError::ENETUNREACH => NetError::NetworkUnreachable,
            LinuxError::EHOSTUNREACH => NetError::HostUnreachable,
            LinuxError::EAGAIN => NetError::WouldBlock,
            LinuxError::EINVAL => NetError::InvalidInput,
            LinuxError::ENOTCONN => NetError::NotConnected,
            LinuxError::EISCONN => NetError::AlreadyConnected,
            LinuxError::ENOBUFS => NetError::BufferFull,
            LinuxError::ENODATA => NetError::BufferEmpty,
            LinuxError::EOPNOTSUPP => NetError::Unsupported,
            _ => NetError::Internal,
        }
    }
}