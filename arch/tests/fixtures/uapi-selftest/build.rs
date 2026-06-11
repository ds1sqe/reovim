//! Build script: link the uapi selftest runner bin without the C runtime
//! startup object, so the arch `_start` is the ELF entry point (#786 Phase 5).
//!
//! A build-script link arg is used rather than `.cargo/config.toml` rustflags
//! because it survives a `RUSTFLAGS` override (the coverage instrumentation
//! run in `scripts/coverage-fixtures.sh`).

fn main() {
    println!("cargo::rustc-link-arg-bins=-nostartfiles");
}
