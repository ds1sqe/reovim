#![no_std]
#![deny(unsafe_code)]
//! Rust adapters for declared operations over the language-independent ABI.
//!
//! Example: composition imports an independently built provider with
//! `import_provider`; Runtime receives only its safe Interface operations.
//! Rust suppliers use `ExportedProvider` for calls, factories and I/O,
//! and `export_pending` for owned results. Native module entry constructors
//! use `export_module!` with their ordinary Rust implementation; the underlying
//! `export_module_instance` owns driver and imported capability cleanup.

extern crate alloc;

#[allow(unsafe_code)]
mod native;

#[allow(unsafe_code)]
mod pending;

#[allow(unsafe_code)]
mod raw;

pub use native::{
    ExportedRegistrar, ModuleImports, NativeModule, import_provider,
    import_state, import_state_init, module_implementation,
};

mod request;
pub use request::Request;

#[allow(unsafe_code)]
mod caller_export;
pub use caller_export::ExportedCaller;

#[allow(unsafe_code)]
mod caller_import;
pub use caller_import::{ImportedCaller, import_caller};

#[allow(unsafe_code)]
mod outcome_export;
#[allow(unsafe_code)]
mod pending_export;
pub use pending_export::export_pending;

#[allow(unsafe_code)]
mod provider_export;
pub use provider_export::ExportedProvider;

#[allow(unsafe_code)]
mod state_export;
pub use state_export::ExportedState;

#[allow(unsafe_code)]
mod registrar_import;
pub use registrar_import::ImportedRegistrar;

#[allow(unsafe_code)]
mod module_export;
pub use module_export::export_module_instance;

mod namespace_value;
