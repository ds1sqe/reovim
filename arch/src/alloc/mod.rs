//! The mmap-backed arch allocator.
//!
//! `arch/` forbids the `alloc` crate (DAG6), so there is no `Box`/`Vec`/
//! global allocator to lean on. This module is the one heap source the arch
//! data structures call directly. It is an ordinary API, **not** a
//! `#[global_allocator]` (that attribute requires the `alloc` crate).
//!
//! ## Shape
//!
//! Two paths, split on requested size:
//!
//! - **Small** (`size <= MAX_SMALL`): served from a size-class free-list.
//!   Each class owns a chain of slots carved out of whole pages obtained via
//!   `mmap`. A freed slot returns to its class's free-list; pages are never
//!   handed back to the kernel (a slab, by design — see the note on
//!   [`Allocator`]).
//! - **Large** (`size > MAX_SMALL`): a dedicated, page-rounded `mmap` whose
//!   length is recorded inline so [`dealloc`]/[`realloc`] can `munmap` it
//!   exactly.
//!
//! ## Accounting
//!
//! [`live_bytes`] reports the exact number of *requested* bytes currently
//! handed out (not the rounded backing size). It is an atomic counter bumped
//! on [`alloc`]/[`realloc`] and decremented on [`dealloc`]/[`realloc`], so a
//! balanced alloc/free workload returns it to its starting value — the
//! Phase 2 integration smoke asserts exactly this.
//!
//! ## Thread-safety
//!
//! The free-list is guarded by a private [`SpinLock`]. A futex-backed mutex
//! does not exist yet (Phase 3); the spin lock is the minimal primitive that
//! makes the allocator usable from the threads Phase 3 introduces. Phase 3
//! may revisit this once the futex mutex lands.
//!
//! ## realloc
//!
//! [`realloc`] is alloc-copy-free; it does not use `mremap`. The size-class
//! front already copies on class change, and large allocations are rare
//! enough that the extra syscall surface (`mremap` + a re-added `syscall5`)
//! is not justified until a consumer profiles it (rule of three).

use core::{
    alloc::Layout,
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::sys::{
    Errno, MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE, mmap as sys_mmap,
    munmap as sys_munmap,
};

#[cfg(feature = "selftest")]
pub(crate) mod fault;

/// The host page size. x86_64-linux uses 4 KiB base pages; the arch floor
/// hardcodes its single target (rule of three).
const PAGE_SIZE: usize = 4096;

/// The largest request served from the size-class front. Anything larger is
/// a dedicated page-rounded mapping.
const MAX_SMALL: usize = 2048;

/// The size classes, in bytes. Each is a power of two up to [`MAX_SMALL`];
/// powers of two keep alignment trivial (a class block is aligned to its own
/// size, which covers every alignment a request of that size can demand).
const SIZE_CLASSES: [usize; 7] = [16, 32, 64, 128, 256, 512, 2048];

/// Failure to allocate: the kernel refused the backing `mmap`, or the
/// request was malformed (zero size, or an alignment the front cannot meet).
///
/// ```rust
/// use reovim_arch::alloc::{AllocError, alloc};
/// use core::alloc::Layout;
///
/// // A zero-size layout is rejected.
/// let zero = Layout::from_size_align(0, 1).unwrap();
/// assert_eq!(alloc(zero), Err(AllocError));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocError;

/// A free slot in a size class's intrusive free-list.
///
/// While free, the first `size_of::<usize>()` bytes of the slot hold the
/// address of the next free slot (`0` terminates the chain). The slot is at
/// least 16 bytes (the smallest class), so the pointer always fits.
struct FreeNode {
    next: usize,
}

/// The mutable allocator state, guarded by the spin lock.
struct Inner {
    /// Head of each size class's free-list (`0` = empty). Indexed parallel
    /// to [`SIZE_CLASSES`].
    free_heads: [usize; SIZE_CLASSES.len()],
}

/// A minimal test-and-set spin lock over an atomic flag.
///
/// Private to the allocator. This is *not* a general lock primitive — the
/// futex-backed [`crate::sync`] mutex (Phase 3) is the real one. It exists
/// only so the allocator is thread-usable before that primitive lands.
struct SpinLock {
    locked: AtomicUsize,
}

impl SpinLock {
    const fn new() -> Self {
        Self {
            locked: AtomicUsize::new(0),
        }
    }

    /// Acquires the lock, spinning until the flag flips `0 -> 1`.
    fn lock(&self) {
        // The CAS is the contended/uncontended branch the tests force both
        // ways (a held lock makes a second acquire spin).
        while self
            .locked
            .compare_exchange_weak(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            while self.locked.load(Ordering::Relaxed) != 0 {
                core::hint::spin_loop();
            }
        }
    }

    /// Releases the lock.
    fn unlock(&self) {
        self.locked.store(0, Ordering::Release);
    }
}

/// The mmap-backed allocator.
///
/// A single shared instance lives in [`GLOBAL`]; arch data structures call
/// the free [`alloc`]/[`realloc`]/[`dealloc`] functions, which delegate to
/// it. By design the slab pages are never returned to the kernel — only the
/// free-list reclaims small slots — because the platform floor's allocations
/// are long-lived and a shrinking slab buys little. Large allocations *are*
/// unmapped on free.
///
/// ```rust
/// use reovim_arch::alloc::{alloc, dealloc, live_bytes};
/// use core::alloc::Layout;
///
/// let layout = Layout::from_size_align(64, 8).unwrap();
/// let before = live_bytes();
/// let ptr = alloc(layout).expect("allocation succeeds");
/// assert_eq!(live_bytes(), before + 64);
/// // SAFETY: `ptr` came from `alloc` with this exact layout.
/// unsafe { dealloc(ptr, layout); }
/// assert_eq!(live_bytes(), before);
/// ```
pub struct Allocator {
    inner: SpinLock,
    /// State the spin lock guards. Wrapped in a `Cell`-free raw form: the
    /// lock is the discipline, so the fields are reached through a raw
    /// pointer only while the lock is held.
    state: core::cell::UnsafeCell<Inner>,
    /// Exact requested-bytes currently live. Atomic so [`live_bytes`] needs
    /// no lock.
    live: AtomicUsize,
}

// SAFETY: every access to `state` goes through `inner` (the spin lock), and
// `live` is atomic. No `&mut` to `Inner` escapes the lock guard, so concurrent
// use is data-race-free.
unsafe impl Sync for Allocator {}

impl Allocator {
    const fn new() -> Self {
        Self {
            inner: SpinLock::new(),
            state: core::cell::UnsafeCell::new(Inner {
                free_heads: [0; SIZE_CLASSES.len()],
            }),
            live: AtomicUsize::new(0),
        }
    }
}

/// The process-wide allocator instance.
static GLOBAL: Allocator = Allocator::new();

/// Rounds `n` up to a multiple of [`PAGE_SIZE`]. `n` must be non-zero; the
/// callers guarantee that.
const fn page_round_up(n: usize) -> usize {
    (n + (PAGE_SIZE - 1)) & !(PAGE_SIZE - 1)
}

/// Returns the index of the smallest size class that fits `(size, align)`,
/// or `None` when the request belongs on the large path.
///
/// A class fits when its block size is `>= size` and `>= align` (a
/// power-of-two class block is aligned to its own size, so meeting `align`
/// only needs the block to be at least as large as the alignment).
///
/// DEV1 restructure (Phase 5 coverage): scanning only the first N-1 classes
/// and returning the last one unconditionally eliminates the dead `None` tail.
/// The last class is always `MAX_SMALL`; a `need <= MAX_SMALL` request that
/// passes the guard above always fits in the last class, so the final arm is
/// structurally `Some(N-1)`, never `None`. The sentinel `None` return that
/// used to follow the loop was unreachable by construction.
const fn class_for(size: usize, align: usize) -> Option<usize> {
    let need = if align > size { align } else { size };
    if need > MAX_SMALL {
        return None;
    }
    // Scan the first N-1 classes; the last is the unconditional fallback
    // (need <= MAX_SMALL == SIZE_CLASSES[N-1] always matches it).
    let mut i = 0;
    while i < SIZE_CLASSES.len() - 1 {
        if SIZE_CLASSES[i] >= need {
            return Some(i);
        }
        i += 1;
    }
    Some(SIZE_CLASSES.len() - 1)
}

/// Maps `len` anonymous, readable-writable bytes. `len` must be page-rounded
/// and non-zero.
fn map_pages(len: usize) -> Result<usize, Errno> {
    sys_mmap(0, len, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0)
}

/// Carves one fresh page into `block`-sized slots and links them onto the
/// free-list head `head`, returning the new head. The lock must be held.
///
/// Returns `Err` if the backing `mmap` is refused.
fn refill_class(block: usize, head: usize) -> Result<usize, AllocError> {
    // Selftest fault injection for the refill-specific `mmap`-refusal arm: the
    // `alloc()` entry hook returns before reaching here, so this dedicated hook
    // is what drives `alloc_small`'s refill-`Err` branch. Compiled out without
    // the `selftest` feature.
    // The selftest refill hook injects an Err *into the mmap result* (it
    // never calls mmap, so no page leaks), so the genuine refusal arm below
    // — the same `map_err`/`?` the flight code takes on kernel OOM — is the
    // branch that executes.
    #[cfg(feature = "selftest")]
    let mapped = if fault::refill_should_fail() {
        Err(crate::sys::ENOMEM)
    } else {
        map_pages(PAGE_SIZE)
    };
    #[cfg(not(feature = "selftest"))]
    let mapped = map_pages(PAGE_SIZE);
    let base = mapped.map_err(|_| AllocError)?;
    let count = PAGE_SIZE / block;
    let mut next = head;
    // Thread the slots front-to-back so the first slot carved is handed out
    // first; the chain order does not matter for correctness.
    let mut i = 0;
    while i < count {
        let slot = base + i * block;
        // SAFETY: `slot` is within the freshly-mapped, writable page and is
        // `block`-aligned (>= 16, so a `usize` write is aligned). Writing the
        // free-list link into the slot's first word is the intrusive-list
        // contract; the slot is not otherwise live.
        unsafe {
            core::ptr::write(slot as *mut FreeNode, FreeNode { next });
        }
        next = slot;
        i += 1;
    }
    Ok(next)
}

/// Allocates `layout.size()` bytes aligned to `layout.align()`.
///
/// # Errors
///
/// Returns [`AllocError`] when the size is zero, the alignment exceeds what
/// the small front can serve for that size, or the kernel refuses the
/// backing `mmap` (out of memory). Never panics.
///
/// ```rust
/// use reovim_arch::alloc::{alloc, dealloc};
/// use core::alloc::Layout;
///
/// let layout = Layout::from_size_align(128, 8).unwrap();
/// let ptr = alloc(layout).expect("small allocation succeeds");
/// // SAFETY: ptr came from alloc with this layout.
/// unsafe { dealloc(ptr, layout); }
/// ```
pub fn alloc(layout: Layout) -> Result<NonNull<u8>, AllocError> {
    let size = layout.size();
    if size == 0 {
        return Err(AllocError);
    }
    // Selftest fault injection: when armed, simulate a kernel allocation
    // refusal so the DS-level `?`-propagation `Err` arms are reachable
    // in-process. Compiled out entirely without the `selftest` feature.
    #[cfg(feature = "selftest")]
    if fault::alloc_should_fail() {
        return Err(AllocError);
    }
    let ptr = match class_for(size, layout.align()) {
        Some(class) => alloc_small(class)?,
        None => alloc_large(size, layout.align())?,
    };
    GLOBAL.live.fetch_add(size, Ordering::Relaxed);
    Ok(ptr)
}

/// Serves a small request from `class`'s free-list, refilling from a fresh
/// page when the list is empty.
fn alloc_small(class: usize) -> Result<NonNull<u8>, AllocError> {
    let block = SIZE_CLASSES[class];
    GLOBAL.inner.lock();
    // SAFETY: the spin lock is held, so this is the unique live reference to
    // `Inner` for the duration of the block.
    let inner = unsafe { &mut *GLOBAL.state.get() };
    let head = inner.free_heads[class];
    let head = if head == 0 {
        match refill_class(block, head) {
            Ok(h) => h,
            Err(e) => {
                GLOBAL.inner.unlock();
                return Err(e);
            }
        }
    } else {
        head
    };
    // Pop the head slot.
    // SAFETY: `head` is a slot address previously written by `refill_class`
    // (or a prior `dealloc`); its first word holds the next link.
    let node = unsafe { &*(head as *const FreeNode) };
    inner.free_heads[class] = node.next;
    GLOBAL.inner.unlock();
    // SAFETY: `head` is non-zero (a mapped slot), so the pointer is valid.
    Ok(unsafe { NonNull::new_unchecked(head as *mut u8) })
}

/// The inline header preceding a large allocation, recording the full mapped
/// length so `dealloc`/`realloc` can `munmap` exactly.
///
/// The header occupies the first [`LARGE_HEADER`] bytes of the mapping; the
/// returned pointer is offset past it. The offset is page-rounded-free: the
/// header size covers every alignment the large path serves (see
/// [`alloc_large`]).
#[repr(C)]
struct LargeHeader {
    map_len: usize,
}

/// Bytes reserved at the front of a large mapping for [`LargeHeader`].
///
/// Sized to [`MAX_SMALL`]'s next power of two so any alignment a large
/// request can demand (alignments larger than this belong nowhere in the
/// floor and are rejected) is satisfied by offsetting the user pointer by
/// this fixed amount.
const LARGE_HEADER: usize = 4096;

/// Serves a large request with a dedicated page-rounded mapping.
fn alloc_large(size: usize, align: usize) -> Result<NonNull<u8>, AllocError> {
    if align > LARGE_HEADER {
        // An alignment larger than the header offset cannot be guaranteed by
        // the fixed-offset scheme; the floor has no such consumer.
        return Err(AllocError);
    }
    let map_len = page_round_up(LARGE_HEADER + size);
    let base = map_pages(map_len).map_err(|_| AllocError)?;
    // SAFETY: `base` is a fresh writable mapping of `map_len >= LARGE_HEADER`
    // bytes; writing the header into its first word is in bounds and aligned
    // (page-aligned base).
    unsafe {
        core::ptr::write(base as *mut LargeHeader, LargeHeader { map_len });
    }
    let user = base + LARGE_HEADER;
    // SAFETY: `user` is `base + LARGE_HEADER`, in bounds of the mapping and
    // non-zero; `LARGE_HEADER` is a multiple of every supported alignment.
    Ok(unsafe { NonNull::new_unchecked(user as *mut u8) })
}

/// Frees a block previously returned by [`alloc`]/[`realloc`] for `layout`.
///
/// `ptr` and `layout` MUST match the original allocation. Small blocks return
/// to their size-class free-list; large blocks are `munmap`-ped.
///
/// # Safety
///
/// `ptr` must come from a prior [`alloc`]/[`realloc`] call with the same
/// `layout` and must not have been freed already.
///
/// ```rust
/// use reovim_arch::alloc::{alloc, dealloc};
/// use core::alloc::Layout;
///
/// let layout = Layout::from_size_align(32, 8).unwrap();
/// let ptr = alloc(layout).unwrap();
/// // SAFETY: ptr was just allocated with this exact layout.
/// unsafe { dealloc(ptr, layout); }
/// ```
pub unsafe fn dealloc(ptr: NonNull<u8>, layout: Layout) {
    let size = layout.size();
    if size == 0 {
        // A zero-size layout never produced a real allocation; nothing to do.
        return;
    }
    match class_for(size, layout.align()) {
        Some(class) => {
            // SAFETY: by the function contract `ptr` is a live small slot of
            // this class; pushing it back onto the free-list reuses its first
            // word for the link.
            unsafe { dealloc_small(ptr, class) }
        }
        None => {
            // SAFETY: by the function contract `ptr` is a live large user
            // pointer; the header sits `LARGE_HEADER` bytes before it.
            unsafe { dealloc_large(ptr) }
        }
    }
    GLOBAL.live.fetch_sub(size, Ordering::Relaxed);
}

/// Pushes a small slot back onto its class free-list.
///
/// # Safety
///
/// `ptr` is a live slot of `class`.
unsafe fn dealloc_small(ptr: NonNull<u8>, class: usize) {
    let slot = ptr.as_ptr() as usize;
    GLOBAL.inner.lock();
    // SAFETY: the spin lock is held; this is the unique live `Inner` ref.
    let inner = unsafe { &mut *GLOBAL.state.get() };
    let head = inner.free_heads[class];
    // SAFETY: `slot` is a writable block of at least 16 bytes; storing the
    // old head as the link reattaches it to the free-list.
    unsafe {
        core::ptr::write(slot as *mut FreeNode, FreeNode { next: head });
    }
    inner.free_heads[class] = slot;
    GLOBAL.inner.unlock();
}

/// Unmaps a large allocation.
///
/// # Safety
///
/// `ptr` is a live large user pointer; the [`LargeHeader`] precedes it.
unsafe fn dealloc_large(ptr: NonNull<u8>) {
    let user = ptr.as_ptr() as usize;
    let base = user - LARGE_HEADER;
    // SAFETY: `base` is the mapping start; its first word is the header
    // written by `alloc_large`.
    let map_len = unsafe { (*(base as *const LargeHeader)).map_len };
    // The munmap result is ignored: a correct (ptr, layout) pair always names
    // a valid mapping, so failure is impossible under the safety contract.
    let _ = sys_munmap(base, map_len);
}

/// Resizes the allocation at `ptr` from `old` to `new_size`, preserving the
/// `min(old.size(), new_size)` leading bytes.
///
/// Implemented as alloc-copy-free; the returned pointer may differ from
/// `ptr`. A `new_size` of zero frees and returns [`AllocError`] (callers that
/// want a true free call [`dealloc`]).
///
/// # Errors
///
/// Returns [`AllocError`] on a zero `new_size` or when the fresh allocation
/// is refused. On error the original allocation is left untouched and valid.
///
/// # Safety
///
/// `ptr`/`old` must name a live allocation from a prior [`alloc`]/[`realloc`].
///
/// ```rust
/// use reovim_arch::alloc::{alloc, dealloc, realloc};
/// use core::alloc::Layout;
///
/// let old = Layout::from_size_align(64, 8).unwrap();
/// let ptr = alloc(old).unwrap();
/// // SAFETY: ptr came from alloc with `old`.
/// let ptr2 = unsafe { realloc(ptr, old, 128) }.unwrap();
/// let new_layout = Layout::from_size_align(128, 8).unwrap();
/// // SAFETY: ptr2 came from realloc with new size 128.
/// unsafe { dealloc(ptr2, new_layout); }
/// ```
pub unsafe fn realloc(
    ptr: NonNull<u8>,
    old: Layout,
    new_size: usize,
) -> Result<NonNull<u8>, AllocError> {
    if new_size == 0 {
        return Err(AllocError);
    }
    // The DS keep alignment stable across a grow, so the new layout reuses the
    // old alignment.
    let new_layout = Layout::from_size_align(new_size, old.align()).map_err(|_| AllocError)?;
    let fresh = alloc(new_layout)?;
    let copy = if new_size < old.size() {
        new_size
    } else {
        old.size()
    };
    // SAFETY: `ptr` is live for `old.size() >= copy` bytes; `fresh` is freshly
    // allocated for `new_size >= copy` bytes; the two regions do not overlap
    // (distinct allocations).
    unsafe {
        core::ptr::copy_nonoverlapping(ptr.as_ptr(), fresh.as_ptr(), copy);
    }
    // SAFETY: `ptr`/`old` name the live allocation being replaced; after the
    // copy it is no longer referenced.
    unsafe {
        dealloc(ptr, old);
    }
    Ok(fresh)
}

/// The exact number of requested bytes currently handed out.
///
/// The introspection hook for tests: a balanced alloc/free workload returns
/// this to its prior value. Counts *requested* bytes, not the rounded backing
/// size.
///
/// ```rust
/// use reovim_arch::alloc::{alloc, dealloc, live_bytes};
/// use core::alloc::Layout;
///
/// let layout = Layout::from_size_align(16, 8).unwrap();
/// let before = live_bytes();
/// let ptr = alloc(layout).unwrap();
/// assert_eq!(live_bytes(), before + 16);
/// // SAFETY: ptr came from alloc with this layout.
/// unsafe { dealloc(ptr, layout); }
/// assert_eq!(live_bytes(), before);
/// ```
#[must_use]
pub fn live_bytes() -> usize {
    GLOBAL.live.load(Ordering::Relaxed)
}

// L12 layout (#785 Phase 5): tests live in the sibling file `tests.rs`,
// declared as a `#[path]` child so `super::` reaches crate-private items
// (`class_for`, `page_round_up`, `LARGE_HEADER`, `PAGE_SIZE`, `SIZE_CLASSES`).
#[cfg(feature = "selftest")]
#[path = "tests.rs"]
mod tests;
