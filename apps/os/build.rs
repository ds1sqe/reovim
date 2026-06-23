//! Build script: per-target link shaping for the `reovim-os` image crates.
//!
//! Hosted targets use `-nostartfiles` so the floor `_start` is the ELF entry.
//! Bare-metal targets wire explicit linker scripts for stack/sections so the
//! floor boot arm can find symbols like `__stack_top` and `__bss_start`.

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
    println!("cargo::rerun-if-env-changed=REOVIM_OS_BOOTLINE");
    println!("cargo::rerun-if-env-changed=REOVIM_OS_PROFILE");
}
