//! Build script: link the fixture bin without the C runtime startup object.
//!
//! The fixture owns its process entry via the arch `_start` symbol, so crt0
//! (which provides its own `_start` and calls `main`) must not be linked, or
//! the two `_start` symbols collide. `-nostartfiles` omits crt0 and makes the
//! arch `_start` the ELF entry point. A build-script link arg is used (rather
//! than `.cargo/config.toml` rustflags) because it survives a `RUSTFLAGS`
//! override — e.g. the Phase 5 coverage run that sets `-C instrument-coverage`.

fn main() {
    println!("cargo::rustc-link-arg-bins=-nostartfiles");
}
