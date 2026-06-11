//! Build script: link the fixture bin without the C runtime startup object.
//!
//! The fixture owns its process entry via the arch `_start` symbol, so crt0
//! (which provides its own `_start` and calls `main`) must not be linked, or
//! the two `_start` symbols collide. `-nostartfiles` omits crt0 and makes the
//! arch `_start` the ELF entry point. A build-script link arg is used (rather
//! than `.cargo/config.toml` rustflags) because it survives a `RUSTFLAGS`
//! override — e.g. the Phase 5 coverage run that sets `-C instrument-coverage`.
//!
//! `-nostartfiles` is a cc-driver flag: it suppresses the driver's implicit
//! crt0. When the configured linker is `rust-lld` (direct invocation, e.g.
//! the cross-built aarch64 fixtures), no driver and no startfiles are in
//! play and `rust-lld` rejects the flag, so it is emitted only for driver
//! links.

fn main() {
    let linker = std::env::var("RUSTC_LINKER").unwrap_or_default();
    if !linker.contains("rust-lld") {
        println!("cargo::rustc-link-arg-bins=-nostartfiles");
    }
    println!("cargo::rerun-if-env-changed=RUSTC_LINKER");
}
