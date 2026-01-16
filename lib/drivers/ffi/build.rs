//! Build script for reovim-driver-ffi.
//!
//! The C header file `include/reovim.h` is manually maintained because:
//! 1. cbindgen cannot access types from dependent crates (reovim-kernel)
//! 2. We need precise control over the C API documentation
//! 3. The header includes types not defined in this crate (`ModuleProbe`, `Version`)
//!
//! This build script exists to:
//! - Track source file changes for rebuild
//! - Potentially validate the header in the future

fn main() {
    // Track source changes for rebuild
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/types.rs");
    println!("cargo:rerun-if-changed=src/logging.rs");
    println!("cargo:rerun-if-changed=src/version.rs");
    println!("cargo:rerun-if-changed=include/reovim.h");
}
