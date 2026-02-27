#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
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
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_extensions_returns_three() {
        let exts = create_extensions();
        assert_eq!(exts.len(), 3);
    }

    #[test]
    fn test_extension_kinds() {
        let exts = create_extensions();
        let kinds: Vec<&str> = exts.iter().map(|e| e.kind()).collect();
        assert!(kinds.contains(&"whichkey"));
        assert!(kinds.contains(&"cmdline"));
        assert!(kinds.contains(&"notification"));
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
