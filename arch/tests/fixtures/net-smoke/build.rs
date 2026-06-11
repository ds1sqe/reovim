//! Build script: link the net-smoke fixture bin without the C runtime startup
//! object.
//!
//! Mirrors the smoke/build.rs pattern: `-nostartfiles` lets the arch `_start`
//! be the ELF entry point without colliding with crt0's `_start`. Emitted
//! only for driver links (not `rust-lld`) per the same convention.

fn main() {
    let linker = std::env::var("RUSTC_LINKER").unwrap_or_default();
    if !linker.contains("rust-lld") {
        println!("cargo::rustc-link-arg-bins=-nostartfiles");
    }
    println!("cargo::rerun-if-env-changed=RUSTC_LINKER");
}
