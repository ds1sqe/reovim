//! `reovim-uapi-module-macros` — L9-clean module vtable export macros.
//!
//! Exports [`declare_module!`] (server-side) and [`declare_module_client!`]
//! (client-side) `macro_rules!` macros that emit the `#[unsafe(no_mangle)]`
//! vtable symbol with a correctly-populated
//! [`VtableHeader`][`::reovim_uapi_abi::VtableHeader`] (AB3/AB12, 6.2 §2).
//!
//! **NOT `proc-macro = true`.** This is an ordinary `#![no_std]` lib crate
//! exporting `macro_rules!` macros. `syn`/`quote`/`proc-macro2` are
//! forbidden by L9/DAG5.
//!
//! **AB12 (no `catch_unwind`).** Under DAG6 the workspace builds
//! `panic = "abort"`; there is no unwinder. An exported vtable slot that
//! panics reaches the `arch/`-owned panic handler directly. The macros do
//! NOT wrap any slot body in `catch_unwind`.
//!
//! **Textual-path discipline.** The macros emit `::reovim_uapi_abi::*`
//! paths as tokens. The macro crate itself has zero deps (DAG5/L9); the
//! **caller's** crate must supply `reovim-uapi-abi` in its own dep graph.
#![no_std]

/// Exports a server-side module vtable with the canonical ABI symbol name.
///
/// # Grammar
///
/// ```text
/// declare_module!(VtableType, vtable_init_expr)
/// ```
///
/// | Argument | Description |
/// |---|---|
/// | `VtableType` | Caller-defined vtable struct; must begin with `VtableHeader` (AB3). |
/// | `vtable_init_expr` | `const` initialiser for the full vtable. |
///
/// The `VtableHeader` embedded in `vtable_init_expr` must have:
/// - `kind = ManifestKind::ModuleServer` (6.2 §2.1 load rule 4)
/// - `size_of_self = core::mem::size_of::<VtableType>()` (AB3 guard)
///
/// These requirements are documented rather than macro-enforced because
/// `macro_rules!` cannot access struct fields inside an arbitrary `$init`
/// expression. The caller owns layout correctness; the layout golden tests
/// (Phase 3) catch systematic drift.
///
/// # Emits
///
/// ```rust,ignore
/// #[allow(unsafe_code)]     // #[unsafe(no_mangle)] is the FFI export seam
/// #[unsafe(no_mangle)]
/// pub static REOVIM_MODULE_SERVER_VTABLE: VtableType = vtable_init_expr;
/// ```
///
/// Symbol name: `REOVIM_MODULE_SERVER_VTABLE` per 6.2 §2 / `ManifestKind` 1.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_abi::{
///     ids::{AbiVersion, Version},
///     vtable::{ManifestKind, VtableHeader},
/// };
/// use core::mem;
///
/// // Caller-defined vtable. Must begin with VtableHeader (AB3).
/// #[repr(C)]
/// struct MyModuleVtable {
///     pub header: VtableHeader,
/// }
///
/// const MY_VTABLE: MyModuleVtable = MyModuleVtable {
///     header: VtableHeader {
///         abi:          AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 },
///         api:          Version    { major: 0, minor: 1, patch: 0, pad: 0 },
///         size_of_self: mem::size_of::<MyModuleVtable>(),
///         kind:         ManifestKind::ModuleServer,
///         flags:        0,
///     },
/// };
///
/// reovim_uapi_module_macros::declare_module!(MyModuleVtable, MY_VTABLE);
/// ```
#[macro_export]
macro_rules! declare_module {
    ($vtable_type:ty, $init:expr) => {
        // AB12: no catch_unwind. Under DAG6 (panic = "abort") there is no
        // unwinder; panics in vtable slots reach the arch panic handler directly.
        //
        // The #[allow(unsafe_code)] suppresses the workspace `unsafe_code = warn`
        // lint for this expansion only. Justified: #[unsafe(no_mangle)] is the
        // FFI export seam described in 6.2 §2. It is the one place in uapi that
        // must carry an unsafe attribute by design.
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        /// Server-side module vtable export (6.2 §2, `REOVIM_MODULE_SERVER_VTABLE`).
        ///
        /// Carries a [`VtableHeader`][::reovim_uapi_abi::vtable::VtableHeader] at
        /// offset 0 per the 6.2 §2.1 load rules (AB3). The loader reads the
        /// header before calling any function pointer.
        pub static REOVIM_MODULE_SERVER_VTABLE: $vtable_type = $init;
    };
}

/// Exports a client-side module vtable with the canonical ABI symbol name.
///
/// # Grammar
///
/// ```text
/// declare_module_client!(VtableType, vtable_init_expr)
/// ```
///
/// | Argument | Description |
/// |---|---|
/// | `VtableType` | Caller-defined vtable struct; must begin with `VtableHeader` (AB3). |
/// | `vtable_init_expr` | `const` initialiser for the full vtable. |
///
/// The `VtableHeader` embedded in `vtable_init_expr` must have:
/// - `kind = ManifestKind::ModuleClient` (6.2 §2.1 load rule 4)
/// - `size_of_self = core::mem::size_of::<VtableType>()` (AB3 guard)
///
/// # Emits
///
/// ```rust,ignore
/// #[allow(unsafe_code)]
/// #[unsafe(no_mangle)]
/// pub static REOVIM_MODULE_CLIENT_VTABLE: VtableType = vtable_init_expr;
/// ```
///
/// Symbol name: `REOVIM_MODULE_CLIENT_VTABLE` per 6.2 §2 / `ManifestKind` 3.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_abi::{
///     ids::{AbiVersion, Version},
///     vtable::{ManifestKind, VtableHeader},
/// };
/// use core::mem;
///
/// #[repr(C)]
/// struct MyClientModuleVtable {
///     pub header: VtableHeader,
/// }
///
/// const MY_VTABLE: MyClientModuleVtable = MyClientModuleVtable {
///     header: VtableHeader {
///         abi:          AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 },
///         api:          Version    { major: 0, minor: 1, patch: 0, pad: 0 },
///         size_of_self: mem::size_of::<MyClientModuleVtable>(),
///         kind:         ManifestKind::ModuleClient,
///         flags:        0,
///     },
/// };
///
/// reovim_uapi_module_macros::declare_module_client!(MyClientModuleVtable, MY_VTABLE);
/// ```
#[macro_export]
macro_rules! declare_module_client {
    ($vtable_type:ty, $init:expr) => {
        // AB12: no catch_unwind. See declare_module! for the full rationale.
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        /// Client-side module vtable export (6.2 §2, `REOVIM_MODULE_CLIENT_VTABLE`).
        pub static REOVIM_MODULE_CLIENT_VTABLE: $vtable_type = $init;
    };
}
