//! `Shared<T>` — an atomic-refcount shared reference over the platform
//! allocator.
//!
//! The `Arc` analog for the zero-std floor, **strong counts only** — there is
//! no weak count. The floor has no weak-reference consumer yet; per the
//! rule-of-three, the weak side is not built until a real consumer needs it.
//! There is no `alloc` crate, so the shared box comes from the platform
//! allocator (reached through the boot-installed `kabi` handle).
//!
//! ## Refcount orderings
//!
//! - **Clone** increments the strong count with [`Relaxed`]: a new reference
//!   does not publish or consume any data, only bumps the count, so no
//!   ordering with other memory is needed.
//! - **Drop** decrements with [`Release`]: every prior use of the value
//!   through this reference must happen-before the decrement, so a later
//!   destroyer sees them. The thread that drops the last reference then issues
//!   an [`Acquire`] fence before reading the value out / freeing, pairing with
//!   all the `Release` decrements so every prior thread's writes are visible
//!   before the box is destroyed. This is the canonical `Arc` ordering.
//!
//! ## Overflow guard
//!
//! [`Clone`] aborts the clone (treats the refcount as saturated, leaking the
//! allocation) once the count reaches `usize::MAX / 2`. A live program cannot
//! hold that many references — reaching it means a refcount-manipulation bug,
//! and saturating-then-leaking is strictly safer than wrapping the count to a
//! value that could later reach zero with live references outstanding.

use core::{
    alloc::Layout,
    ops::Deref,
    ptr::NonNull,
    sync::atomic::{
        AtomicUsize,
        Ordering::{Acquire, Relaxed, Release},
        fence,
    },
};

use reovim_kabi_platform::{AllocError, handle};

/// The strong-count value at which [`Clone`] saturates and leaks rather than
/// risk overflow. No live program holds this many references.
///
/// `pub` under `selftest` so the arch-hosted overflow-guard test can assert the
/// saturation boundary directly.
#[cfg(not(feature = "selftest"))]
const MAX_REFCOUNT: usize = usize::MAX / 2;
/// See above; `selftest`-visible variant for the arch-hosted shared tests.
#[cfg(feature = "selftest")]
pub const MAX_REFCOUNT: usize = usize::MAX / 2;

/// Whether a pre-increment count of `old` has reached the saturation guard.
///
/// Factored out so the guard's decision is unit-testable on both sides: a real
/// clone can never reach [`MAX_REFCOUNT`] (it would need `usize::MAX / 2` live
/// references), so the guarded path is otherwise structurally unreachable.
///
/// `pub` under `selftest` so the arch-hosted test can drive both sides of the
/// guard predicate.
#[cfg(not(feature = "selftest"))]
const fn refcount_overflowed(old: usize) -> bool {
    old >= MAX_REFCOUNT
}

/// See above; `selftest`-visible variant for the arch-hosted shared tests.
#[cfg(feature = "selftest")]
pub const fn refcount_overflowed(old: usize) -> bool {
    old >= MAX_REFCOUNT
}

/// The heap box a [`Shared`] points at: the strong count plus the value.
struct SharedBox<T> {
    strong: AtomicUsize,
    value: T,
}

/// An atomic-refcount shared reference to a `T`.
///
/// Cloning hands out another owner of the same heap value; the value is
/// dropped and freed when the last `Shared` is dropped. Strong counts only.
///
/// ```no_run
/// use reovim_lib_ds::Shared;
///
/// let a = Shared::try_new(42u32).unwrap();
/// assert_eq!(*a, 42);
/// assert_eq!(a.strong_count(), 1);
/// let b = a.clone();
/// assert_eq!(a.strong_count(), 2);
/// assert_eq!(*b, 42);
/// drop(b);
/// assert_eq!(a.strong_count(), 1);
/// ```
pub struct Shared<T> {
    ptr: NonNull<SharedBox<T>>,
}

impl<T> Shared<T> {
    /// Allocates a shared box holding `value` with a strong count of 1.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] when the backing allocation is refused.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Shared;
    /// let s = Shared::try_new(99u32).unwrap();
    /// assert_eq!(*s, 99);
    /// assert_eq!(s.strong_count(), 1);
    /// ```
    pub fn try_new(value: T) -> Result<Self, AllocError> {
        let layout = Layout::new::<SharedBox<T>>();
        let raw = handle().alloc(layout)?.cast::<SharedBox<T>>();
        // SAFETY: `raw` is a fresh, correctly-sized, aligned allocation for
        // `SharedBox<T>`; writing the initial box initializes it.
        unsafe {
            core::ptr::write(
                raw.as_ptr(),
                SharedBox {
                    strong: AtomicUsize::new(1),
                    value,
                },
            );
        }
        Ok(Self { ptr: raw })
    }

    /// The box behind the pointer.
    const fn inner(&self) -> &SharedBox<T> {
        // SAFETY: `self.ptr` points at a live box whose strong count includes
        // this `Shared`, so it has not been freed.
        unsafe { self.ptr.as_ref() }
    }

    /// The current strong reference count.
    ///
    /// ```no_run
    /// use reovim_lib_ds::Shared;
    /// let a = Shared::try_new(1u32).unwrap();
    /// let b = a.clone();
    /// assert_eq!(a.strong_count(), 2);
    /// drop(b);
    /// assert_eq!(a.strong_count(), 1);
    /// ```
    #[must_use]
    pub fn strong_count(&self) -> usize {
        self.inner().strong.load(Relaxed)
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        // Relaxed: a clone only bumps the count; it publishes/consumes nothing.
        let old = self.inner().strong.fetch_add(1, Relaxed);
        // Saturate-and-leak past the guard rather than risk an overflow that
        // could wrap to a count reaching zero with live owners outstanding.
        // The predicate is tested directly (`refcount_overflowed`); a real
        // clone cannot reach the guard.
        assert!(!refcount_overflowed(old), "Shared strong count overflow");
        Self { ptr: self.ptr }
    }
}

impl<T> Deref for Shared<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner().value
    }
}

impl<T> Drop for Shared<T> {
    fn drop(&mut self) {
        // Release: every use of the value through this reference happens-before
        // this decrement, so the final destroyer observes it.
        if self.inner().strong.fetch_sub(1, Release) != 1 {
            return;
        }
        // This was the last reference. Acquire-fence to pair with all the
        // Release decrements, making every prior owner's writes visible before
        // the value is dropped and the box freed.
        fence(Acquire);
        // SAFETY: the strong count reached zero, so this is the unique owner;
        // dropping the value in place runs its destructor exactly once.
        unsafe {
            core::ptr::drop_in_place(core::ptr::addr_of_mut!((*self.ptr.as_ptr()).value));
        }
        let layout = Layout::new::<SharedBox<T>>();
        // The box came from `handle().alloc` with this exact layout and is no
        // longer referenced (count zero, value dropped).
        handle().dealloc(self.ptr.cast(), layout);
    }
}

// SAFETY: `Shared<T>` hands out shared access to `T` across threads, so `T`
// must be `Sync` to allow concurrent `&T`, and `Send` so the value can be
// dropped on whichever thread holds the last reference. These are the standard
// `Arc` bounds.
unsafe impl<T: Send + Sync> Send for Shared<T> {}
// SAFETY: sharing a `&Shared<T>` lets another thread clone it and read the
// value, requiring the same `T: Send + Sync` bounds as `Send`. Standard `Arc`
// bounds.
unsafe impl<T: Send + Sync> Sync for Shared<T> {}
