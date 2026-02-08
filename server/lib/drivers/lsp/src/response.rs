//! LSP response types.
//!
//! Defines response variants for LSP operations. Most responses are
//! handled via oneshot channels in [`LspRequest`], but this module
//! provides unified types for response handling.

use lsp_types::{
    CodeActionResponse, CompletionResponse, DocumentSymbolResponse, GotoDefinitionResponse, Hover,
    Location, SignatureHelp, SymbolInformation, TextEdit, WorkspaceEdit,
};

/// LSP response variants.
///
/// Corresponds to the request types in [`LspRequest`].
///
/// # Note
///
/// Most responses are sent via oneshot channels directly to the requester.
/// This enum provides a unified type for cases where responses need to be
/// stored or processed generically.
#[derive(Debug)]
pub enum LspResponse {
    /// Response to `GotoDefinition` request.
    Definition(Option<GotoDefinitionResponse>),
    /// Response to `References` request.
    References(Option<Vec<Location>>),
    /// Response to `Hover` request.
    Hover(Option<Hover>),
    /// Response to `DocumentSymbol` request (future).
    DocumentSymbol(Option<DocumentSymbolResponse>),
    /// Response to `Completion` request (future).
    Completion(Option<CompletionResponse>),
    /// Response to `SignatureHelp` request (future).
    SignatureHelp(Option<SignatureHelp>),
    /// Response to `CodeAction` request (future).
    CodeAction(Option<CodeActionResponse>),
    /// Response to `Rename` request (future).
    Rename(Option<WorkspaceEdit>),
    /// Response to `WorkspaceSymbol` request (future).
    WorkspaceSymbol(Option<Vec<SymbolInformation>>),
    /// Response to `Formatting` request (future).
    Formatting(Option<Vec<TextEdit>>),
    /// Empty response (for notifications or void responses).
    Empty,
}

impl LspResponse {
    /// Check if the response is empty or None.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        matches!(
            self,
            Self::Empty
                | Self::Definition(None)
                | Self::References(None)
                | Self::Hover(None)
                | Self::DocumentSymbol(None)
                | Self::Completion(None)
                | Self::SignatureHelp(None)
                | Self::CodeAction(None)
                | Self::Rename(None)
                | Self::WorkspaceSymbol(None)
                | Self::Formatting(None)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_debug() {
        let resp = LspResponse::Empty;
        assert_eq!(format!("{resp:?}"), "Empty");
    }

    #[test]
    fn test_is_empty() {
        assert!(LspResponse::Empty.is_empty());
        assert!(LspResponse::Definition(None).is_empty());
        assert!(LspResponse::References(None).is_empty());
        assert!(LspResponse::Hover(None).is_empty());
    }

    #[test]
    fn test_is_not_empty() {
        let locations = vec![];
        assert!(!LspResponse::References(Some(locations)).is_empty());
    }

    #[test]
    fn test_is_empty_all_none_variants() {
        assert!(LspResponse::DocumentSymbol(None).is_empty());
        assert!(LspResponse::Completion(None).is_empty());
        assert!(LspResponse::SignatureHelp(None).is_empty());
        assert!(LspResponse::CodeAction(None).is_empty());
        assert!(LspResponse::Rename(None).is_empty());
        assert!(LspResponse::WorkspaceSymbol(None).is_empty());
        assert!(LspResponse::Formatting(None).is_empty());
    }

    #[test]
    fn test_is_not_empty_with_some_variants() {
        assert!(!LspResponse::Formatting(Some(vec![])).is_empty());
        assert!(!LspResponse::WorkspaceSymbol(Some(vec![])).is_empty());
        assert!(!LspResponse::CodeAction(Some(vec![])).is_empty());
    }

    #[test]
    fn test_response_debug_variants() {
        let resp = LspResponse::Definition(None);
        assert!(format!("{resp:?}").contains("Definition"));

        let resp = LspResponse::Hover(None);
        assert!(format!("{resp:?}").contains("Hover"));

        let resp = LspResponse::Rename(None);
        assert!(format!("{resp:?}").contains("Rename"));
    }
}
