//! Product-facing `uapi` namespace facade.
//!
//! This crate gives upper layers a stable `reovim_uapi::{net, sched, ...}`
//! spelling while the physical `reovim-uapi-*` leaf crates remain the
//! dependency-graph units and source-of-truth surfaces. There is intentionally
//! no POSIX module in the public up-face: product semantics live in domain uapi
//! leaves, while provider-facing POSIX-shaped scalars live in `kabi/platform`.

#![no_std]

/// Module/plugin ABI surface.
pub mod abi {
    pub use reovim_uapi_abi::*;
}

/// Product-facing diagnostic dump control vocabulary.
pub mod dump {
    pub use reovim_uapi_dump::*;
}

/// Product-facing filesystem and borrowed-fd vocabulary.
pub mod fs {
    pub use reovim_uapi_fs::*;
}

/// Product-facing log sink vocabulary.
pub mod log {
    pub use reovim_uapi_log::*;
}

/// Product-facing memory-management vocabulary.
pub mod mm {
    pub use reovim_uapi_mm::*;
}

/// Product-facing local networking and IPC vocabulary.
pub mod net {
    pub use reovim_uapi_net::*;
}

/// Product-facing panic policy vocabulary.
pub mod panic {
    pub use reovim_uapi_panic::*;
}

/// Product-facing process syscall vocabulary.
pub mod process {
    pub use reovim_uapi_process::*;
}

/// Framed client/server protocol surface.
pub mod protocol {
    pub use reovim_uapi_protocol::*;
}

/// Product-facing scheduler, clock, and synchronization vocabulary.
pub mod sched {
    pub use reovim_uapi_sched::*;
}

/// Product-facing shell session vocabulary.
pub mod session {
    pub use reovim_uapi_session::*;
}

/// Product-facing service lifecycle vocabulary.
pub mod service {
    pub use reovim_uapi_service::*;
}

/// Product-facing executable source-install vocabulary.
pub mod source {
    pub use reovim_uapi_source::*;
}

/// Raw syscall transport spine.
///
/// Domain leaves own semantic APIs; this module only exposes numbering,
/// argument packing, return/error decoding, and backend hook types used beneath
/// those wrappers.
pub mod syscall {
    pub use reovim_uapi_syscall::*;
}

/// Product-facing system boot facts and device inventory.
pub mod system {
    pub use reovim_uapi_system::*;
}

/// Product-facing terminal and raw-mode vocabulary.
pub mod terminal {
    pub use reovim_uapi_terminal::*;
}
