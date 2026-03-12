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

use reovim_driver_display::render_backend::TuiExtension;

/// Create all default TUI extensions.
///
/// Called once at engine startup. The engine stores these and dispatches
/// notifications/rendering generically — zero extension knowledge.
#[must_use]
pub fn create_extensions() -> Vec<Box<dyn TuiExtension>> {
    vec![
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
        Box::new(reovim_tui_ext_landing::LandingExtension::new()),
    ]
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
