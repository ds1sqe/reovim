//! LSP request types.
//!
//! Defines the request variants for LSP operations. Currently implements
//! 7 core variants; additional variants will be added in future phases.

use std::fmt;

use {
    lsp_types::{
        CompletionResponse, GotoDefinitionResponse, Hover, Location, Position, SignatureHelp, Uri,
    },
    reovim_kernel::api::v1::OneshotSender,
};

use crate::error::LspError;

/// Result type for navigation requests.
pub type NavigationResult<T> = Result<T, LspError>;

/// LSP request variants.
///
/// # Notification vs Request
///
/// - **Notifications** (`DidOpen`, `DidChange`, `DidClose`, `Shutdown`): Fire-and-forget,
///   no response expected.
/// - **Requests** (`GotoDefinition`, `References`, `Hover`): Include a `response_tx`
///   oneshot channel for async response.
///
/// # Future Extensions
///
/// Additional variants will be added in Phase 5:
/// - `Completion`
/// - `SignatureHelp`
/// - `CodeAction`
/// - `Rename`
/// - `DocumentSymbol`
/// - `WorkspaceSymbol`
/// - `Formatting`
/// - `RangeFormatting`
pub enum LspRequest {
    /// Notify server that a document was opened.
    DidOpen {
        /// Document URI.
        uri: Uri,
        /// Language ID (e.g., "rust", "python").
        language_id: String,
        /// Document version.
        version: i32,
        /// Document content.
        content: String,
    },
    /// Notify server that a document changed (full content sync).
    DidChange {
        /// Document URI.
        uri: Uri,
        /// Document version.
        version: i32,
        /// New content (FULL sync mode).
        content: String,
    },
    /// Notify server that a document was closed.
    DidClose {
        /// Document URI.
        uri: Uri,
    },
    /// Request go-to-definition.
    GotoDefinition {
        /// Document URI.
        uri: Uri,
        /// Cursor position.
        position: Position,
        /// Response channel.
        response_tx: OneshotSender<NavigationResult<Option<GotoDefinitionResponse>>>,
    },
    /// Request find-references.
    References {
        /// Document URI.
        uri: Uri,
        /// Cursor position.
        position: Position,
        /// Include the declaration in results.
        include_declaration: bool,
        /// Response channel.
        response_tx: OneshotSender<NavigationResult<Option<Vec<Location>>>>,
    },
    /// Request hover information.
    Hover {
        /// Document URI.
        uri: Uri,
        /// Cursor position.
        position: Position,
        /// Response channel.
        response_tx: OneshotSender<NavigationResult<Option<Hover>>>,
    },
    /// Request completion items at a position.
    Completion {
        /// Document URI.
        uri: Uri,
        /// Cursor position.
        position: Position,
        /// Response channel.
        response_tx: OneshotSender<NavigationResult<Option<CompletionResponse>>>,
    },
    /// Notify server that a document was saved.
    DidSave {
        /// Document URI.
        uri: Uri,
        /// Document content (included if server requests it via save options).
        text: Option<String>,
    },
    /// Request signature help at a position.
    SignatureHelp {
        /// Document URI.
        uri: Uri,
        /// Cursor position.
        position: Position,
        /// Response channel.
        response_tx: OneshotSender<NavigationResult<Option<SignatureHelp>>>,
    },
    /// Request server shutdown.
    Shutdown,
}

// Custom Debug impl that hides response channels and large content
impl fmt::Debug for LspRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DidOpen {
                uri,
                language_id,
                version,
                ..
            } => f
                .debug_struct("DidOpen")
                .field("uri", uri)
                .field("language_id", language_id)
                .field("version", version)
                .finish(),
            Self::DidChange { uri, version, .. } => f
                .debug_struct("DidChange")
                .field("uri", uri)
                .field("version", version)
                .finish(),
            Self::DidClose { uri } => f.debug_struct("DidClose").field("uri", uri).finish(),
            Self::GotoDefinition { uri, position, .. } => f
                .debug_struct("GotoDefinition")
                .field("uri", uri)
                .field("position", position)
                .finish_non_exhaustive(),
            Self::References {
                uri,
                position,
                include_declaration,
                ..
            } => f
                .debug_struct("References")
                .field("uri", uri)
                .field("position", position)
                .field("include_declaration", include_declaration)
                .finish_non_exhaustive(),
            Self::Hover { uri, position, .. } => f
                .debug_struct("Hover")
                .field("uri", uri)
                .field("position", position)
                .finish_non_exhaustive(),
            Self::Completion { uri, position, .. } => f
                .debug_struct("Completion")
                .field("uri", uri)
                .field("position", position)
                .finish_non_exhaustive(),
            Self::DidSave { uri, .. } => f
                .debug_struct("DidSave")
                .field("uri", uri)
                .finish_non_exhaustive(),
            Self::SignatureHelp { uri, position, .. } => f
                .debug_struct("SignatureHelp")
                .field("uri", uri)
                .field("position", position)
                .finish_non_exhaustive(),
            Self::Shutdown => write!(f, "Shutdown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_uri(path: &str) -> Uri {
        path.parse().expect("test URI should parse")
    }

    #[test]
    fn test_did_open_debug() {
        let req = LspRequest::DidOpen {
            uri: make_uri("file:///test.rs"),
            language_id: "rust".to_string(),
            version: 1,
            content: "fn main() {}".to_string(),
        };
        let debug_str = format!("{req:?}");
        assert!(debug_str.contains("DidOpen"));
        assert!(debug_str.contains("rust"));
        assert!(debug_str.contains("version: 1"));
        // Content should NOT be in debug output
        assert!(!debug_str.contains("fn main"));
    }

    #[test]
    fn test_did_change_debug() {
        let req = LspRequest::DidChange {
            uri: make_uri("file:///test.rs"),
            version: 2,
            content: "fn main() { println!(\"hello\"); }".to_string(),
        };
        let debug_str = format!("{req:?}");
        assert!(debug_str.contains("DidChange"));
        assert!(debug_str.contains("version: 2"));
    }

    #[test]
    fn test_did_close_debug() {
        let req = LspRequest::DidClose {
            uri: make_uri("file:///test.rs"),
        };
        let debug_str = format!("{req:?}");
        assert!(debug_str.contains("DidClose"));
        assert!(debug_str.contains("test.rs"));
    }

    #[test]
    fn test_shutdown_debug() {
        let req = LspRequest::Shutdown;
        assert_eq!(format!("{req:?}"), "Shutdown");
    }

    #[test]
    fn test_goto_definition_debug() {
        let (tx, _rx) = reovim_kernel::api::v1::oneshot();
        let req = LspRequest::GotoDefinition {
            uri: make_uri("file:///test.rs"),
            position: Position::new(10, 5),
            response_tx: tx,
        };
        let debug_str = format!("{req:?}");
        assert!(debug_str.contains("GotoDefinition"));
        assert!(debug_str.contains("position"));
        // response_tx should NOT be in debug output
        assert!(!debug_str.contains("response_tx"));
    }

    #[test]
    fn test_references_debug() {
        let (tx, _rx) = reovim_kernel::api::v1::oneshot();
        let req = LspRequest::References {
            uri: make_uri("file:///test.rs"),
            position: Position::new(5, 10),
            include_declaration: true,
            response_tx: tx,
        };
        let debug_str = format!("{req:?}");
        assert!(debug_str.contains("References"));
        assert!(debug_str.contains("include_declaration: true"));
    }

    #[test]
    fn test_completion_debug() {
        let (tx, _rx) = reovim_kernel::api::v1::oneshot();
        let req = LspRequest::Completion {
            uri: make_uri("file:///test.rs"),
            position: Position::new(5, 10),
            response_tx: tx,
        };
        let debug_str = format!("{req:?}");
        assert!(debug_str.contains("Completion"));
        assert!(debug_str.contains("position"));
        assert!(!debug_str.contains("response_tx"));
    }

    #[test]
    fn test_hover_debug() {
        let (tx, _rx) = reovim_kernel::api::v1::oneshot();
        let req = LspRequest::Hover {
            uri: make_uri("file:///test.rs"),
            position: Position::new(1, 1),
            response_tx: tx,
        };
        let debug_str = format!("{req:?}");
        assert!(debug_str.contains("Hover"));
        assert!(debug_str.contains("position"));
    }

    #[test]
    fn test_did_save_debug() {
        let req = LspRequest::DidSave {
            uri: make_uri("file:///test.rs"),
            text: Some("fn main() {}".to_string()),
        };
        let debug_str = format!("{req:?}");
        assert!(debug_str.contains("DidSave"));
        assert!(debug_str.contains("test.rs"));
        // text should NOT be in debug output (finish_non_exhaustive)
    }

    #[test]
    fn test_did_save_no_text_debug() {
        let req = LspRequest::DidSave {
            uri: make_uri("file:///test.rs"),
            text: None,
        };
        let debug_str = format!("{req:?}");
        assert!(debug_str.contains("DidSave"));
    }

    #[test]
    fn test_signature_help_debug() {
        let (tx, _rx) = reovim_kernel::api::v1::oneshot();
        let req = LspRequest::SignatureHelp {
            uri: make_uri("file:///test.rs"),
            position: Position::new(3, 7),
            response_tx: tx,
        };
        let debug_str = format!("{req:?}");
        assert!(debug_str.contains("SignatureHelp"));
        assert!(debug_str.contains("position"));
        assert!(!debug_str.contains("response_tx"));
    }
}
