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
mod tests {
    use {super::*, std::sync::Arc};

    #[test]
    fn test_mutex_basic() {
        let mutex = Mutex::new(42);
        {
            let guard = mutex.lock();
            assert_eq!(*guard, 42);
            drop(guard);
        }
        {
            let mut guard = mutex.lock();
            *guard = 100;
        }
        {
            let guard = mutex.lock();
            assert_eq!(*guard, 100);
            drop(guard);
        }
    }

    #[test]
    fn test_rwlock_basic() {
        let rwlock = RwLock::new(String::from("hello"));
        {
            let read = rwlock.read();
            assert_eq!(*read, "hello");
            drop(read);
        }
        {
            let mut write = rwlock.write();
            write.push_str(" world");
        }
        {
            let read = rwlock.read();
            assert_eq!(*read, "hello world");
            drop(read);
        }
    }

    #[test]
    fn test_rwlock_multiple_readers() {
        let rwlock = RwLock::new(10);
        let r1 = rwlock.read();
        let r2 = rwlock.read();
        assert_eq!(*r1, 10);
        drop(r1);
        assert_eq!(*r2, 10);
        drop(r2);
    }

    #[test]
    fn test_arc_swap_basic() {
        let arc_swap = ArcSwap::from_pointee(42);
        let loaded = arc_swap.load();
        assert_eq!(**loaded, 42);

        arc_swap.store(Arc::new(100));
        let loaded = arc_swap.load();
        assert_eq!(**loaded, 100);
    }

    #[test]
    fn test_condvar_basic() {
        let pair = Arc::new((Mutex::new(false), Condvar::new()));
        let pair_clone = Arc::clone(&pair);

        let thread = std::thread::spawn(move || {
            let (lock, cvar) = &*pair_clone;
            let mut started = lock.lock();
            *started = true;
            drop(started);
            cvar.notify_one();
        });

        let (lock, cvar) = &*pair;
        let mut started = lock.lock();
        while !*started {
            cvar.wait(&mut started);
        }
        assert!(*started);
        drop(started);

        thread.join().unwrap();
    }
}
