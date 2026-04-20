//! Enforces that every codec variant crate declares the module entry
//! macro dependency.
//!
//! Codec crates follow the three-tier pattern: `uapi/<concern>-codec/`
//! defines the closed Codec trait, `server/lib/subsys/<concern>-codec/`
//! defines the registry contract, and `ext/<concern>-codec/<variant>/`
//! supplies a concrete impl. Variant crates are dynamically loadable
//! modules — their lifetime is bound by the `declare_module!` macro
//! which emits the `REOVIM_MODULE_API_VERSION` FFI symbol and
//! register/unregister hooks. Missing this dep means the crate cannot
//! be loaded as a module, which silently breaks the codec lifetime =
//! module lifetime invariant.

use cargo_metadata::{DependencyKind, MetadataCommand};
use std::path::Path;

const CONCERNS: &[&str] = &["input", "content", "surface", "render"];
const MODULE_MACROS_CRATE: &str = "reovim-module-macros";

#[test]
fn every_codec_variant_depends_on_module_macros() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();

    let mut missing: Vec<String> = Vec::new();
    let mut checked_count = 0usize;

    for pkg in &metadata.packages {
        let manifest = pkg.manifest_path.as_std_path();
        let Ok(rel) = manifest.strip_prefix(workspace_root) else {
            continue;
        };
        let dir = rel.parent().expect("manifest has parent");
        let rel_str = dir.to_string_lossy().replace('\\', "/");

        let is_codec_variant = CONCERNS.iter().any(|c| {
            let tree = format!("ext/{c}-codec/");
            rel_str.starts_with(&tree) && rel_str.len() > tree.len()
        });

        if !is_codec_variant {
            continue;
        }
        checked_count += 1;

        let has_macros = pkg.dependencies.iter().any(|d| {
            d.kind == DependencyKind::Normal && d.name.as_str() == MODULE_MACROS_CRATE
        });

        if !has_macros {
            missing.push(format!("{} @ {rel_str}", pkg.name));
        }
    }

    assert!(
        checked_count > 0,
        "no ext/<concern>-codec/<variant>/ crates found — did the codec trees get relocated?",
    );

    assert!(
        missing.is_empty(),
        "codec variant crates missing production dep on `{MODULE_MACROS_CRATE}`:\n  {}\n\
         \n\
         Every ext/<concern>-codec/<variant>/ crate must declare `{MODULE_MACROS_CRATE}`\n\
         as a production dependency so it can emit the `declare_module!` FFI surface.\n\
         Without it, codec lifetime cannot be bound to module lifetime.",
        missing.join("\n  ")
    );
}
