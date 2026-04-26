//! Static probe metadata for the text-buffer cdylib.
//!
//! Returned by `TextBufferDriverImpl::probe()` and consumed by the
//! cdylib trampoline that the `declare_buffer_driver!` macro emits.
//! The values must match the buffer-subsys ABI contract: `KIND` is the
//! family discriminant ("buffer"), `NAME` is a free-form display label
//! the host surfaces in diagnostics.

/// Family discriminant — must equal the buffer subsys's expected
/// `BufferDriver::kind()` return value.
pub const KIND: &str = "buffer";

/// Human-readable driver name surfaced in host diagnostics.
pub const NAME: &str = "Reovim text buffer driver";
