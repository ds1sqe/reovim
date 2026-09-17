//! The process heap, counted. Nothing above this crate owns an
//! allocator, so the count is here: one `#[global_allocator]` around
//! `System` and three atomics. It counts the whole process, Modules
//! and Client included, as `/proc/<pid>/status` does.

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicU64, Ordering};
use std::alloc::System;

use crate::Heap;

/// Bytes handed out and not given back.
static LIVE: AtomicU64 = AtomicU64::new(0);
/// The largest `LIVE` ever reached.
static PEAK: AtomicU64 = AtomicU64::new(0);
/// Allocations made since the process began.
static ALLOCS: AtomicU64 = AtomicU64::new(0);

/// What the three counters hold now. `Relaxed` throughout: each number
/// is a reading of the heap, not a lock on it, and a reader asking
/// between two allocations is the only reader there is.
pub fn heap() -> Heap {
    Heap {
        live: LIVE.load(Ordering::Relaxed),
        peak: PEAK.load(Ordering::Relaxed),
        allocs: ALLOCS.load(Ordering::Relaxed),
    }
}

/// A size as the counters hold one.
fn counted(size: usize) -> u64 {
    u64::try_from(size).unwrap_or(u64::MAX)
}

fn grew(size: usize) {
    let size = counted(size);
    let live = LIVE.fetch_add(size, Ordering::Relaxed) + size;
    PEAK.fetch_max(live, Ordering::Relaxed);
    ALLOCS.fetch_add(1, Ordering::Relaxed);
}

fn shrank(size: usize) {
    LIVE.fetch_sub(counted(size), Ordering::Relaxed);
}

/// `System`, with what it hands out and takes back counted around it.
struct Counting;

#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let block = unsafe { System.alloc(layout) };
        if !block.is_null() {
            grew(layout.size());
        }
        block
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let block = unsafe { System.alloc_zeroed(layout) };
        if !block.is_null() {
            grew(layout.size());
        }
        block
    }

    unsafe fn dealloc(&self, block: *mut u8, layout: Layout) {
        unsafe { System.dealloc(block, layout) };
        shrank(layout.size());
    }

    unsafe fn realloc(
        &self,
        block: *mut u8,
        layout: Layout,
        size: usize,
    ) -> *mut u8 {
        let moved = unsafe { System.realloc(block, layout, size) };
        if !moved.is_null() {
            shrank(layout.size());
            grew(size);
        }
        moved
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;
