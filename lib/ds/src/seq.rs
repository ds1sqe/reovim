//! `Seq<T>` — a growable sequence owning raw allocator memory.
//!
//! The `Vec` analog for the zero-std floor. There is no `alloc` crate, so
//! `Seq` reaches the platform allocator through the boot-installed `kabi`
//! handle and is fallible where growth can fail ([`try_push`](Seq::try_push)
//! surfaces [`AllocError`]).
//!
//! `T` must be non-zero-sized. The floor has no zero-sized-element consumer,
//! and a non-ZST bound keeps the capacity/pointer arithmetic free of the ZST
//! special-casing `Vec` carries. The bound is enforced at compile time via a
//! const assertion, so a ZST `Seq` simply does not build.

use core::{
    alloc::Layout,
    ops::{Index, IndexMut},
    ptr::NonNull,
};

use reovim_kabi_platform::{AllocError, handle};

/// The capacity a non-empty `Seq` first grows to. Small, because most floor
/// sequences are tiny; growth doubles from here.
///
/// `pub` under `selftest` so the growth-ladder selftests (hosted by `arch`,
/// which owns the `no_std` runner) can assert the doubling boundary.
#[cfg(not(feature = "selftest"))]
const FIRST_CAPACITY: usize = 4;
/// See above; `selftest`-visible variant for the arch-hosted growth tests.
#[cfg(feature = "selftest")]
pub const FIRST_CAPACITY: usize = 4;

/// Resizes the allocation at `ptr` (`old` layout) to `new_size` bytes,
/// preserving the `min(old.size(), new_size)` leading bytes.
///
/// lib/ds-side realloc (SP03 settled decision 1: no `realloc` vtable slot). The
/// handle exposes only `alloc`/`dealloc`; a grow is `alloc + copy + dealloc`,
/// the same shape arch's old `realloc` used internally. The returned pointer
/// may differ from `ptr`.
///
/// # Safety
///
/// `ptr`/`old` must name a live allocation from a prior `handle().alloc`;
/// `new_size` is non-zero (the callers guarantee it via a non-zero `Layout`).
unsafe fn realloc(
    ptr: NonNull<u8>,
    old: Layout,
    new_size: usize,
) -> Result<NonNull<u8>, AllocError> {
    // Keep alignment stable across the grow: the new layout reuses old's align.
    let new_layout = Layout::from_size_align(new_size, old.align()).map_err(|_| AllocError)?;
    let fresh = handle().alloc(new_layout)?;
    let copy = if new_size < old.size() {
        new_size
    } else {
        old.size()
    };
    // SAFETY: `ptr` is live for `old.size() >= copy` bytes; `fresh` is freshly
    // allocated for `new_size >= copy` bytes; the two are distinct allocations,
    // so they do not overlap.
    unsafe {
        core::ptr::copy_nonoverlapping(ptr.as_ptr(), fresh.as_ptr(), copy);
    }
    // The original allocation is no longer referenced after the copy.
    handle().dealloc(ptr, old);
    Ok(fresh)
}

/// A growable, heap-owning sequence of `T`.
///
/// Owns a single allocation of `capacity` `T`-slots, of which the first `len`
/// are initialized. Empty `Seq`s hold no allocation (a dangling pointer and
/// zero capacity), so [`new`](Seq::new) never allocates.
///
/// ```no_run
/// use reovim_lib_ds::Seq;
///
/// let mut s: Seq<u32> = Seq::new();
/// assert!(s.is_empty());
/// s.try_push(1).unwrap();
/// s.try_push(2).unwrap();
/// s.try_push(3).unwrap();
/// assert_eq!(s.len(), 3);
/// assert_eq!(s[0], 1);
/// assert_eq!(s[2], 3);
/// assert_eq!(s.pop(), Some(3));
/// assert_eq!(s.len(), 2);
/// ```
pub struct Seq<T> {
    ptr: NonNull<T>,
    len: usize,
    cap: usize,
}

impl<T> Seq<T> {
    /// Compile-time guard: `T` must not be zero-sized.
    const NON_ZST: () = assert!(core::mem::size_of::<T>() != 0, "Seq<T> requires a non-ZST T");

    /// Creates an empty `Seq` with no allocation.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Seq;
    /// let s: Seq<i32> = Seq::new();
    /// assert!(s.is_empty());
    /// assert_eq!(s.len(), 0);
    /// assert_eq!(s.capacity(), 0);
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        // Force the non-ZST assertion to be evaluated.
        () = Self::NON_ZST;
        Self {
            ptr: NonNull::dangling(),
            len: 0,
            cap: 0,
        }
    }

    /// The number of initialized elements.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Seq;
    /// let mut s: Seq<u8> = Seq::new();
    /// assert_eq!(s.len(), 0);
    /// s.try_push(42).unwrap();
    /// assert_eq!(s.len(), 1);
    /// ```
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the sequence holds no elements.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Seq;
    /// let mut s: Seq<u8> = Seq::new();
    /// assert!(s.is_empty());
    /// s.try_push(1).unwrap();
    /// assert!(!s.is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The current allocated capacity in elements.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Seq;
    /// let mut s: Seq<u8> = Seq::new();
    /// assert_eq!(s.capacity(), 0);
    /// s.try_push(1).unwrap();
    /// assert!(s.capacity() >= 1);
    /// ```
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.cap
    }

    /// The `Layout` of `n` `T`-slots. `n` is non-zero at the call sites.
    fn layout_for(n: usize) -> Result<Layout, AllocError> {
        Layout::array::<T>(n).map_err(|_| AllocError)
    }

    /// Ensures capacity for at least one more element, growing if full.
    fn grow_if_full(&mut self) -> Result<(), AllocError> {
        if self.len < self.cap {
            return Ok(());
        }
        // One body owns growth: the doubling ladder, the layout checks, and
        // the alloc/realloc arms all live in `try_reserve` alone, so every
        // growth branch is a single, directly-tested code path.
        self.try_reserve(1)
    }

    /// Ensures capacity for at least `additional` more elements.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] when the required capacity overflows the
    /// layout size or the allocation is refused; the `Seq` is unchanged on
    /// error.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Seq;
    /// let mut s: Seq<u32> = Seq::new();
    /// s.try_reserve(10).unwrap();
    /// assert!(s.capacity() >= 10);
    /// ```
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), AllocError> {
        let needed = self.len.checked_add(additional).ok_or(AllocError)?;
        if needed <= self.cap {
            return Ok(());
        }
        // Grow to the doubling ladder's next rung covering `needed`, so a
        // reserve does not defeat amortized growth.
        let mut new_cap = if self.cap == 0 {
            FIRST_CAPACITY
        } else {
            self.cap
        };
        while new_cap < needed {
            new_cap = new_cap.checked_mul(2).ok_or(AllocError)?;
        }
        // The layout check is a REAL arm: a `new_cap` that fits `checked_mul`
        // can still overflow `Layout::array`'s `isize::MAX` byte bound (e.g.
        // reserving `isize::MAX/8 + 1` u64 slots), so the Err propagates.
        let new_layout = Self::layout_for(new_cap)?;
        let new_ptr = if self.cap == 0 {
            handle().alloc(new_layout)?
        } else {
            // SAFETY: this recomputes, deterministically, the exact layout that
            // succeeded when the current `self.cap` allocation was made (same
            // `T`, same count); a pure function repeated on the same input
            // cannot newly fail.
            let old_layout = unsafe { Self::layout_for(self.cap).unwrap_unchecked() };
            // SAFETY: `self.ptr` is the live allocation of `old_layout`
            // (cap > 0 here); `realloc` preserves the initialized prefix.
            unsafe { realloc(self.ptr.cast(), old_layout, new_layout.size())? }
        };
        self.ptr = new_ptr.cast();
        self.cap = new_cap;
        Ok(())
    }

    /// Appends `value`, growing the backing allocation if necessary.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] when a needed growth allocation is refused; on
    /// error `value` is returned to the caller and the `Seq` is unchanged.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Seq;
    /// let mut s: Seq<u32> = Seq::new();
    /// s.try_push(10).unwrap();
    /// s.try_push(20).unwrap();
    /// assert_eq!(s[0], 10);
    /// assert_eq!(s[1], 20);
    /// ```
    pub fn try_push(&mut self, value: T) -> Result<(), AllocError> {
        self.grow_if_full()?;
        // SAFETY: `self.len < self.cap` now (grow ensured it); the slot at
        // `len` is allocated but uninitialized, so a `write` initializes it.
        unsafe {
            core::ptr::write(self.ptr.as_ptr().add(self.len), value);
        }
        self.len += 1;
        Ok(())
    }

    /// Removes and returns the last element, or `None` when empty.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Seq;
    /// let mut s: Seq<u32> = Seq::new();
    /// assert_eq!(s.pop(), None);
    /// s.try_push(7).unwrap();
    /// assert_eq!(s.pop(), Some(7));
    /// assert_eq!(s.pop(), None);
    /// ```
    pub const fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        // SAFETY: the slot at the new `len` was initialized and is now logically
        // removed; reading it out by value transfers ownership to the caller.
        Some(unsafe { core::ptr::read(self.ptr.as_ptr().add(self.len)) })
    }

    /// Returns a reference to the element at `index`, or `None` if out of
    /// bounds.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Seq;
    /// let mut s: Seq<u32> = Seq::new();
    /// s.try_push(5).unwrap();
    /// assert_eq!(s.get(0), Some(&5));
    /// assert_eq!(s.get(1), None);
    /// ```
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        // SAFETY: `index < len`, so the slot is initialized and in bounds.
        Some(unsafe { &*self.ptr.as_ptr().add(index) })
    }

    /// Returns a mutable reference to the element at `index`, or `None`.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Seq;
    /// let mut s: Seq<u32> = Seq::new();
    /// s.try_push(1).unwrap();
    /// if let Some(v) = s.get_mut(0) { *v = 99; }
    /// assert_eq!(s[0], 99);
    /// ```
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }
        // SAFETY: `index < len`, so the slot is initialized and in bounds; the
        // `&mut self` borrow makes the returned reference exclusive.
        Some(unsafe { &mut *self.ptr.as_ptr().add(index) })
    }

    /// Returns the initialized elements as a slice.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Seq;
    /// let mut s: Seq<u8> = Seq::new();
    /// s.try_push(1).unwrap();
    /// s.try_push(2).unwrap();
    /// assert_eq!(s.as_slice(), &[1, 2]);
    /// ```
    #[must_use]
    pub const fn as_slice(&self) -> &[T] {
        // SAFETY: the first `len` slots are initialized and contiguous; an
        // empty `Seq` yields an empty slice from the dangling-but-aligned ptr.
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    /// Returns the initialized elements as a mutable slice.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Seq;
    /// let mut s: Seq<u8> = Seq::new();
    /// s.try_push(10).unwrap();
    /// s.as_mut_slice()[0] = 20;
    /// assert_eq!(s[0], 20);
    /// ```
    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: as `as_slice`; `&mut self` makes the slice exclusive.
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }

    /// Iterates the initialized elements by reference.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Seq;
    /// let mut s: Seq<u32> = Seq::new();
    /// s.try_push(1).unwrap();
    /// s.try_push(2).unwrap();
    /// let collected: Vec<u32> = s.iter().copied().collect();
    /// assert_eq!(collected, vec![1, 2]);
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.as_slice().iter()
    }
}

// SAFETY: `Seq<T>` owns its heap allocation exclusively (a `NonNull<T>` it
// alloc'd and frees on drop), so it behaves like a `Vec<T>`: sending it to
// another thread moves sole ownership of the `T`s, sound when `T: Send`. The
// `NonNull` only blocks the auto-impl out of conservatism; the ownership is
// unique. Required so a `Mutex<Seq<T>>` can cross a thread boundary.
unsafe impl<T: Send> Send for Seq<T> {}
// SAFETY: a shared `&Seq<T>` hands out `&T`, so concurrent readers need
// `T: Sync`; matches `Vec`'s bound.
unsafe impl<T: Sync> Sync for Seq<T> {}

impl<T> Default for Seq<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, T> IntoIterator for &'a Seq<T> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T> Index<usize> for Seq<T> {
    type Output = T;

    fn index(&self, index: usize) -> &T {
        self.get(index).expect("Seq index out of bounds")
    }
}

impl<T> IndexMut<usize> for Seq<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        self.get_mut(index).expect("Seq index out of bounds")
    }
}

impl<T> Drop for Seq<T> {
    fn drop(&mut self) {
        if self.cap == 0 {
            return;
        }
        // Drop the initialized elements first.
        // SAFETY: the first `len` slots are initialized; dropping the slice in
        // place runs each element's destructor exactly once.
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.ptr.as_ptr(),
                self.len,
            ));
        }
        // Then free the backing allocation. `layout_for(cap)` succeeded when
        // the allocation was made, so it succeeds here too.
        let layout = Self::layout_for(self.cap).expect("layout valid at alloc time");
        // `self.ptr`/`layout` name the live allocation from the last grow; no
        // element is referenced after the drops above.
        handle().dealloc(self.ptr.cast(), layout);
    }
}
