//! Re-export the opaque input event envelope from `reovim-input-codec`.
//!
//! The envelope types are owned by `uapi/input-codec/` (the closed UAPI
//! mechanism layer).  This module re-exports them so that consumers that
//! depended on `reovim-subsys-input` can continue to import from here
//! without a direct dependency on the uapi crate.
//!
//! Linux equivalent: `include/linux/input.h` re-exporting `include/uapi/linux/input.h`.

pub use reovim_input_codec::{
    INPUT_HEADER_SIZE, InputEvent, InputFlags, InputPayloadError, input_context, input_flags,
    input_kind,
};
