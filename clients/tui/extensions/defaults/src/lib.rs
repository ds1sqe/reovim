//! Default TUI modules meta-crate.
//!
//! This is the game-mod boundary: the engine depends ONLY on this crate
//! for module registration. It never imports individual module crates.
//!
//! All extensions have been migrated to native `ClientModule` implementations.
//! The legacy `TuiExtension` trait and bridge adapter are no longer used.

use reovim_client_driver::ClientModule;

/// Create all native `ClientModule` instances.
///
/// Called once at engine startup. The engine stores these and dispatches
/// notifications/rendering generically -- zero UI feature knowledge.
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
        Box::new(reovim_tui_mod_line_numbers::LineNumbersModule::new()),
        Box::new(reovim_tui_mod_fold::FoldModule::new()),
        Box::new(reovim_tui_mod_jump::JumpModule::new()),
        Box::new(reovim_tui_mod_pair::PairModule::new()),
        Box::new(reovim_tui_mod_diagnostics::DiagnosticsModule::new()),
        Box::new(reovim_tui_mod_markdown::MarkdownModule::new()),
    ]
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
