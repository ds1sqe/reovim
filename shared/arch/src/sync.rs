//! Platform-optimized synchronization primitives.
//!
//! This module re-exports synchronization primitives optimized for the
//! current platform. The kernel layer uses these instead of depending
//! directly on external crates, maintaining the dependency rule:
//!
//! ```text
//! shared/arch/          -> (platform crates)
//! server/lib/kernel/    -> shared/arch/ only
//! ```
//!
//! # Why not `std::sync`?
//!
//! - `parking_lot::Mutex`: No poisoning, smaller, ~2x faster
//! - `parking_lot::Condvar`: Can wait without holding guard
//! - `arc_swap::ArcSwap`: Lock-free reads (no std equivalent)
//!
//! # Usage
//!
//! ```ignore
//! use reovim_arch::sync::{Mutex, Condvar, ArcSwap};
//!
//! let mutex = Mutex::new(42);
//! let guard = mutex.lock();
//! ```

// === Lock-free Primitives ===

pub use arc_swap::ArcSwap;

// === Synchronization Primitives ===

pub use parking_lot::{Condvar, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
