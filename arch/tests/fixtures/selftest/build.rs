//! Build script: per-target link shaping for the test-runner bin.
//!
//! Hosted targets: link without the C runtime startup object, so the arch
//! `_start` is the ELF entry point (see the other arch fixtures' `build.rs`
//! for the full rationale). A build-script link arg is used because it
//! survives a `RUSTFLAGS` override (the coverage run). `-nostartfiles` is a
//! cc-driver flag, so it is emitted only for driver links: `rust-lld`
//! (direct invocation) and freestanding targets (no driver, no startfiles)
//! both reject it.
//!
//! `aarch64-unknown-none`: the bare-metal image needs the fixed kernel8.img
//! layout (load address 0x80000, `.text.boot` first, testrt section
//! brackets, BSS/stack symbols the boot arm consumes), supplied by the
//! package's linker script.

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let linker = std::env::var("RUSTC_LINKER").unwrap_or_default();
    if !linker.contains("rust-lld") && !target.ends_with("-none") {
        println!("cargo::rustc-link-arg-bins=-nostartfiles");
    }
    if target == "aarch64-unknown-none" {
        let dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
        println!("cargo::rustc-link-arg-bins=-T{dir}/link-none-aarch64.ld");
        println!("cargo::rerun-if-changed=link-none-aarch64.ld");
    }
    println!("cargo::rerun-if-env-changed=RUSTC_LINKER");
}
