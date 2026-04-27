//! Builtin client module factory map.
//!
//! This is the canonical list of all TUI client modules. Keys MUST match
//! each module's `kind()` return value. Ordering comes from dependency
//! resolution in `ClientModuleLoader`, not from this map.

use std::collections::HashMap;

use reovim_client_driver::ClientModuleFactory;

/// Number of builtin client modules. Update when adding/removing modules.
const BUILTIN_MODULE_COUNT: usize = 20;

/// Return the builtin client module factory map.
///
/// Each entry maps a module kind string to a factory function that
/// constructs a `Box<dyn ClientModule>`. The loader uses these factories
/// to instantiate modules, resolve dependencies, and manage lifecycle.
#[must_use]
pub fn builtin_client_modules() -> HashMap<&'static str, ClientModuleFactory> {
    let mut map = HashMap::with_capacity(BUILTIN_MODULE_COUNT);
    map.insert(
        "statusline",
        (|| Box::new(reovim_tui_mod_statusline::StatuslineModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "hover",
        (|| Box::new(reovim_tui_mod_hover::HoverModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "signature-help",
        (|| Box::new(reovim_tui_mod_signature_help::SignatureHelpModule::new()))
            as ClientModuleFactory,
    );
    map.insert(
        "landing",
        (|| Box::new(reovim_tui_mod_landing::LandingModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "completion",
        (|| Box::new(reovim_tui_mod_completion::CompletionModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "notification",
        (|| Box::new(reovim_tui_mod_notification::NotificationModule::new()))
            as ClientModuleFactory,
    );
    map.insert(
        "whichkey",
        (|| Box::new(reovim_tui_mod_whichkey::WhichKeyModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "cmdline",
        (|| Box::new(reovim_tui_mod_cmdline::CmdlineModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "microscope",
        (|| Box::new(reovim_tui_mod_microscope::MicroscopeModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "explorer",
        (|| Box::new(reovim_tui_mod_explorer::ExplorerModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "polyblocks",
        (|| Box::new(reovim_tui_mod_tetromino::TetrominoModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "line-numbers",
        (|| Box::new(reovim_tui_mod_line_numbers::LineNumbersModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "range-finder-fold",
        (|| Box::new(reovim_tui_mod_fold::FoldModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "range-finder-jump",
        (|| Box::new(reovim_tui_mod_jump::JumpModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "pair",
        (|| Box::new(reovim_tui_mod_pair::PairModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "yank-flash",
        (|| Box::new(reovim_tui_mod_yank_flash::YankFlashModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "illuminate",
        (|| Box::new(reovim_tui_mod_illuminate::IlluminateModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "diagnostics",
        (|| Box::new(reovim_tui_mod_diagnostics::DiagnosticsModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "markdown",
        (|| Box::new(reovim_tui_mod_markdown::MarkdownModule::new())) as ClientModuleFactory,
    );
    map.insert(
        "bufferline",
        (|| Box::new(reovim_tui_mod_bufferline::BufferlineModule::new())) as ClientModuleFactory,
    );
    map
}

#[cfg(test)]
#[path = "static_client_modules_tests.rs"]
mod tests;
