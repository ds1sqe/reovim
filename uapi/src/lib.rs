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

/// Framed client/server protocol surface.
pub mod protocol {
    pub use reovim_uapi_protocol::*;
}

/// Product-facing scheduler, clock, and synchronization vocabulary.
pub mod sched {
    pub use reovim_uapi_sched::*;
}

/// Product-facing system boot facts and device inventory.
pub mod system {
    pub use reovim_uapi_system::*;
}

/// Product-facing terminal and raw-mode vocabulary.
pub mod terminal {
    pub use reovim_uapi_terminal::*;
}
