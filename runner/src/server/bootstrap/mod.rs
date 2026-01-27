//! Server bootstrap - session and registry initialization.
//!
//! This module contains the "generation position" functions that wire up
//! modules, registries, and session defaults.
//!
//! # Epic #417 Part 3: Clean Architecture
//!
//! All built-in modules now come from the `defaults` bundle. The runner no
//! longer imports specific module types directly - modules self-register
//! their resources during `init()` via `ServiceRegistry`.

mod kernel;
mod registries;
mod session;

pub use {
    kernel::real_kernel_context_with_options,
    registries::{build_default_registries, build_empty_session_registry, handle_empty_session},
    session::create_session_with_defaults,
};
