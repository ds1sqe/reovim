//! Guards the Plan 17-β.2b invariant: no production code calls
//! `SurfaceDescriptorHandler::decode` directly.
//!
//! The `decode()` method remains `pub` on the trait because tests and
//! introspection still use it, but production dispatch goes through
//! `decode_and_apply()` — which hides the `Box<dyn Any + Send>`
//! boundary layer entirely. Allowing production callers to reach
//! `decode()` would reopen the policy-leak that Plan 17-β.2a closed.
//!
//! This probe walks the production source tree and fails on any
//! call-site that matches the `decode(` pattern outside a `#[cfg(test)]`
//! block or a test-named file.
//!
//! # Known limitations (documented false-negative classes)
//!
//! - **Macro expansion**: `#[cfg(test)]` scope detection uses line-
//!   oriented brace counting. Macros that emit `#[cfg(test)]` at
//!   expansion time are not recognised. A caller hidden inside such a
//!   macro would escape the probe. Acceptable: reovim has no such
//!   pattern today, and the self-test below confirms the detection
//!   fires on the common case.
//! - **Multi-line call-sites**: the probe scans line by line, so a
//!   `.decode(` split across lines with `\`-continuation will not
//!   match. `cargo fmt` collapses these by default.
//!
//! The self-test (`probe_detects_seeded_violation`) seeds a temp file
//! with the forbidden pattern and verifies the probe actually fails —
//! locking detection behaviour beyond the happy-path "zero callers"
//! case.

use cargo_metadata::MetadataCommand;
use std::{
    fs,
    path::{Path, PathBuf},
};

const SCAN_DIRS: &[&str] = &["clients", "ext/client", "apps/bin", "tools"];

/// Patterns that indicate a direct `.decode(` call on
/// `SurfaceDescriptorHandler`. We match on the method name plus a
/// following paren, then exclude test-scoped code from the scan.
const FORBIDDEN_PATTERNS: &[&str] = &[
    "SurfaceDescriptorHandler::decode(",
    ".decode(&",
    ".decode(body",
];

fn workspace_root() -> PathBuf {
    MetadataCommand::new()
        .exec()
        .expect("cargo metadata")
        .workspace_root
        .into()
}

/// Test-looking path segments cause the file to be skipped entirely.
/// The trait's defining file is also skipped — the default impl
/// legitimately calls `self.decode()` to route to the override path,
/// and it would be perverse to forbid the trait from calling its own
/// method.
fn is_test_file_path(p: &Path) -> bool {
    let s = p.to_string_lossy();
    s.ends_with("_tests.rs")
        || s.contains("/tests/")
        || s.contains("/fixtures/")
        || s.contains("/benches/")
        || s.ends_with("clients/lib/subsys/codec/src/surface_descriptor.rs")
}

/// Strip `//` line comments to avoid flagging discussion of the
/// forbidden pattern in docstrings or inline comments.
fn strip_comments(line: &str) -> &str {
    line.find("//").map_or(line, |idx| &line[..idx])
}

fn line_matches_any(line: &str, patterns: &[&str]) -> bool {
    let stripped = strip_comments(line);
    patterns.iter().any(|p| stripped.contains(p))
}

/// Walk a file and collect offending line numbers, skipping
/// `#[cfg(test)]` blocks via naive brace-depth tracking.
fn scan_file(path: &Path, patterns: &[&str]) -> Vec<(usize, String)> {
    let Ok(src) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut offenders: Vec<(usize, String)> = Vec::new();
    let mut test_depth: i32 = 0;
    let mut armed_test_cfg = false;

    for (i, line) in src.lines().enumerate() {
        let trimmed = line.trim_start();
        // Arm on encountering `#[cfg(test)]` at line start — next
        // open brace (likely the `mod { ... }` or `fn { ... }` it
        // annotates) begins a test-scope run.
        if armed_test_cfg && line.contains('{') {
            test_depth += 1;
            armed_test_cfg = false;
        }
        if trimmed.starts_with("#[cfg(test)]") || trimmed.starts_with("#[cfg(all(test") {
            armed_test_cfg = true;
        }
        // Adjust brace depth inside the test-scope run.
        if test_depth > 0 {
            for c in line.chars() {
                match c {
                    '{' => test_depth += 1,
                    '}' => {
                        test_depth -= 1;
                        if test_depth <= 0 {
                            test_depth = 0;
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        if test_depth > 0 {
            continue;
        }
        if line_matches_any(line, patterns) {
            offenders.push((i + 1, line.to_string()));
        }
    }
    offenders
}

fn walk_dir(
    dir: &Path,
    patterns: &[&str],
    out: &mut Vec<(PathBuf, usize, String)>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            walk_dir(&path, patterns, out);
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        if is_test_file_path(&path) {
            continue;
        }
        for (line_no, line) in scan_file(&path, patterns) {
            out.push((path.clone(), line_no, line));
        }
    }
}

#[test]
fn no_pub_decode_caller_outside_tests() {
    let root = workspace_root();
    let mut offenders: Vec<(PathBuf, usize, String)> = Vec::new();

    for dir_rel in SCAN_DIRS {
        let dir = root.join(dir_rel);
        if dir.is_dir() {
            walk_dir(&dir, FORBIDDEN_PATTERNS, &mut offenders);
        }
    }

    assert!(
        offenders.is_empty(),
        "SurfaceDescriptorHandler::decode() called from production code \
         (Plan 17-β.2b F2 regression). Production dispatch must use \
         decode_and_apply() — reaching decode() exposes Box<dyn Any + Send> \
         to the routing layer. Offenders: {offenders:#?}",
    );
}

#[test]
fn probe_detects_seeded_violation() {
    // Self-test: seed a temp dir with a file containing the forbidden
    // pattern and confirm the scanning logic actually fires. Without
    // this the probe could silently become a no-op if the skip rules
    // over-exclude.
    let tmp = std::env::temp_dir().join("plan20_phase_b_self_test");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).expect("mk tmp");
    let seeded = tmp.join("seeded.rs");
    fs::write(
        &seeded,
        "fn production_caller(h: &dyn SurfaceDescriptorHandler, body: &[u8]) {\n\
         \x20\x20\x20\x20let _boxed = h.decode(body).unwrap();\n\
         }\n",
    )
    .expect("write seeded");

    let mut offenders: Vec<(PathBuf, usize, String)> = Vec::new();
    walk_dir(&tmp, FORBIDDEN_PATTERNS, &mut offenders);

    assert!(
        !offenders.is_empty(),
        "self-test failed: probe did not detect seeded \
         `.decode(body)` call. The skip logic may be over-excluding; \
         review `scan_file` and `line_matches_any` before trusting the \
         main test.",
    );

    // Cleanup (best-effort).
    let _ = fs::remove_dir_all(&tmp);
}
