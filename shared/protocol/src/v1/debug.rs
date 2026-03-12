//! Debug RPC types for state inspection and diagnostics.
//!
//! This module contains types for debug endpoints that provide
//! runtime information, performance metrics, and log access.

use serde::{Deserialize, Serialize};

use super::types::Position;

// ============================================================================
// Phase 1: Foundation Types
// ============================================================================

/// Result for `debug/version` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionResult {
    /// Semantic version string (e.g., "0.9.0").
    pub version: String,
    /// Git commit hash (short form, e.g., "abc1234").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_hash: Option<String>,
    /// Git commit hash (full 40-char form).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_commit: Option<String>,
    /// Whether the working directory has uncommitted changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_dirty: Option<bool>,
    /// Build date in ISO 8601 format (date only, legacy).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_date: Option<String>,
    /// Build timestamp in full ISO 8601 format (with time).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_timestamp: Option<String>,
    /// Rust version used for compilation.
    pub rust_version: String,
    /// Target triple (e.g., "x86_64-unknown-linux-gnu").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// Result for `debug/uptime` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UptimeResult {
    /// Uptime in seconds (float for sub-second precision).
    pub uptime_seconds: f64,
    /// Human-readable uptime string (e.g., "1h 23m 45s").
    pub uptime_human: String,
    /// Server start time in ISO 8601 format.
    pub start_time: String,
}

// ============================================================================
// Phase 2: Inspection Types
// ============================================================================

/// Result for `debug/kernel_state` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelStateResult {
    /// Number of buffers currently loaded.
    pub buffer_count: usize,
    /// ID of the active buffer, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_buffer: Option<usize>,
    /// List of all buffer IDs.
    pub buffer_ids: Vec<usize>,
    /// Number of event handlers registered.
    pub event_handlers: usize,
    /// Number of events in the queue.
    pub event_queue_len: usize,
}

/// Yank type for registers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum YankType {
    /// Characterwise yank.
    Characterwise,
    /// Linewise yank.
    Linewise,
}

/// A single register entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterEntry {
    /// Register name ('"' for unnamed, 'a'-'z' for named).
    pub name: String,
    /// Register content (may be truncated for large values).
    pub content: String,
    /// Original content length before truncation.
    pub content_length: usize,
    /// Type of yank operation.
    pub yank_type: YankType,
}

/// Result for `debug/registers` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistersResult {
    /// Unnamed register (").
    pub unnamed: RegisterEntry,
    /// Named registers (a-z), only non-empty ones.
    pub named: Vec<RegisterEntry>,
}

/// A single mark entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkEntry {
    /// Mark name ('a'-'z' for local, 'A'-'Z' for global, special chars for special marks).
    pub name: String,
    /// Position of the mark.
    pub position: Position,
    /// Buffer ID for global marks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_id: Option<usize>,
}

/// Result for `debug/marks` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarksResult {
    /// Local marks (a-z) for current buffer.
    pub local: Vec<MarkEntry>,
    /// Global marks (A-Z) across all buffers.
    pub global: Vec<MarkEntry>,
    /// Special marks ('.', '^', etc.).
    pub special: Vec<MarkEntry>,
}

/// Result for `debug/mode_stack` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeStackResult {
    /// Current active mode display name.
    pub current: String,
    /// Full mode stack (bottom to top).
    pub stack: Vec<String>,
    /// Stack depth.
    pub depth: usize,
}

// ============================================================================
// Phase 3: Metrics Types
// ============================================================================

/// A single metric entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEntry {
    /// Metric name.
    pub name: String,
    /// Metric value.
    pub value: u64,
}

/// Result for `debug/metrics` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResult {
    /// Server uptime in seconds.
    pub uptime_seconds: f64,
    /// Total RPC requests handled.
    pub total_requests: u64,
    /// Counter metrics.
    pub counters: Vec<MetricEntry>,
}

/// Handler statistics entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandlerStats {
    /// Handler method name.
    pub method: String,
    /// Total number of calls.
    pub call_count: u64,
    /// Total time spent in microseconds.
    pub total_micros: u64,
    /// Average time per call in microseconds.
    pub avg_micros: f64,
}

/// Result for `debug/handlers` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandlersResult {
    /// Statistics for each handler.
    pub handlers: Vec<HandlerStats>,
}

// ============================================================================
// Phase 4: Log Access Types
// ============================================================================

/// Parameters for `debug/log_level` method.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LogLevelParams {
    /// New log level to set (None = get current level).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
}

/// Result for `debug/log_level` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLevelResult {
    /// Current log level.
    pub level: String,
    /// Previous log level (only when setting).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
}

/// Parameters for `debug/log_tail` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogTailParams {
    /// Number of log entries to return (default: 50).
    #[serde(default = "default_log_count")]
    pub count: usize,

    /// Filter by minimum log level (trace, debug, info, warn, error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,

    /// Filter by target module (substring match).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    /// Filter by message content (substring match, case-insensitive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grep: Option<String>,
}

const fn default_log_count() -> usize {
    50
}

impl Default for LogTailParams {
    fn default() -> Self {
        Self {
            count: default_log_count(),
            level: None,
            target: None,
            grep: None,
        }
    }
}

/// A single log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntryResult {
    /// Timestamp in ISO 8601 format.
    pub timestamp: String,
    /// Log level (error, warn, info, debug, trace).
    pub level: String,
    /// Target module.
    pub target: String,
    /// Log message.
    pub message: String,
}

/// Result for `debug/log_tail` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogTailResult {
    /// Log entries (newest first).
    pub entries: Vec<LogEntryResult>,
    /// Number of entries dropped due to buffer overflow.
    pub overflow_count: u64,
}

/// Parameters for `debug/log_subscribe` method.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LogSubscribeParams {
    /// Minimum log level to receive (default: info).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
}

/// Result for `debug/log_subscribe` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSubscribeResult {
    /// Unique subscription ID for unsubscribing.
    pub subscription_id: u64,
}

/// Parameters for `debug/log_unsubscribe` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogUnsubscribeParams {
    /// Subscription ID to cancel.
    pub subscription_id: u64,
}

/// Result for `debug/log_unsubscribe` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogUnsubscribeResult {
    /// Whether the subscription was found and removed.
    pub success: bool,
}

// ============================================================================
// Phase 5: Visual Debug Types
// ============================================================================

/// Server information section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotServerSection {
    /// Server version.
    pub version: String,
    /// Uptime in seconds.
    pub uptime_seconds: f64,
    /// Session ID.
    pub session_id: String,
    /// Number of connected clients.
    pub client_count: usize,
}

/// Editor state section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEditorSection {
    /// Current mode.
    pub mode: String,
    /// Cursor position.
    pub cursor: Position,
    /// Selection info (if active).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<SnapshotSelectionInfo>,
}

/// Selection info for snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotSelectionInfo {
    /// Selection mode (character, line, block).
    pub mode: String,
    /// Anchor position.
    pub anchor: Position,
    /// Cursor position.
    pub cursor: Position,
}

/// Buffer info for snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotBufferInfo {
    /// Buffer ID.
    pub id: usize,
    /// File path if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// Whether buffer is modified.
    pub modified: bool,
    /// Number of lines.
    pub line_count: usize,
    /// Preview of first few lines.
    pub preview: Vec<String>,
}

/// Buffers section for snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotBuffersSection {
    /// Total buffer count.
    pub count: usize,
    /// Active buffer ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_id: Option<usize>,
    /// Buffer info list.
    pub buffers: Vec<SnapshotBufferInfo>,
}

/// UI section for snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotUiSection {
    /// Screen width.
    pub width: u16,
    /// Screen height.
    pub height: u16,
    /// ASCII art representation of the screen.
    pub ascii_art: String,
}

/// Vim state section for snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotVimSection {
    /// Registers summary.
    pub registers: RegistersResult,
    /// Marks summary.
    pub marks: MarksResult,
    /// Mode stack.
    pub mode_stack: ModeStackResult,
}

/// Result for `debug/visual_snapshot` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualSnapshotResult {
    /// Schema version for compatibility.
    pub schema_version: String,
    /// Timestamp in ISO 8601 format.
    pub timestamp: String,
    /// Server information.
    pub server: SnapshotServerSection,
    /// Editor state.
    pub editor: SnapshotEditorSection,
    /// Buffer information.
    pub buffers: SnapshotBuffersSection,
    /// UI state.
    pub ui: SnapshotUiSection,
    /// Vim state (registers, marks, modes).
    pub vim: SnapshotVimSection,
    /// Performance metrics.
    pub metrics: MetricsResult,
}

#[cfg(test)]
#[path = "debug_tests.rs"]
mod tests;
