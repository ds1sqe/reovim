//! Shared non-driver traits.
//!
//! Currently hosts only `PortAllocator`, the transport-neutral port
//! picker consumed internally by the gRPC driver when
//! `TransportConfig::Tcp` requests an OS-pick fallback.

use crate::{NetError, transport::TransportConfig};

/// Port allocator for multi-instance TCP server support.
pub trait PortAllocator: Send + Sync {
    /// Default port. Default: 12521 (`'r'*100 + 'e'*10 + 'o'`).
    fn default_port(&self) -> u16 {
        TransportConfig::DEFAULT_PORT
    }

    /// Fallback port range, inclusive. Default: (12521, 12530).
    fn port_range(&self) -> (u16, u16) {
        (TransportConfig::DEFAULT_PORT, TransportConfig::MAX_PORT)
    }

    /// Try to allocate a port, starting with the default and
    /// falling back through `port_range`.
    ///
    /// # Errors
    /// [`NetError::PortExhausted`] when every port in the range is
    /// already in use.
    fn allocate(&self) -> Result<u16, NetError>;

    /// True when the port is currently available.
    fn is_port_available(&self, port: u16) -> bool;
}

#[cfg(test)]
#[path = "traits_tests.rs"]
mod tests;
