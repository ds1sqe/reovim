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

use {
    reovim_driver_depgraph::{DepEntry, resolve_dependencies},
    reovim_driver_display::render_backend::TuiExtension,
    std::collections::HashMap,
};

/// Create all default TUI extensions, sorted by dependency order (#583).
///
/// Called once at engine startup. Extensions are topologically sorted via
/// `reovim-driver-depgraph` and `init()` is called on each in dependency
/// order. The engine stores these and dispatches notifications/rendering
/// generically — zero extension knowledge.
///
/// # Panics
///
/// Panics if the extension dependency graph contains a cycle or an
/// unresolvable dependency — this is a programming error.
#[must_use]
pub fn create_extensions() -> Vec<Box<dyn TuiExtension>> {
    let mut extensions: Vec<Box<dyn TuiExtension>> = vec![
        Box::new(reovim_tui_ext_whichkey::WhichKeyExtension::new()),
        Box::new(reovim_tui_ext_cmdline::CmdlineExtension::new()),
        Box::new(reovim_tui_ext_notification::NotificationExtension::new()),
        Box::new(reovim_tui_ext_microscope::MicroscopeExtension::new()),
        Box::new(reovim_tui_ext_completion::CompletionExtension::new()),
        Box::new(reovim_tui_ext_explorer::ExplorerExtension::new()),
        Box::new(reovim_tui_ext_tetromino::TetrominoExtension::new()),
        Box::new(reovim_tui_ext_range_finder::RangeFinderJumpExtension::new()),
        Box::new(reovim_tui_ext_range_finder::RangeFinderFoldExtension::new()),
        Box::new(reovim_tui_ext_hover::HoverExtension::new()),
        Box::new(reovim_tui_ext_signature_help::SignatureHelpExtension::new()),
        Box::new(reovim_tui_ext_diagnostics::DiagnosticsExtension::new()),
        Box::new(reovim_tui_ext_markdown::MarkdownRenderExtension::new()),
    ];

    // Build dependency entries from extension declarations
    let entries: Vec<DepEntry<&'static str>> = extensions
        .iter()
        .map(|ext| DepEntry {
            key: ext.kind(),
            required: ext.dependencies().to_vec(),
            optional: Vec::new(),
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
