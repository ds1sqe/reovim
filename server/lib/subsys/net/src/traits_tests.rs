//! Tests for `PortAllocator`.

use {
    super::PortAllocator,
    crate::{NetError, transport::TransportConfig},
    std::sync::atomic::{AtomicU16, Ordering},
};

struct StubAllocator {
    next: AtomicU16,
    unavailable_below: u16,
}

impl PortAllocator for StubAllocator {
    fn allocate(&self) -> Result<u16, NetError> {
        let (lo, hi) = self.port_range();
        let p = self.next.fetch_add(1, Ordering::SeqCst);
        if p < lo || p > hi {
            Err(NetError::PortExhausted)
        } else {
            Ok(p)
        }
    }

    fn is_port_available(&self, port: u16) -> bool {
        port >= self.unavailable_below
    }
}

#[test]
fn default_port_matches_constant() {
    let a = StubAllocator {
        next: AtomicU16::new(TransportConfig::DEFAULT_PORT),
        unavailable_below: 0,
    };
    assert_eq!(a.default_port(), TransportConfig::DEFAULT_PORT);
}

#[test]
fn port_range_is_default_to_max() {
    let a = StubAllocator {
        next: AtomicU16::new(TransportConfig::DEFAULT_PORT),
        unavailable_below: 0,
    };
    let (lo, hi) = a.port_range();
    assert_eq!(lo, TransportConfig::DEFAULT_PORT);
    assert_eq!(hi, TransportConfig::MAX_PORT);
}

#[test]
fn allocate_within_range_returns_port() {
    let a = StubAllocator {
        next: AtomicU16::new(TransportConfig::DEFAULT_PORT),
        unavailable_below: 0,
    };
    let p = a.allocate().expect("port in range");
    assert_eq!(p, TransportConfig::DEFAULT_PORT);
}

#[test]
fn allocate_exhausts_when_past_max() {
    let a = StubAllocator {
        next: AtomicU16::new(TransportConfig::MAX_PORT + 1),
        unavailable_below: 0,
    };
    assert!(matches!(a.allocate(), Err(NetError::PortExhausted)));
}

#[test]
fn is_port_available_honors_stub_policy() {
    let a = StubAllocator {
        next: AtomicU16::new(TransportConfig::DEFAULT_PORT),
        unavailable_below: 12525,
    };
    assert!(!a.is_port_available(12524));
    assert!(a.is_port_available(12525));
    assert!(a.is_port_available(12530));
}

#[test]
fn trait_is_object_safe() {
    let a: Box<dyn PortAllocator> = Box::new(StubAllocator {
        next: AtomicU16::new(TransportConfig::DEFAULT_PORT),
        unavailable_below: 0,
    });
    assert_eq!(a.default_port(), TransportConfig::DEFAULT_PORT);
}

#[test]
fn trait_is_send_sync() {
    fn assert_send_sync<T: Send + Sync + ?Sized>() {}
    assert_send_sync::<dyn PortAllocator>();
}
