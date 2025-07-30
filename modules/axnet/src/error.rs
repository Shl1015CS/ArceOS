//! 网络错误类型定义

use axerrno::AxError;

/// 网络操作结果类型
pub type NetResult<T> = Result<T, NetError>;

/// 网络错误类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetError {
    /// 连接被拒绝
    ConnectionRefused,
    /// 连接被重置
    ConnectionReset,
    /// 连接超时
    Timeout,
    /// 地址已被使用
    AddrInUse,
    /// 地址不可达
    AddrNotAvailable,
    /// 网络不可达
    NetworkUnreachable,
    /// 主机不可达
    HostUnreachable,
    /// 操作会阻塞
    WouldBlock,
    /// 无效参数
    InvalidInput,
    /// 未连接
    NotConnected,
    /// 已连接
    AlreadyConnected,
    /// 缓冲区已满
    BufferFull,
    /// 缓冲区为空
    BufferEmpty,
    /// 不支持的操作
    Unsupported,
    /// 内部错误
    Internal,
}

impl From<NetError> for AxError {
    fn from(err: NetError) -> Self {
        match err {
            NetError::ConnectionRefused => AxError::ConnectionRefused,
            NetError::ConnectionReset => AxError::ConnectionReset,
            NetError::Timeout => AxError::Timeout,
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
            AxError::Timeout => NetError::Timeout,
            AxError::AddrInUse => NetError::AddrInUse,
            AxError::BadAddress => NetError::AddrNotAvailable,
            AxError::WouldBlock => NetError::WouldBlock,
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