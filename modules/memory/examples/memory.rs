#![allow(unsafe_code)]
//! Native library entry for the memory module, built as a Cargo cdylib example.
//! Activation publishes `create`; invoking it stages readonly memory access.

use reovim_kernel_interface::declaration::ValueLimits;

// SAFETY: Construction retains only Rust capabilities and performs no calls.
// Starting stages the provider; the SDK owns native cleanup. The host keeps
// code and borrowed contexts through Closed and final dependent destruction.
reovim_sdk::export_module! {
    unsafe {
        implementation: reovim_module_memory::implementation(),
        id: [0xa2; 16],
        name: b"memory",
        revision: 1,
        scopes: 1, // Global activation.
        dependencies: [],
        required: [b"create"], // Exports required before activation is Ready.
        export_limit: 1,
        limits: ValueLimits { bytes: 8192, elements: 256, references: 8 },
        views: 16, // Maximum outstanding imported Caller delivery views.
    }
}
