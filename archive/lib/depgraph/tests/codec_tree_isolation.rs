//! Enforces codec concern-tree isolation.
//!
//! The four codec concerns (input, content, surface, render) each own a
//! separate ext tree rooted at `ext/<concern>-codec/`. A crate in tree
//! A must not take a production dependency on any crate in tree B when
//! `B != A`: cross-concern coupling is a design smell that would make
//! e.g. a TUI input codec drag in a PDF content codec.
//!
//! Intra-concern deps are fine — variant crates sharing a tree (e.g.
//! `ext/content-codec/cjk` → `ext/content-codec/text`) are legitimate
//! shared infrastructure within a single concern.
//!
//! Tree membership is determined by manifest path prefix, NOT by
//! crate-name substring. This prevents misclassifying
//! `reovim-content-codec` (the uapi crate living at `uapi/content-codec/`)
//! as a member of the ext content-codec tree.

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::{collections::HashMap, path::Path},
};

const CONCERNS: &[&str] = &["input", "content", "surface", "render"];

fn concern_of_path(rel: &str) -> Option<&'static str> {
    for c in CONCERNS {
        let tree = format!("ext/{c}-codec/");
        if rel.starts_with(&tree) && rel.len() > tree.len() {
            return Some(*c);
        }
    }
    None
}

#[test]
fn codec_trees_have_no_cross_concern_deps() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();

    let mut concern_of: HashMap<String, &'static str> = HashMap::new();
    for pkg in &metadata.packages {
        let manifest = pkg.manifest_path.as_std_path();
        let Ok(rel) = manifest.strip_prefix(workspace_root) else {
            continue;
        };
        let dir = rel.parent().expect("manifest has parent");
        let rel_str = dir.to_string_lossy().replace('\\', "/");
        if let Some(c) = concern_of_path(&rel_str) {
            concern_of.insert(pkg.name.to_string(), c);
        }
    }

    assert!(
        !concern_of.is_empty(),
        "no ext/<concern>-codec/<variant>/ crates found — did the codec trees get relocated?",
    );

    let mut violations: Vec<String> = Vec::new();

    for pkg in &metadata.packages {
        let Some(origin) = concern_of.get(pkg.name.as_ref()) else {
            continue;
        };

        for dep in &pkg.dependencies {
            if dep.kind != DependencyKind::Normal {
                continue;
            }
            let Some(target) = concern_of.get(dep.name.as_str()) else {
                continue;
            };
            if origin != target {
                violations.push(format!(
                    "{} (tree: {origin}) -> {} (tree: {target})",
                    pkg.name, dep.name,
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Codec tree isolation violations (cross-concern production deps between ext/<concern>-codec/ trees):\n  {}\n\
         \n\
         Concern trees (input / content / surface / render) must not take production\n\
         deps on each other. Move shared logic up to uapi/ or a cross-cutting lib/ crate.",
        violations.join("\n  "),
    );
}
