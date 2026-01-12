//! Re-exports from `lsp-types` crate.
//!
//! Provides convenient access to essential LSP protocol types.

// Core types
pub use lsp_types::{Diagnostic, DiagnosticSeverity, Location, Position, Range, Uri};

// Document sync
pub use lsp_types::TextDocumentContentChangeEvent;

// Navigation
pub use lsp_types::{GotoDefinitionResponse, Hover, HoverContents};

// Completion (for future phases)
pub use lsp_types::{CompletionResponse, CompletionTriggerKind};

// Code action (for future phases)
pub use lsp_types::CodeActionResponse;

// Symbols
pub use lsp_types::{DocumentSymbolResponse, SymbolInformation};

// Formatting
pub use lsp_types::{FormattingOptions, TextEdit};

// Workspace
pub use lsp_types::{WorkspaceEdit, WorkspaceFolder};

// Progress
pub use lsp_types::ProgressParams;

// Messages
pub use lsp_types::MessageType;

// Signature
pub use lsp_types::SignatureHelp;
