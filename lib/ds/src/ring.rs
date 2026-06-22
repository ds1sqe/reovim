//! `Ring<T>` — a bounded ring buffer with oldest-first eviction.
//!
//! The fixed-capacity circular buffer the LOG6 kernel log ring (and any other
//! bounded-history consumer) is built on. There is no `alloc` crate, so the
//! backing storage is a single allocation from the installed allocation
//! backend, made once at construction.
//!
//! When the ring is full, [`push`](Ring::push) evicts the oldest element and
//! returns it. The capacity is fixed at construction (a single allocation, no
//! growth) — a ring is a bounded window by definition, so growth would defeat
//! its purpose.
//!
//! ## Index arithmetic
//!
//! Storage is a flat `cap`-slot buffer; `head` is the oldest slot and `len`
//! the live count. The slot for logical position `i` is `(head + i) % cap`.
//! Modulo (not masking) is used because the capacity is caller-chosen and need
//! not be a power of two. Both wrap branches — `head` wrapping at eviction and
//! the tail write index wrapping — are exercised by the tests.

use core::{alloc::Layout, ptr::NonNull};

use crate::alloc_backend::{AllocError, alloc, dealloc};

/// A bounded ring buffer of `T` with oldest-first eviction.
///
/// Owns a single fixed allocation of `cap` slots, of which `len` (from `head`,
/// wrapping) are initialized. `cap` is always `>= 1`.
///
/// ```no_run
/// use reovim_lib_ds::Ring;
///
/// let mut r: Ring<u32> = Ring::try_with_capacity(3).unwrap();
/// assert!(r.push(1).is_none());
/// assert!(r.push(2).is_none());
/// assert!(r.push(3).is_none());
/// // Ring is full; next push evicts the oldest.
/// assert_eq!(r.push(4), Some(1));
/// assert_eq!(r.pop_oldest(), Some(2));
/// assert_eq!(r.len(), 2);
/// ```
pub struct Ring<T> {
    ptr: NonNull<T>,
    /// Index of the oldest live element.
    head: usize,
    /// Number of live elements.
    len: usize,
    /// Total slot count (fixed; `>= 1`).
    cap: usize,
}

impl<T> Ring<T> {
    /// Compile-time guard: `T` must not be zero-sized (as [`Seq`], the slot
    /// arithmetic assumes a non-zero stride).
    ///
    /// [`Seq`]: crate::Seq
    const NON_ZST: () = assert!(core::mem::size_of::<T>() != 0, "Ring<T> requires a non-ZST T");

    /// Creates a ring with room for exactly `cap` elements.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] when `cap` is zero, the layout overflows, or the
    /// single backing allocation is refused.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Ring;
    /// assert!(Ring::<u32>::try_with_capacity(0).is_err());
    /// let r: Ring<u32> = Ring::try_with_capacity(4).unwrap();
    /// assert_eq!(r.capacity(), 4);
    /// assert!(r.is_empty());
    /// ```
    pub fn try_with_capacity(cap: usize) -> Result<Self, AllocError> {
        () = Self::NON_ZST;
        if cap == 0 {
            return Err(AllocError);
        }
        let layout = Layout::array::<T>(cap).map_err(|_| AllocError)?;
        let ptr = alloc(layout)?.cast::<T>();
        Ok(Self {
            ptr,
            head: 0,
            len: 0,
            cap,
        })
    }

    /// The number of live elements.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Ring;
    /// let mut r: Ring<u8> = Ring::try_with_capacity(2).unwrap();
    /// assert_eq!(r.len(), 0);
    /// r.push(1);
    /// assert_eq!(r.len(), 1);
    /// ```
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// The fixed slot capacity.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Ring;
    /// let r: Ring<u8> = Ring::try_with_capacity(5).unwrap();
    /// assert_eq!(r.capacity(), 5);
    /// ```
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.cap
    }

    /// Whether the ring holds no elements.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Ring;
    /// let mut r: Ring<u8> = Ring::try_with_capacity(2).unwrap();
    /// assert!(r.is_empty());
    /// r.push(1);
    /// assert!(!r.is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the ring is at capacity (the next push evicts).
    ///
    /// ```no_run
    /// use reovim_lib_ds::Ring;
    /// let mut r: Ring<u8> = Ring::try_with_capacity(1).unwrap();
    /// assert!(!r.is_full());
    /// r.push(1);
    /// assert!(r.is_full());
    /// ```
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len == self.cap
    }

    /// The physical slot index for logical position `i` (`0` is oldest).
    const fn slot(&self, i: usize) -> usize {
        let raw = self.head + i;
        if raw >= self.cap { raw - self.cap } else { raw }
    }

    /// Appends `value`. When full, evicts and returns the oldest element;
    /// otherwise returns `None`.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Ring;
    /// let mut r: Ring<u32> = Ring::try_with_capacity(2).unwrap();
    /// assert_eq!(r.push(10), None);
    /// assert_eq!(r.push(20), None);
    /// // Full: next push evicts the oldest (10).
    /// assert_eq!(r.push(30), Some(10));
    /// ```
    pub const fn push(&mut self, value: T) -> Option<T> {
        if self.is_full() {
            // Overwrite the oldest slot, then advance head (the wrap branch).
            let head = self.head;
            // SAFETY: `head` indexes the live oldest slot; reading it out moves
            // the evicted element, then the slot is reinitialized by the write
            // below — no double-init, no leak.
            let evicted = unsafe { core::ptr::read(self.ptr.as_ptr().add(head)) };
            // SAFETY: same slot, now logically vacant; writing initializes it.
            unsafe {
                core::ptr::write(self.ptr.as_ptr().add(head), value);
            }
            self.head = if head + 1 >= self.cap { 0 } else { head + 1 };
            Some(evicted)
        } else {
            let tail = self.slot(self.len);
            // SAFETY: `tail` is a vacant slot within the allocation (len < cap),
            // so a write initializes it.
            unsafe {
                core::ptr::write(self.ptr.as_ptr().add(tail), value);
            }
            self.len += 1;
            None
        }
    }

    /// Removes and returns the oldest element, or `None` when empty.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Ring;
    /// let mut r: Ring<u32> = Ring::try_with_capacity(3).unwrap();
    /// assert_eq!(r.pop_oldest(), None);
    /// r.push(1); r.push(2); r.push(3);
    /// assert_eq!(r.pop_oldest(), Some(1));
    /// assert_eq!(r.pop_oldest(), Some(2));
    /// ```
    pub const fn pop_oldest(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let head = self.head;
        // SAFETY: `head` indexes the live oldest slot; reading it out moves the
        // element, and decrementing `len` + advancing `head` marks it vacant.
        let value = unsafe { core::ptr::read(self.ptr.as_ptr().add(head)) };
        self.head = if head + 1 >= self.cap { 0 } else { head + 1 };
        self.len -= 1;
        Some(value)
    }

    /// Iterates the live elements oldest-first.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Ring;
    /// let mut r: Ring<u32> = Ring::try_with_capacity(3).unwrap();
    /// r.push(1); r.push(2); r.push(3);
    /// let v: Vec<u32> = r.iter().copied().collect();
    /// assert_eq!(v, vec![1, 2, 3]);
    /// ```
    #[must_use]
    pub const fn iter(&self) -> RingIter<'_, T> {
        RingIter { ring: self, pos: 0 }
    }
}

/// Iterator over a [`Ring`]'s elements, oldest-first.
///
/// See [`Ring::iter`] for usage.
pub struct RingIter<'a, T> {
    ring: &'a Ring<T>,
    pos: usize,
}

impl<'a, T> Iterator for RingIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.ring.len {
            return None;
        }
        let slot = self.ring.slot(self.pos);
        self.pos += 1;
        // SAFETY: `slot` indexes a live element (pos < len), initialized and in
        // bounds; the borrow is tied to the ring's lifetime.
        Some(unsafe { &*self.ring.ptr.as_ptr().add(slot) })
    }
}

impl<'a, T> IntoIterator for &'a Ring<T> {
    type Item = &'a T;
    type IntoIter = RingIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T> Drop for Ring<T> {
    fn drop(&mut self) {
        // Drop only the live elements (head..head+len, wrapping); the vacant
        // slots hold no initialized value.
        for i in 0..self.len {
            let slot = self.slot(i);
            // SAFETY: `slot` indexes a live, initialized element; dropping it in
            // place runs its destructor exactly once.
            unsafe {
                core::ptr::drop_in_place(self.ptr.as_ptr().add(slot));
            }
        }
        // `cap >= 1`, so the allocation always exists; the layout that
        // succeeded at construction succeeds here.
        let layout = Layout::array::<T>(self.cap).expect("layout valid at alloc time");
        // SAFETY: `self.ptr`/`layout` name the single live allocation; no
        // element is referenced after the drops above.
        unsafe {
            dealloc(self.ptr.cast(), layout);
        }
    }
}

// `Ring<T>` owns a `NonNull<T>` raw pointer; `NonNull` is `!Send + !Sync`
// by default (conservative opt-out for raw pointer wrappers). The safety
// arguments are identical to `Seq<T>` and `std::vec::Vec<T>`:
//
// - `Send`: `Ring<T>` is the sole owner of its allocation (unique, no aliases
//   outside the type). Moving it to another thread is sound when `T: Send`,
//   because the receiving thread becomes the sole owner of the `T`s.
// - `Sync`: `&Ring<T>` gives shared read access to the `T` values; concurrent
//   shared reads are safe when `T: Sync`, matching `&Vec<T>`.
//
// SAFETY: see justification above; matches the pattern used by `Seq<T>`.
unsafe impl<T: Send> Send for Ring<T> {}
// SAFETY: see above.
unsafe impl<T: Sync> Sync for Ring<T> {}
