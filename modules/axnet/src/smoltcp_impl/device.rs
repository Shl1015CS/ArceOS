//! Network device adapters
//!
//! Provides device abstraction layer for smoltcp, including Ethernet and loopback devices

use alloc::vec;
use alloc::vec::Vec;
use axdriver::prelude::*;
use axsync::Mutex;
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;

/// Ethernet device adapter
pub struct EthernetDevice {
    inner: AxNetDevice,
    rx_buffer: Mutex<Vec<u8>>,
    tx_buffer: Mutex<Vec<u8>>,
}

impl EthernetDevice {
    /// Create new Ethernet device
    pub fn new(device: AxNetDevice) -> Self {
        Self {
            inner: device,
            rx_buffer: Mutex::new(Vec::with_capacity(1536)),
            tx_buffer: Mutex::new(Vec::with_capacity(1536)),
        }
    }

    /// Check link status
    pub fn is_link_up(&self) -> bool {
        // Simplified implementation, assume link is always up
        true
    }
}

impl Device for EthernetDevice {
    type RxToken<'a> = EthernetRxToken where Self: 'a;
    type TxToken<'a> = EthernetTxToken<'a> where Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let mut rx_buf = self.rx_buffer.lock();
        rx_buf.clear();
        rx_buf.resize(1536, 0);

        match self.inner.receive() {
            Ok(net_buf) => {
                let len = net_buf.packet_len();
                if len > 0 {
                    rx_buf.clear();
                    rx_buf.extend_from_slice(net_buf.packet());
                    let rx_token = EthernetRxToken {
                        buffer: rx_buf.clone(),
                    };
                    let tx_token = EthernetTxToken {
                        device: &mut self.inner,
                        buffer: &self.tx_buffer,
                    };
                    Some((rx_token, tx_token))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(EthernetTxToken {
            device: &mut self.inner,
            buffer: &self.tx_buffer,
        })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1500;
        caps.max_burst_size = Some(1);
        caps.medium = Medium::Ethernet;
        caps
    }
}

/// Ethernet receive token
pub struct EthernetRxToken {
    buffer: Vec<u8>,
}

impl RxToken for EthernetRxToken {
    fn consume<R, F>(mut self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        f(&mut self.buffer)
    }
}

/// Ethernet transmit token
pub struct EthernetTxToken<'a> {
    device: &'a mut AxNetDevice,
    buffer: &'a Mutex<Vec<u8>>,
}

impl<'a> TxToken for EthernetTxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut tx_buf = self.buffer.lock();
        tx_buf.clear();
        tx_buf.resize(len, 0);
        
        let result = f(&mut tx_buf);
        
        let net_buf = axdriver_net::NetBufPtr::new(
            core::ptr::NonNull::new(tx_buf.as_ptr() as *mut u8).unwrap(),
            core::ptr::NonNull::new(tx_buf.as_ptr() as *mut u8).unwrap(),
            tx_buf.len()
        );
        if let Err(e) = self.device.transmit(net_buf) {
            warn!("Send Package Failed: {:?}", e);
        }
        
        result
    }
}

/// Loopback device adapter
pub struct LoopbackDevice {
    queue: Mutex<Vec<Vec<u8>>>,
}

impl LoopbackDevice {
    /// Create new loopback device
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
        }
    }
}

impl Device for LoopbackDevice {
    type RxToken<'a> = LoopbackRxToken where Self: 'a;
    type TxToken<'a> = LoopbackTxToken<'a> where Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let mut queue = self.queue.lock();
        if let Some(packet) = queue.pop() {
            let rx_token = LoopbackRxToken { buffer: packet };
            let tx_token = LoopbackTxToken {
                queue: &self.queue,
            };
            Some((rx_token, tx_token))
        } else {
            None
        }
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(LoopbackTxToken {
            queue: &self.queue,
        })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 65535;
        caps.max_burst_size = Some(1);
        caps.medium = Medium::Ip;
        caps
    }
}

/// Loopback receive token
pub struct LoopbackRxToken {
    buffer: Vec<u8>,
}

impl RxToken for LoopbackRxToken {
    fn consume<R, F>(mut self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        f(&mut self.buffer)
    }
}

/// Loopback transmit token
pub struct LoopbackTxToken<'a> {
    queue: &'a Mutex<Vec<Vec<u8>>>,
}

impl<'a> TxToken for LoopbackTxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = vec![0u8; len];
        let result = f(&mut buffer);
        
        // Put packet into queue to implement loopback
        self.queue.lock().push(buffer);
        
        result
    }
}