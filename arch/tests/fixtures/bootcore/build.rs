//! Build script: per-target link shaping for the boot-core bin.
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
//! layout (load address 0x80000, `.text.boot` first, BSS/stack symbols the
//! boot arm consumes), supplied by the package's linker script.
//!
//! `x86_64-unknown-none`: the Multiboot1 image needs its own layout (load
//! address 0x100000, `.multiboot` header first, then `.text.boot`; same
//! BSS/stack symbols), supplied by `link-none-x86_64.ld`.

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let linker = std::env::var("RUSTC_LINKER").unwrap_or_default();
    if !linker.contains("rust-lld") && !target.ends_with("-none") {
        println!("cargo::rustc-link-arg-bins=-nostartfiles");
    }
    let script = match target.as_str() {
        "aarch64-unknown-none" => Some("link-none-aarch64.ld"),
        "x86_64-unknown-none" => Some("link-none-x86_64.ld"),
        _ => None,
    };
    if let Some(script) = script {
        let dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
        println!("cargo::rustc-link-arg-bins=-T{dir}/{script}");
        println!("cargo::rerun-if-changed={script}");
    }
    println!("cargo::rerun-if-env-changed=RUSTC_LINKER");
}
