//! LSP progress notification types (LSP 3.15+)
//!
//! Handles $/progress notifications for work done progress reporting.

use serde::{Deserialize, Serialize};

/// Progress token (can be string or number)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProgressToken {
    /// String token (e.g., "rust-analyzer/Indexing")
    String(String),
    /// Numeric token
    Number(i64),
}

/// Progress notification params for $/progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressParams {
    /// Progress token
    pub token: ProgressToken,
    /// Progress value
    pub value: WorkDoneProgressValue,
}

/// Work done progress value (begin/report/end)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum WorkDoneProgressValue {
    /// Progress started
    #[serde(rename = "begin")]
    Begin(WorkDoneProgressBegin),
    /// Progress update
    #[serde(rename = "report")]
    Report(WorkDoneProgressReport),
    /// Progress completed
    #[serde(rename = "end")]
    End(WorkDoneProgressEnd),
}

/// Begin progress notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkDoneProgressBegin {
    /// Operation title
    pub title: String,
    /// Optional message (e.g., "294/295")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Percentage (0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<u32>,
    /// Whether operation can be cancelled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellable: Option<bool>,
}

/// Report progress notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkDoneProgressReport {
    /// Optional message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Percentage (0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<u32>,
    /// Whether operation can be cancelled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellable: Option<bool>,
}

/// End progress notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkDoneProgressEnd {
    /// Optional completion message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
