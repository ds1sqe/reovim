//! Build script: link the test-runner bin without the C runtime startup
//! object, so the arch `_start` is the ELF entry point (see the other arch
//! fixtures' `build.rs` for the full rationale). A build-script link arg is
//! used because it survives a `RUSTFLAGS` override (the coverage run).

fn main() {
    println!("cargo::rustc-link-arg-bins=-nostartfiles");
}
