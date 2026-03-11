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
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_extensions_count() {
        let exts = create_extensions();
        assert_eq!(exts.len(), 12);
    }

    #[test]
    fn test_extension_kinds() {
        let exts = create_extensions();
        let kinds: Vec<&str> = exts.iter().map(|e| e.kind()).collect();
        assert!(kinds.contains(&"whichkey"));
        assert!(kinds.contains(&"cmdline"));
        assert!(kinds.contains(&"notification"));
        assert!(kinds.contains(&"microscope"));
        assert!(kinds.contains(&"completion"));
        assert!(kinds.contains(&"explorer"));
        assert!(kinds.contains(&"polyblocks"));
        assert!(kinds.contains(&"range-finder-jump"));
        assert!(kinds.contains(&"range-finder-fold"));
        assert!(kinds.contains(&"hover"));
        assert!(kinds.contains(&"signature-help"));
        assert!(kinds.contains(&"diagnostics"));
    }

    #[test]
    fn test_all_extensions_initially_inactive() {
        let exts = create_extensions();
        for ext in &exts {
            assert!(!ext.is_active(), "Extension '{}' should start inactive", ext.kind());
        }
    }

    #[test]
    fn test_extension_dispatch() {
        let mut exts = create_extensions();

        // Find whichkey extension and activate it
        for ext in &mut exts {
            if ext.kind() == "whichkey" {
                ext.apply_notification(
                    r#"{"active":true,"prefix":"g","hints":[{"key":"g","command":"top"}]}"#,
                );
            }
        }

        // With the default 500ms show-delay, whichkey is not yet visible
        // (server_active=true but visible=false until tick() after delay)
        let active_count = exts.iter().filter(|e| e.is_active()).count();
        assert_eq!(active_count, 0);
    }
}
