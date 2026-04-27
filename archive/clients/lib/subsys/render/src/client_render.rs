//! `ClientRender` driver trait.
//!
//! A client-render driver owns a platform-specific render pipeline and
//! hands out `RenderTarget` handles to modules that submit encoded
//! frames. The trait is exposed at the subsys tier so
//! `uapi/driver-macros/` can generate FFI trampolines against it.

use {crate::target::RenderTarget, std::ffi::c_void};

/// Driver error reported at the FFI boundary.
///
/// At the boundary this is converted to `i32` + `*mut *mut c_char`
/// out-param. The macro-generated trampolines allocate the error string
/// via `CString::into_raw` on the driver side; the host reads it and
/// returns it through the vtable's `destroy_error_string` slot.
#[derive(Debug, Clone)]
pub struct ClientRenderError(pub String);

impl std::fmt::Display for ClientRenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ClientRenderError {}

/// Fixed-size probe metadata published by a client-render driver.
///
/// Read by the host during the scan phase before any driver code is
/// allowed to run. Fields are byte arrays instead of pointers so the
/// probe can live inside the static vtable as plain `#[repr(C)]` data.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ClientRenderDriverProbe {
    /// Short kind identifier (null-terminated, max 63 bytes + nul).
    pub kind: [u8; 64],
    /// Human-readable name (null-terminated, max 127 bytes + nul).
    pub name: [u8; 128],
}

impl ClientRenderDriverProbe {
    /// Build a probe from static strings. Truncates inputs that exceed
    /// the field buffers. `const fn` so this can appear in a `#[no_mangle]
    /// pub static` vtable initializer.
    #[must_use]
    pub const fn new(kind: &str, name: &str) -> Self {
        let mut out = Self {
            kind: [0; 64],
            name: [0; 128],
        };
        let kind_bytes = kind.as_bytes();
        let kind_len = if kind_bytes.len() < 63 {
            kind_bytes.len()
        } else {
            63
        };
        let mut i = 0;
        while i < kind_len {
            out.kind[i] = kind_bytes[i];
            i += 1;
        }
        let name_bytes = name.as_bytes();
        let name_len = if name_bytes.len() < 127 {
            name_bytes.len()
        } else {
            127
        };
        i = 0;
        while i < name_len {
            out.name[i] = name_bytes[i];
            i += 1;
        }
        out
    }
}

/// A loadable client-render driver.
///
/// Implementations are produced by driver cdylibs. The host loads the
/// cdylib, reads `REOVIM_CLIENT_RENDER_DRIVER_VTABLE`, validates the
/// vtable header, calls `construct`, then drives the driver's
/// `RenderTarget` sub-handles to submit frames.
pub trait ClientRender: Send + Sync {
    /// Metadata surfaced before any driver code runs.
    fn probe() -> ClientRenderDriverProbe
    where
        Self: Sized;

    /// Instantiate the driver.
    ///
    /// The `platform` pointer is the host's platform handle, reserved
    /// for Phase 2's `FfiPlatformCaps` tightening (see master plan §O7).
    /// In Phase 0 the host passes `std::ptr::null_mut()` and drivers
    /// ignore the value.
    ///
    /// # Errors
    ///
    /// Returns `ClientRenderError` if construction fails. The error
    /// string is reported back to the host through the vtable's
    /// `construct` out-param.
    fn construct(platform: *mut c_void) -> Result<Self, ClientRenderError>
    where
        Self: Sized;

    /// Borrow the driver's render target for frame submission.
    ///
    /// The returned reference borrows `&mut self` so callers cannot
    /// hold two simultaneous targets from the same driver instance.
    fn target(&mut self) -> &mut dyn RenderTarget;

    /// Drain outstanding work and prepare for destruction.
    ///
    /// # Errors
    ///
    /// Returns `ClientRenderError` if shutdown fails.
    fn shutdown(&mut self) -> Result<(), ClientRenderError>;
}

#[cfg(test)]
#[path = "client_render_tests.rs"]
mod tests;
