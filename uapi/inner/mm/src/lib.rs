//! Product-facing memory-management vocabulary.
//!
//! This crate declares only the generic allocation control table currently
//! needed by pure data-structure code. Concrete allocation policy lives below
//! the system-kernel bridge.

#![no_std]

use core::{alloc::Layout, ptr::NonNull};

/// Failure to allocate through an injected allocation service.
///
/// ```rust
/// use reovim_uapi_mm::AllocError;
///
/// assert_eq!(AllocError, AllocError);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocError;

/// Function pointer for allocating a block with a Rust [`Layout`].
///
/// ```rust
/// use core::{alloc::Layout, ptr::NonNull};
/// use reovim_uapi_mm::{AllocError, AllocFn};
///
/// fn alloc(_: Layout) -> Result<NonNull<u8>, AllocError> { Err(AllocError) }
/// let f: AllocFn = alloc;
/// assert_eq!(f(Layout::new::<u8>()), Err(AllocError));
/// ```
pub type AllocFn = fn(Layout) -> Result<NonNull<u8>, AllocError>;

/// Function pointer for deallocating a block previously returned by the same
/// control table.
///
/// # Safety
///
/// `ptr` and `layout` must name a live allocation returned by the matching
/// [`AllocFn`], and that allocation must not be used after this call.
///
/// ```rust
/// use core::{alloc::Layout, ptr::NonNull};
/// use reovim_uapi_mm::DeallocFn;
///
/// fn dealloc(_: NonNull<u8>, _: Layout) {}
///
/// let f: DeallocFn = dealloc;
/// let mut byte = 0_u8;
/// f(NonNull::from(&mut byte), Layout::new::<u8>());
/// ```
pub type DeallocFn = fn(NonNull<u8>, Layout);

/// Product-facing allocation control table.
///
/// ```rust
/// use core::{alloc::Layout, ptr::NonNull};
/// use reovim_uapi_mm::{AllocControl, AllocError};
///
/// fn alloc(_: Layout) -> Result<NonNull<u8>, AllocError> { Err(AllocError) }
/// fn dealloc(_: NonNull<u8>, _: Layout) {}
///
/// let control = AllocControl::new(alloc, dealloc);
/// assert_eq!(control.allocate(Layout::new::<u8>()), Err(AllocError));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct AllocControl {
    /// Allocates a block with the requested layout.
    pub alloc_fn: AllocFn,
    /// Deallocates a block returned by `alloc_fn`.
    pub dealloc_fn: DeallocFn,
}

impl AllocControl {
    /// Creates a control table that refuses every allocation and ignores
    /// deallocation.
    ///
    /// ```rust
    /// use core::alloc::Layout;
    /// use reovim_uapi_mm::{AllocControl, AllocError};
    ///
    /// assert_eq!(AllocControl::noop().allocate(Layout::new::<u8>()), Err(AllocError));
    /// ```
    #[must_use]
    pub const fn noop() -> Self {
        Self::new(noop_alloc, noop_dealloc)
    }

    /// Creates an allocation control table.
    ///
    /// ```rust
    /// use core::{alloc::Layout, ptr::NonNull};
    /// use reovim_uapi_mm::{AllocControl, AllocError};
    ///
    /// fn alloc(_: Layout) -> Result<NonNull<u8>, AllocError> { Err(AllocError) }
    /// fn dealloc(_: NonNull<u8>, _: Layout) {}
    ///
    /// let _control = AllocControl::new(alloc, dealloc);
    /// ```
    #[must_use]
    pub const fn new(alloc_fn: AllocFn, dealloc_fn: DeallocFn) -> Self {
        Self {
            alloc_fn,
            dealloc_fn,
        }
    }

    /// Allocates a block with `layout`.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] when the allocation service refuses the request.
    ///
    /// ```rust
    /// use core::{alloc::Layout, ptr::NonNull};
    /// use reovim_uapi_mm::{AllocControl, AllocError};
    ///
    /// fn alloc(_: Layout) -> Result<NonNull<u8>, AllocError> {
    ///     Ok(NonNull::<u8>::dangling())
    /// }
    /// fn dealloc(_: NonNull<u8>, _: Layout) {}
    ///
    /// let ptr = AllocControl::new(alloc, dealloc)
    ///     .allocate(Layout::new::<u8>())
    ///     .unwrap();
    /// assert_eq!(ptr, NonNull::<u8>::dangling());
    /// ```
    pub fn allocate(self, layout: Layout) -> Result<NonNull<u8>, AllocError> {
        (self.alloc_fn)(layout)
    }

    /// Deallocates a block previously returned by this control table.
    ///
    /// # Safety
    ///
    /// `ptr` and `layout` must name a live allocation returned by this table,
    /// and the caller must not use that allocation after this call.
    ///
    /// ```rust
    /// use core::{alloc::Layout, ptr::NonNull};
    /// use reovim_uapi_mm::{AllocControl, AllocError};
    ///
    /// fn alloc(_: Layout) -> Result<NonNull<u8>, AllocError> {
    ///     Ok(NonNull::<u8>::dangling())
    /// }
    /// fn dealloc(_: NonNull<u8>, _: Layout) {}
    ///
    /// let mut byte = 0_u8;
    /// AllocControl::new(alloc, dealloc).deallocate(
    ///     NonNull::from(&mut byte),
    ///     Layout::new::<u8>(),
    /// );
    /// ```
    pub fn deallocate(self, ptr: NonNull<u8>, layout: Layout) {
        (self.dealloc_fn)(ptr, layout);
    }
}

impl Default for AllocControl {
    fn default() -> Self {
        Self::noop()
    }
}

fn noop_alloc(_: Layout) -> Result<NonNull<u8>, AllocError> {
    Err(AllocError)
}

fn noop_dealloc(_: NonNull<u8>, _: Layout) {}
