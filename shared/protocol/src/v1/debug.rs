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
mod tests {
    use super::*;

    #[test]
    fn test_version_result_serialization() {
        let result = VersionResult {
            version: "0.9.0".to_string(),
            git_hash: Some("abc1234".to_string()),
            git_commit: Some("abc1234567890abcdef1234567890abcdef123456".to_string()),
            git_dirty: Some(false),
            build_date: Some("2025-01-14".to_string()),
            build_timestamp: Some("2025-01-14T10:30:00+0000".to_string()),
            rust_version: "1.92.0".to_string(),
            target: Some("x86_64-unknown-linux-gnu".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"version\":\"0.9.0\""));
        assert!(json.contains("\"git_hash\":\"abc1234\""));
        assert!(json.contains("\"git_commit\":"));
        assert!(json.contains("\"git_dirty\":false"));
        assert!(json.contains("\"rust_version\":\"1.92.0\""));
        assert!(json.contains("\"target\":\"x86_64-unknown-linux-gnu\""));
    }

    #[test]
    fn test_version_result_without_optional() {
        let result = VersionResult {
            version: "0.9.0".to_string(),
            git_hash: None,
            git_commit: None,
            git_dirty: None,
            build_date: None,
            build_timestamp: None,
            rust_version: "1.92.0".to_string(),
            target: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("git_hash"));
        assert!(!json.contains("git_commit"));
        assert!(!json.contains("git_dirty"));
        assert!(!json.contains("build_date"));
        assert!(!json.contains("build_timestamp"));
        assert!(!json.contains("target"));
    }

    #[test]
    fn test_uptime_result_serialization() {
        let result = UptimeResult {
            uptime_seconds: 3661.5,
            uptime_human: "1h 1m 1s".to_string(),
            start_time: "2025-01-14T10:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"uptime_seconds\":3661.5"));
        assert!(json.contains("\"uptime_human\":\"1h 1m 1s\""));
    }

    #[test]
    fn test_kernel_state_result_serialization() {
        let result = KernelStateResult {
            buffer_count: 3,
            active_buffer: Some(1),
            buffer_ids: vec![1, 2, 3],
            event_handlers: 10,
            event_queue_len: 0,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"buffer_count\":3"));
        assert!(json.contains("\"active_buffer\":1"));
    }

    #[test]
    fn test_yank_type_serialization() {
        assert_eq!(serde_json::to_string(&YankType::Characterwise).unwrap(), "\"characterwise\"");
        assert_eq!(serde_json::to_string(&YankType::Linewise).unwrap(), "\"linewise\"");
    }

    #[test]
    fn test_register_entry_serialization() {
        let entry = RegisterEntry {
            name: "\"".to_string(),
            content: "hello".to_string(),
            content_length: 5,
            yank_type: YankType::Characterwise,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"name\":\"\\\"\""));
        assert!(json.contains("\"yank_type\":\"characterwise\""));
    }

    #[test]
    fn test_mark_entry_serialization() {
        let entry = MarkEntry {
            name: "a".to_string(),
            position: Position::new(10, 5),
            buffer_id: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"name\":\"a\""));
        assert!(!json.contains("buffer_id"));
    }

    #[test]
    fn test_log_tail_params_default() {
        let params = LogTailParams::default();
        assert_eq!(params.count, 50);
        assert!(params.level.is_none());
        assert!(params.target.is_none());
        assert!(params.grep.is_none());
    }

    #[test]
    fn test_log_tail_params_with_filters() {
        let params = LogTailParams {
            count: 100,
            level: Some("warn".to_string()),
            target: Some("runner::server".to_string()),
            grep: Some("error".to_string()),
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"count\":100"));
        assert!(json.contains("\"level\":\"warn\""));
        assert!(json.contains("\"target\":\"runner::server\""));
        assert!(json.contains("\"grep\":\"error\""));
    }

    #[test]
    fn test_log_tail_params_partial_filters() {
        // Only level filter
        let params = LogTailParams {
            count: 50,
            level: Some("info".to_string()),
            target: None,
            grep: None,
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"level\":\"info\""));
        assert!(!json.contains("\"target\""));
        assert!(!json.contains("\"grep\""));
    }

    #[test]
    fn test_log_tail_params_deserialization() {
        // With all filters
        let json = r#"{"count": 25, "level": "debug", "target": "mymod", "grep": "test"}"#;
        let params: LogTailParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.count, 25);
        assert_eq!(params.level.as_deref(), Some("debug"));
        assert_eq!(params.target.as_deref(), Some("mymod"));
        assert_eq!(params.grep.as_deref(), Some("test"));

        // With default count
        let json = r#"{"level": "warn"}"#;
        let params: LogTailParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.count, 50);
        assert_eq!(params.level.as_deref(), Some("warn"));
    }

    #[test]
    fn test_handler_stats_serialization() {
        let stats = HandlerStats {
            method: "input/keys".to_string(),
            call_count: 100,
            total_micros: 5000,
            avg_micros: 50.0,
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"method\":\"input/keys\""));
        assert!(json.contains("\"avg_micros\":50.0"));
    }

    #[test]
    fn test_log_level_params_serialization() {
        // Get current level (no level specified)
        let params = LogLevelParams::default();
        let json = serde_json::to_string(&params).unwrap();
        assert_eq!(json, "{}");

        // Set new level
        let params = LogLevelParams {
            level: Some("debug".to_string()),
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"level\":\"debug\""));
    }

    // Phase 1 tests for #332 - subscription types

    #[test]
    fn test_log_subscribe_params_serialization() {
        // Default (no level filter)
        let params = LogSubscribeParams::default();
        let json = serde_json::to_string(&params).unwrap();
        assert_eq!(json, "{}");

        // With level filter
        let params = LogSubscribeParams {
            level: Some("warn".to_string()),
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"level\":\"warn\""));
    }

    #[test]
    fn test_log_subscribe_result_serialization() {
        let result = LogSubscribeResult {
            subscription_id: 42,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"subscription_id\":42"));
    }

    #[test]
    fn test_log_unsubscribe_params_serialization() {
        let params = LogUnsubscribeParams {
            subscription_id: 123,
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"subscription_id\":123"));
    }

    #[test]
    fn test_log_unsubscribe_result_serialization() {
        let result = LogUnsubscribeResult { success: true };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));

        let result = LogUnsubscribeResult { success: false };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":false"));
    }
}
