#![allow(unsafe_code)]

use reovim_kernel_interface::declaration::ValueLimits;

// SAFETY: Construction retains only Rust capabilities and performs no calls.
// Starting stages the provider; the SDK owns native cleanup. The host keeps
// code and borrowed contexts through Closed and final dependent destruction.
reovim_sdk::export_module! {
    unsafe {
        implementation: reovim_module_counter::implementation(),
        id: [0xa1; 16],
        name: b"counter",
        revision: 1,
        scopes: 1,
        dependencies: [],
        required: [b"value"],
        export_limit: 1,
        limits: ValueLimits { bytes: 4096, elements: 256, references: 8 },
        views: 16,
    }
}
