#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Shared extension kind identifiers.
//!
//! These constants are the single source of truth for extension kind strings
//! used by server bridge modules ([`ExtensionStateBridge::kind()`]) and client
//! extensions ([`TuiExtension::kind()`], [`WebExtension.kind()`]).
//!
//! Adding a new extension: add a constant here, then use it in both the
//! server bridge and the client extension.

/// Which-key popup hint overlay.
pub const WHICHKEY: &str = "whichkey";

/// Command-line input popup.
pub const CMDLINE: &str = "cmdline";

/// Toast notification popup.
pub const NOTIFICATION: &str = "notification";

/// Microscope fuzzy-finder picker UI.
pub const MICROSCOPE: &str = "microscope";

/// Completion popup.
pub const COMPLETION: &str = "completion";

/// File explorer sidebar.
pub const EXPLORER: &str = "explorer";

/// Polyblocks (tetromino) game overlay.
pub const POLYBLOCKS: &str = "polyblocks";

/// Range-finder jump navigation labels.
pub const RANGE_FINDER_JUMP: &str = "range-finder-jump";

/// Range-finder fold markers.
pub const RANGE_FINDER_FOLD: &str = "range-finder-fold";

/// LSP hover popup.
pub const HOVER: &str = "hover";

/// LSP signature help popup.
pub const SIGNATURE_HELP: &str = "signature-help";

/// LSP diagnostics inline annotations.
pub const DIAGNOSTICS: &str = "diagnostics";

/// Markdown rendering (tables, conceals).
pub const MARKDOWN: &str = "markdown";

/// All known extension kinds, alphabetically sorted.
///
/// Useful for validation and debugging.
pub const ALL: &[&str] = &[
    CMDLINE,
    COMPLETION,
    DIAGNOSTICS,
    EXPLORER,
    HOVER,
    MARKDOWN,
    MICROSCOPE,
    NOTIFICATION,
    POLYBLOCKS,
    RANGE_FINDER_FOLD,
    RANGE_FINDER_JUMP,
    SIGNATURE_HELP,
    WHICHKEY,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_count() {
        assert_eq!(ALL.len(), 13);
    }

    #[test]
    fn test_all_sorted() {
        for window in ALL.windows(2) {
            assert!(window[0] < window[1], "ALL is not sorted: {:?} >= {:?}", window[0], window[1]);
        }
    }

    #[test]
    fn test_all_unique() {
        let mut seen = std::collections::HashSet::new();
        for kind in ALL {
            assert!(seen.insert(kind), "Duplicate kind in ALL: {kind:?}");
        }
    }

    #[test]
    fn test_constants_nonempty() {
        for kind in ALL {
            assert!(!kind.is_empty(), "Kind constant must not be empty");
        }
    }

    #[test]
    fn test_constants_match_expected_values() {
        assert_eq!(WHICHKEY, "whichkey");
        assert_eq!(CMDLINE, "cmdline");
        assert_eq!(NOTIFICATION, "notification");
        assert_eq!(MICROSCOPE, "microscope");
        assert_eq!(COMPLETION, "completion");
        assert_eq!(EXPLORER, "explorer");
        assert_eq!(POLYBLOCKS, "polyblocks");
        assert_eq!(RANGE_FINDER_JUMP, "range-finder-jump");
        assert_eq!(RANGE_FINDER_FOLD, "range-finder-fold");
        assert_eq!(HOVER, "hover");
        assert_eq!(SIGNATURE_HELP, "signature-help");
        assert_eq!(DIAGNOSTICS, "diagnostics");
        assert_eq!(MARKDOWN, "markdown");
    }
}
