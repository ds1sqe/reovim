//! Default TUI extensions meta-crate.
//!
//! This is the game-mod boundary: the engine depends ONLY on this crate
//! for extension registration. It never imports individual extension crates.
//!
//! # Adding a new extension
//!
//! 1. Create `clients/tui/extensions/{name}/` with `TuiExtension` impl
//! 2. Add dependency here in `Cargo.toml`
//! 3. Add `Box::new(YourExtension::new())` to `create_extensions()`
//! 4. Done — engine picks it up automatically

use std::collections::{HashMap, HashSet};

use {
    reovim_depgraph::{DepEntry, resolve_dependencies},
    reovim_driver_display::render_backend::TuiExtension,
};

use reovim_client_driver::ClientModule;

/// Create native `ClientModule` instances that bypass the bridge adapter.
///
/// These modules implement `ClientModule` directly (not wrapped via
/// `TuiExtensionBridge`). Called alongside `create_extensions()` during
/// engine startup.
#[must_use]
pub fn create_native_modules() -> Vec<Box<dyn ClientModule>> {
    vec![
        Box::new(reovim_tui_mod_statusline::StatuslineModule::new()),
        Box::new(reovim_tui_mod_hover::HoverModule::new()),
        Box::new(reovim_tui_mod_signature_help::SignatureHelpModule::new()),
        Box::new(reovim_tui_mod_landing::LandingModule::new()),
        Box::new(reovim_tui_mod_completion::CompletionModule::new()),
        Box::new(reovim_tui_mod_notification::NotificationModule::new()),
        Box::new(reovim_tui_mod_whichkey::WhichKeyModule::new()),
        Box::new(reovim_tui_mod_cmdline::CmdlineModule::new()),
        Box::new(reovim_tui_mod_microscope::MicroscopeModule::new()),
        Box::new(reovim_tui_mod_explorer::ExplorerModule::new()),
        Box::new(reovim_tui_mod_tetromino::TetrominoModule::new()),
    ]
}

/// Create all default TUI extensions, sorted by dependency order (#583).
///
/// Called once at engine startup. Extensions are topologically sorted via
/// `reovim-depgraph` and `init()` is called on each in dependency
/// order. The engine stores these and dispatches notifications/rendering
/// generically — zero extension knowledge.
///
/// # Panics
///
/// Panics if the extension dependency graph contains a cycle or an
/// unresolvable dependency — this is a programming error.
#[must_use]
pub fn create_extensions() -> Vec<Box<dyn TuiExtension>> {
    create_extensions_filtered(&HashSet::new())
}

/// Create TUI extensions, excluding those whose kind is in `disabled_kinds`.
///
/// `disabled_kinds` contains extension kind strings (e.g., `"polyblocks"`,
/// `"completion"`) that should not be loaded. Extensions are filtered
/// before dependency resolution and initialization.
///
/// # Panics
///
/// Panics if the filtered extension dependency graph contains a cycle or an
/// unresolvable dependency — this is a programming error.
#[must_use]
pub fn create_extensions_filtered<S: std::hash::BuildHasher>(
    disabled_kinds: &HashSet<String, S>,
) -> Vec<Box<dyn TuiExtension>> {
    let all: Vec<Box<dyn TuiExtension>> = all_extensions();

    // Filter out disabled extensions before depgraph resolution
    let mut extensions: Vec<Box<dyn TuiExtension>> = if disabled_kinds.is_empty() {
        all
    } else {
        all.into_iter()
            .filter(|ext| !disabled_kinds.contains(ext.kind()))
            .collect()
    };

    // Build dependency entries from extension declarations
    let entries: Vec<DepEntry<&'static str>> = extensions
        .iter()
        .map(|ext| DepEntry {
            key: ext.kind(),
            required: ext.dependencies().to_vec(),
            optional: Vec::new(),
            provides_caps: vec![],
            requires_caps: vec![],
        })
        .collect();

    // Resolve dependency order via Kahn's topological sort
    let dep_order = resolve_dependencies(&entries)
        .expect("extension dependency graph is invalid — programming error");

    // Reorder extensions to match the sorted order
    let mut by_kind: HashMap<&'static str, Box<dyn TuiExtension>> = extensions
        .into_iter()
        .map(|ext| (ext.kind(), ext))
        .collect();
    extensions = dep_order
        .order
        .iter()
        .filter_map(|kind| by_kind.remove(kind))
        .collect();

    // Call init() in dependency order
    for ext in &mut extensions {
        ext.init();
    }

    extensions
}

/// Create all default TUI extension instances.
///
/// This is the canonical list. Filtering is applied by callers.
fn all_extensions() -> Vec<Box<dyn TuiExtension>> {
    vec![
        // cmdline migrated to native ClientModule (reovim-tui-mod-cmdline)
        // microscope migrated to native ClientModule (reovim-tui-mod-microscope)
        // explorer migrated to native ClientModule (reovim-tui-mod-explorer)
        // tetromino migrated to native ClientModule (reovim-tui-mod-tetromino)
        Box::new(reovim_tui_ext_range_finder::RangeFinderJumpExtension::new()),
        Box::new(reovim_tui_ext_range_finder::RangeFinderFoldExtension::new()),
        Box::new(reovim_tui_ext_diagnostics::DiagnosticsExtension::new()),
        Box::new(reovim_tui_ext_markdown::MarkdownRenderExtension::new()),
        Box::new(reovim_tui_ext_pair::PairExtension::new()),
    ]
}

/// Validate client extensions against server-declared extension kinds (#584).
///
/// For each loaded extension, checks if the server has a matching kind
/// in `server_available_kinds`. Logs warnings for unmatched extensions.
/// Non-fatal — extensions still load but won't receive server notifications.
pub fn validate_extensions(
    extensions: &[Box<dyn TuiExtension>],
    server_available_kinds: &[String],
) {
    for ext in extensions {
        for kind in ext.server_kinds() {
            if !server_available_kinds.iter().any(|sk| sk == kind) {
                tracing::warn!(
                    extension = ext.kind(),
                    server_kind = kind,
                    "Client extension expects server kind not available on server"
                );
            }
        }
    }
    tracing::info!(
        extensions = extensions.len(),
        server_kinds = server_available_kinds.len(),
        "Client extension validation complete"
    );
}

/// Shutdown all extensions in reverse dependency order (#583).
///
/// Calls `exit()` on each extension in reverse order (dependents first,
/// then their dependencies). Should be called during engine shutdown.
pub fn shutdown_extensions(extensions: &mut [Box<dyn TuiExtension>]) {
    for ext in extensions.iter_mut().rev() {
        ext.exit();
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
