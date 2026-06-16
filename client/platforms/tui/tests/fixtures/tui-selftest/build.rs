//! Build script: link the test-runner bin without the C runtime startup
//! object, so the arch `_start` is the ELF entry point. A build-script link
//! arg is used because it survives a `RUSTFLAGS` override (the coverage run).
//! Emitted only for cc-driver links: `rust-lld` (direct invocation) links no
//! startfiles and rejects the driver-level flag.

fn main() {
    let linker = std::env::var("RUSTC_LINKER").unwrap_or_default();
    if !linker.contains("rust-lld") {
        println!("cargo::rustc-link-arg-bins=-nostartfiles");
    }
    println!("cargo::rerun-if-env-changed=RUSTC_LINKER");
}
