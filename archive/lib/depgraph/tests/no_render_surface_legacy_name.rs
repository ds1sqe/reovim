//! Tripwire: prevents re-introduction of the legacy `RenderSurface` trait
//! name anywhere in workspace Rust sources.
//!
//! The trait was renamed to `ChromeSurface`. This probe scans every
//! `*.rs` file under the workspace (excluding `archive/` and `target/`)
//! for the structural shapes that would reintroduce the old name:
//!
//! - `trait RenderSurface` — redefining the deleted trait.
//! - `dyn RenderSurface` — a trait-object bound referencing the old name.
//! - `use …::RenderSurface` — importing the old name.
//!
//! Scope decisions:
//!
//! - **TypeScript sources are excluded** (`clients/web/**/*.ts`). The web
//!   client has its own `RenderSurface` TypeScript interface that is
//!   renamed as part of the 17-δ web-client reconstruction work — a
//!   distinct flight. Excluding `.ts` here prevents this Rust-focused
//!   probe from blocking that unrelated effort.
//!
//! - **`archive/` and CHANGELOG history are excluded** — those preserve
//!   historical references to the old trait name for provenance; the
//!   probe only walks live Rust sources.
//!
//! - **`FfiRenderSurfaceHost` / `FfiRenderSurfaceRef` struct names are
//!   NOT matched.** The probe's regex targets the structural uses
//!   (`trait`, `dyn`, `use`-path), not struct-name substrings. Those
//!   FFI struct-name renames are a Phase D polish item deferred to a
//!   later flight.

use std::{fs, path::PathBuf};

use {cargo_metadata::MetadataCommand, walkdir::WalkDir};

fn workspace_root() -> PathBuf {
    MetadataCommand::new()
        .exec()
        .expect("cargo metadata")
        .workspace_root
        .into()
}

fn is_legacy_render_surface(line: &str) -> bool {
    // Trait definition: `trait RenderSurface` (optional pub, optional generics).
    if let Some(idx) = line.find("trait RenderSurface") {
        let after = &line[idx + "trait RenderSurface".len()..];
        if after
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric() && c != '_')
        {
            return true;
        }
    }
    // Trait-object bound: `dyn RenderSurface`.
    if let Some(idx) = line.find("dyn RenderSurface") {
        let after = &line[idx + "dyn RenderSurface".len()..];
        if after
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric() && c != '_')
        {
            return true;
        }
    }
    // Import path ending in `::RenderSurface` — `use …::RenderSurface` or
    // `use …::{…, RenderSurface, …}`.
    if line.contains("use ") && line.contains("RenderSurface") {
        // Reject false positives on Ffi* struct names like FfiRenderSurfaceHost.
        let around = line;
        // Match when the token `RenderSurface` appears with a non-word char
        // (or start) before it AND a non-word char (or end) after it.
        let bytes = around.as_bytes();
        let needle = b"RenderSurface";
        let mut i = 0;
        while i + needle.len() <= bytes.len() {
            if &bytes[i..i + needle.len()] == needle {
                let left_ok =
                    i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
                let right = i + needle.len();
                let right_ok = right == bytes.len()
                    || (!bytes[right].is_ascii_alphanumeric() && bytes[right] != b'_');
                if left_ok && right_ok {
                    return true;
                }
                i += needle.len();
            } else {
                i += 1;
            }
        }
    }
    false
}

#[test]
fn no_legacy_render_surface_name_in_workspace_rs() {
    let root = workspace_root();

    let mut offenders: Vec<(PathBuf, usize, String)> = Vec::new();

    for entry in WalkDir::new(&root).follow_links(false) {
        let Ok(entry) = entry else { continue };

        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        // Skip archive/target.
        let rel = path.strip_prefix(&root).unwrap_or(path);
        let rel_str = rel.to_string_lossy();
        if rel_str.starts_with("archive/") || rel_str.starts_with("target/") {
            continue;
        }
        // Skip this probe itself (its doc comment names the legacy token).
        if rel_str.ends_with("no_render_surface_legacy_name.rs") {
            continue;
        }

        let Ok(src) = fs::read_to_string(path) else {
            continue;
        };
        for (idx, line) in src.lines().enumerate() {
            if is_legacy_render_surface(line) {
                offenders.push((rel.to_path_buf(), idx + 1, line.trim().to_string()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "Legacy `RenderSurface` name re-introduced. Offenders:\n{}",
        offenders
            .iter()
            .map(|(p, l, text)| format!("  {}:{}: {}", p.display(), l, text))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn probe_detects_trait_definition() {
    assert!(is_legacy_render_surface("pub trait RenderSurface {"));
    assert!(is_legacy_render_surface("trait RenderSurface: Send {"));
}

#[test]
fn probe_detects_dyn_bound() {
    assert!(is_legacy_render_surface("    surface: &mut dyn RenderSurface,"));
    assert!(is_legacy_render_surface("fn f(s: Box<dyn RenderSurface>) {}"));
}

#[test]
fn probe_detects_use_import() {
    assert!(is_legacy_render_surface("use reovim_client_driver::RenderSurface;"));
    assert!(is_legacy_render_surface("use crate::{ChromeSurface, RenderSurface};"));
}

#[test]
fn probe_skips_ffi_struct_names() {
    assert!(!is_legacy_render_surface("pub struct FfiRenderSurfaceHost<'a> { ... }"));
    assert!(!is_legacy_render_surface("let mut host = FfiRenderSurfaceHost::new(surface);"));
    assert!(!is_legacy_render_surface(
        "    let mut ffi_surface = FfiRenderSurface::from_host(&mut host);"
    ));
}

#[test]
fn probe_skips_chrome_surface() {
    assert!(!is_legacy_render_surface("pub trait ChromeSurface {"));
    assert!(!is_legacy_render_surface("    surface: &mut dyn ChromeSurface,"));
    assert!(!is_legacy_render_surface("use reovim_client_driver::ChromeSurface;"));
}
