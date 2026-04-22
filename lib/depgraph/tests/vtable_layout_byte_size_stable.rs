//! ABI guard: `FfiRenderSurface` and `FfiRenderSurfaceHost` byte-size
//! stability.
//!
//! The FFI vtable that bridges `&mut dyn ChromeSurface` across a C
//! ABI was locked at Plan 24 landing. Plan 25 (17-γ.1) does not
//! touch the `ChromeSurface` trait or either FFI struct — yet this
//! probe exists precisely to catch *future accidental* amendments.
//!
//! Any change to `FfiRenderSurface`'s layout (add/remove a method
//! pointer, reorder fields) breaks the vtable ABI. Dynamic module
//! loaders compiled against the old layout would silently
//! mis-dispatch. The probe forces such changes to be intentional: if
//! the size changes, a reviewer must explicitly bump the baseline
//! here while also incrementing the FFI version in
//! `clients/lib/driver/src/ffi.rs`.
//!
//! The assertions scale with `size_of::<usize>()` rather than
//! hardcoding bytes so the probe runs on any pointer width (32/64).

use std::mem::size_of;

use reovim_client_driver::ffi::{FfiRenderSurface, FfiRenderSurfaceHost};

/// `FfiRenderSurface` layout (Plan 24 baseline):
///
/// - `opaque: *mut c_void` → 1 × pointer
/// - 6 × function pointer (`write_styled`, `apply_style`, `overlay_bg`,
///   `fill`, `clear`, `size`) → 6 × pointer
///
/// Total: 7 pointers.
const FFI_RENDER_SURFACE_PTRS: usize = 7;

/// `FfiRenderSurfaceHost` layout (Plan 24 baseline):
///
/// - `surface: &'a mut dyn ChromeSurface` → fat pointer, 2 × pointer
/// - `_anchor: PhantomData<&'a mut dyn ChromeSurface>` → 0 bytes
///
/// Total: 2 pointers.
const FFI_RENDER_SURFACE_HOST_PTRS: usize = 2;

#[test]
fn ffi_render_surface_byte_size_matches_plan24_baseline() {
    let expected = FFI_RENDER_SURFACE_PTRS * size_of::<usize>();
    let actual = size_of::<FfiRenderSurface>();
    assert_eq!(
        actual, expected,
        "FfiRenderSurface size changed: {expected} → {actual} bytes. \
         This indicates the FFI vtable layout has drifted. If this is \
         intentional, bump the FFI version in ffi.rs AND update \
         FFI_RENDER_SURFACE_PTRS in this probe. If unintentional, the \
         ABI is now broken for all modules compiled against the prior \
         layout — revert or roll forward deliberately."
    );
}

#[test]
fn ffi_render_surface_host_byte_size_matches_plan24_baseline() {
    let expected = FFI_RENDER_SURFACE_HOST_PTRS * size_of::<usize>();
    let actual = size_of::<FfiRenderSurfaceHost<'static>>();
    assert_eq!(
        actual, expected,
        "FfiRenderSurfaceHost size changed: {expected} → {actual} \
         bytes. The host wrapper holds a fat pointer to a \
         ChromeSurface trait object. A size change implies the trait \
         gained or lost a method (vtable widens) or the wrapper grew \
         an additional field. Both are ABI-breaking; update this \
         baseline only when the change is intentional and the \
         modules' FFI version has been bumped."
    );
}
