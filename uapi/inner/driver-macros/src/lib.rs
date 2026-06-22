//! `reovim-uapi-driver-macros` — L9-clean driver/capability/domain/provider/stream vtable macros.
//!
//! Exports one macro per non-module `ManifestKind` variant (6.2 §7):
//!
//! | Macro | Symbol | `ManifestKind` |
//! |---|---|---|
//! | [`declare_driver_server!`] | `REOVIM_DRIVER_SERVER_VTABLE` | `DriverServer = 2` |
//! | [`declare_driver_client!`] | `REOVIM_DRIVER_CLIENT_VTABLE` | `DriverClient = 4` |
//! | [`declare_capability_client!`] | `REOVIM_CAPABILITY_CLIENT_VTABLE` | `CapabilityClient = 5` |
//! | [`declare_domain_server!`] | `REOVIM_DOMAIN_SERVER_VTABLE` | `DomainServer = 6` |
//! | [`declare_provider_server!`] | `REOVIM_PROVIDER_SERVER_VTABLE` | `ProviderServer = 7` |
//! | [`declare_stream_scheme!`] | `REOVIM_STREAM_SCHEME_VTABLE` | `StreamScheme = 8` |
//!
//! The `ModuleServer` and `ModuleClient` kinds are handled by
//! `reovim-uapi-module-macros`.
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
//! **Textual-path discipline.** Macros emit `::reovim_uapi_abi::*` paths as
//! tokens. The macro crate itself has zero deps (DAG5/L9); the **caller's**
//! crate must supply `reovim-uapi-abi` in its dep graph.
#![no_std]

/// Exports a server-side driver vtable with the canonical ABI symbol name.
///
/// # Grammar
///
/// ```text
/// declare_driver_server!(VtableType, vtable_init_expr)
/// ```
///
/// | Argument | Description |
/// |---|---|
/// | `VtableType` | Caller-defined vtable struct; must begin with `VtableHeader` (AB3). |
/// | `vtable_init_expr` | `const` initialiser for the full vtable. |
///
/// The `VtableHeader` embedded in `vtable_init_expr` must have:
/// - `kind = ManifestKind::DriverServer`
/// - `size_of_self = core::mem::size_of::<VtableType>()`
///
/// # Emits
///
/// ```rust,ignore
/// #[allow(unsafe_code)]
/// #[unsafe(no_mangle)]
/// pub static REOVIM_DRIVER_SERVER_VTABLE: VtableType = vtable_init_expr;
/// ```
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
/// struct MyServerDriverVtable {
///     pub header: VtableHeader,
/// }
///
/// const MY_VTABLE: MyServerDriverVtable = MyServerDriverVtable {
///     header: VtableHeader {
///         abi:          AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 },
///         api:          Version    { major: 0, minor: 1, patch: 0, pad: 0 },
///         size_of_self: mem::size_of::<MyServerDriverVtable>(),
///         kind:         ManifestKind::DriverServer,
///         flags:        0,
///     },
/// };
///
/// reovim_uapi_driver_macros::declare_driver_server!(MyServerDriverVtable, MY_VTABLE);
/// ```
#[macro_export]
macro_rules! declare_driver_server {
    ($vtable_type:ty, $init:expr) => {
        // AB12: no catch_unwind. Under DAG6 (panic = "abort") there is no
        // unwinder; panics in vtable slots reach the arch panic handler directly.
        //
        // The #[allow(unsafe_code)] suppresses the workspace `unsafe_code = warn`
        // lint for this expansion only. Justified: #[unsafe(no_mangle)] is the
        // FFI export seam described in 6.2 §2.
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        /// Server-side driver vtable export (6.2 §2, `REOVIM_DRIVER_SERVER_VTABLE`).
        pub static REOVIM_DRIVER_SERVER_VTABLE: $vtable_type = $init;
    };
}

/// Exports a client-side driver vtable with the canonical ABI symbol name.
///
/// # Grammar
///
/// ```text
/// declare_driver_client!(VtableType, vtable_init_expr)
/// ```
///
/// The `VtableHeader.kind` must be `ManifestKind::DriverClient`.
///
/// # Emits
///
/// `pub static REOVIM_DRIVER_CLIENT_VTABLE: VtableType = vtable_init_expr`
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
/// struct MyClientDriverVtable {
///     pub header: VtableHeader,
/// }
///
/// const MY_VTABLE: MyClientDriverVtable = MyClientDriverVtable {
///     header: VtableHeader {
///         abi:          AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 },
///         api:          Version    { major: 0, minor: 1, patch: 0, pad: 0 },
///         size_of_self: mem::size_of::<MyClientDriverVtable>(),
///         kind:         ManifestKind::DriverClient,
///         flags:        0,
///     },
/// };
///
/// reovim_uapi_driver_macros::declare_driver_client!(MyClientDriverVtable, MY_VTABLE);
/// ```
#[macro_export]
macro_rules! declare_driver_client {
    ($vtable_type:ty, $init:expr) => {
        // AB12: no catch_unwind. See declare_driver_server! for the full rationale.
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        /// Client-side driver vtable export (6.2 §2, `REOVIM_DRIVER_CLIENT_VTABLE`).
        pub static REOVIM_DRIVER_CLIENT_VTABLE: $vtable_type = $init;
    };
}

/// Exports a client-side capability vtable with the canonical ABI symbol name.
///
/// # Grammar
///
/// ```text
/// declare_capability_client!(VtableType, vtable_init_expr)
/// ```
///
/// The `VtableHeader.kind` must be `ManifestKind::CapabilityClient`.
///
/// # Emits
///
/// `pub static REOVIM_CAPABILITY_CLIENT_VTABLE: VtableType = vtable_init_expr`
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
/// struct MyCapabilityVtable {
///     pub header: VtableHeader,
/// }
///
/// const MY_VTABLE: MyCapabilityVtable = MyCapabilityVtable {
///     header: VtableHeader {
///         abi:          AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 },
///         api:          Version    { major: 0, minor: 1, patch: 0, pad: 0 },
///         size_of_self: mem::size_of::<MyCapabilityVtable>(),
///         kind:         ManifestKind::CapabilityClient,
///         flags:        0,
///     },
/// };
///
/// reovim_uapi_driver_macros::declare_capability_client!(MyCapabilityVtable, MY_VTABLE);
/// ```
#[macro_export]
macro_rules! declare_capability_client {
    ($vtable_type:ty, $init:expr) => {
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        /// Client-side capability vtable export (6.2 §2, `REOVIM_CAPABILITY_CLIENT_VTABLE`).
        pub static REOVIM_CAPABILITY_CLIENT_VTABLE: $vtable_type = $init;
    };
}

/// Exports a server-side domain vtable with the canonical ABI symbol name.
///
/// # Grammar
///
/// ```text
/// declare_domain_server!(VtableType, vtable_init_expr)
/// ```
///
/// The `VtableHeader.kind` must be `ManifestKind::DomainServer`.
///
/// # Emits
///
/// `pub static REOVIM_DOMAIN_SERVER_VTABLE: VtableType = vtable_init_expr`
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
/// struct MyDomainVtable {
///     pub header: VtableHeader,
/// }
///
/// const MY_VTABLE: MyDomainVtable = MyDomainVtable {
///     header: VtableHeader {
///         abi:          AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 },
///         api:          Version    { major: 0, minor: 1, patch: 0, pad: 0 },
///         size_of_self: mem::size_of::<MyDomainVtable>(),
///         kind:         ManifestKind::DomainServer,
///         flags:        0,
///     },
/// };
///
/// reovim_uapi_driver_macros::declare_domain_server!(MyDomainVtable, MY_VTABLE);
/// ```
#[macro_export]
macro_rules! declare_domain_server {
    ($vtable_type:ty, $init:expr) => {
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        /// Server-side domain vtable export (6.2 §2, `REOVIM_DOMAIN_SERVER_VTABLE`).
        pub static REOVIM_DOMAIN_SERVER_VTABLE: $vtable_type = $init;
    };
}

/// Exports a server-side provider vtable with the canonical ABI symbol name.
///
/// # Grammar
///
/// ```text
/// declare_provider_server!(VtableType, vtable_init_expr)
/// ```
///
/// The `VtableHeader.kind` must be `ManifestKind::ProviderServer`.
///
/// # Emits
///
/// `pub static REOVIM_PROVIDER_SERVER_VTABLE: VtableType = vtable_init_expr`
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
/// struct MyProviderVtable {
///     pub header: VtableHeader,
/// }
///
/// const MY_VTABLE: MyProviderVtable = MyProviderVtable {
///     header: VtableHeader {
///         abi:          AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 },
///         api:          Version    { major: 0, minor: 1, patch: 0, pad: 0 },
///         size_of_self: mem::size_of::<MyProviderVtable>(),
///         kind:         ManifestKind::ProviderServer,
///         flags:        0,
///     },
/// };
///
/// reovim_uapi_driver_macros::declare_provider_server!(MyProviderVtable, MY_VTABLE);
/// ```
#[macro_export]
macro_rules! declare_provider_server {
    ($vtable_type:ty, $init:expr) => {
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        /// Server-side provider vtable export (6.2 §2, `REOVIM_PROVIDER_SERVER_VTABLE`).
        pub static REOVIM_PROVIDER_SERVER_VTABLE: $vtable_type = $init;
    };
}

/// Exports a stream-scheme vtable with the canonical ABI symbol name.
///
/// # Grammar
///
/// ```text
/// declare_stream_scheme!(VtableType, vtable_init_expr)
/// ```
///
/// The `VtableHeader.kind` must be `ManifestKind::StreamScheme`.
///
/// # Emits
///
/// `pub static REOVIM_STREAM_SCHEME_VTABLE: VtableType = vtable_init_expr`
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
/// struct MyStreamSchemeVtable {
///     pub header: VtableHeader,
/// }
///
/// const MY_VTABLE: MyStreamSchemeVtable = MyStreamSchemeVtable {
///     header: VtableHeader {
///         abi:          AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 },
///         api:          Version    { major: 0, minor: 1, patch: 0, pad: 0 },
///         size_of_self: mem::size_of::<MyStreamSchemeVtable>(),
///         kind:         ManifestKind::StreamScheme,
///         flags:        0,
///     },
/// };
///
/// reovim_uapi_driver_macros::declare_stream_scheme!(MyStreamSchemeVtable, MY_VTABLE);
/// ```
#[macro_export]
macro_rules! declare_stream_scheme {
    ($vtable_type:ty, $init:expr) => {
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        /// Stream-scheme vtable export (6.2 §2, `REOVIM_STREAM_SCHEME_VTABLE`).
        pub static REOVIM_STREAM_SCHEME_VTABLE: $vtable_type = $init;
    };
}
