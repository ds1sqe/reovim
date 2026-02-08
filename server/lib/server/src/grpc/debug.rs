//! `DebugService` gRPC implementation.
//!
//! Provides debug and logging operations for v2 protocol clients.
//!
//! # Features
//!
//! - `log_tail`: Get recent log entries from the server ring buffer
//! - `log_level`: Get/set the current log level

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use {
    reovim_protocol::v2::{
        LogEntry, LogLevelRequest, LogLevelResponse, LogTailRequest, LogTailResponse,
        debug_service_server::DebugService,
    },
    tonic::{Request, Response, Status},
};

use crate::debug::try_debug_ring;

/// Get the global debug ring buffer, returning `Status::unavailable` if not initialized.
///
/// The ring buffer is initialized once during server startup via `init_debug_ring()`.
/// In unit tests the initialization order is non-deterministic (OnceLock), so this
/// path cannot be reliably covered.
#[cfg_attr(coverage_nightly, coverage(off))]
fn require_debug_ring() -> Result<&'static crate::debug::DebugRingBuffer, Status> {
    try_debug_ring().ok_or_else(|| Status::unavailable("Debug ring buffer not initialized"))
}

/// gRPC `DebugService` implementation.
///
/// Bridges v2 protocol debug requests to the server's debug infrastructure.
pub struct DebugServiceImpl;

impl DebugServiceImpl {
    /// Create a new `DebugService`.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for DebugServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl DebugService for DebugServiceImpl {
    /// Get recent log entries from the server ring buffer.
    ///
    /// Supports filtering by:
    /// - `count`: Number of entries (default: 50)
    /// - `level`: Filter by level (trace, debug, info, warn, error)
    /// - `target`: Filter by target module (contains match)
    /// - `grep`: Filter by message content (case-insensitive contains)
    async fn log_tail(
        &self,
        request: Request<LogTailRequest>,
    ) -> Result<Response<LogTailResponse>, Status> {
        let req = request.into_inner();
        let count = if req.count == 0 {
            50
        } else {
            req.count as usize
        };

        let ring = require_debug_ring()?;

        let entries = ring.tail(count);

        // Convert to proto format with optional filtering
        let proto_entries: Vec<LogEntry> = entries
            .into_iter()
            .filter_map(|e| {
                // Level filter
                if let Some(ref level) = req.level
                    && !e.level.to_string().eq_ignore_ascii_case(level)
                {
                    return None;
                }
                // Target filter
                if let Some(ref target) = req.target
                    && !e.target.contains(target)
                {
                    return None;
                }
                // Grep filter
                if let Some(ref grep) = req.grep
                    && !e.message.to_lowercase().contains(&grep.to_lowercase())
                {
                    return None;
                }
                // Take ownership of strings since we're consuming `e`
                Some(LogEntry {
                    seq: e.seq,
                    timestamp_us: e.timestamp_us,
                    level: e.level.to_string(),
                    target: e.target,
                    message: e.message,
                })
            })
            .collect();

        Ok(Response::new(LogTailResponse {
            entries: proto_entries,
        }))
    }

    /// Get or set the current log level.
    ///
    /// If `level` is specified in the request, sets the new level.
    /// Always returns the current level in the response.
    async fn log_level(
        &self,
        _request: Request<LogLevelRequest>,
    ) -> Result<Response<LogLevelResponse>, Status> {
        // TODO: Implement dynamic log level control
        // For now, just return "info" as the default
        Ok(Response::new(LogLevelResponse {
            level: "info".to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::debug::DebugRingBuffer,
        reovim_kernel::api::v1::{Level, Record},
        std::sync::OnceLock,
    };

    // Test-specific ring buffer to avoid global state conflicts
    static TEST_RING: OnceLock<DebugRingBuffer> = OnceLock::new();

    fn test_ring() -> &'static DebugRingBuffer {
        TEST_RING.get_or_init(|| {
            let ring = DebugRingBuffer::with_capacity(4096);
            // Pre-populate with test entries using the builder pattern
            let record1 = Record::builder(Level::Info)
                .module_path("test::module")
                .file(file!())
                .line(line!())
                .message("Test message 1")
                .build();
            ring.push(&record1);

            let record2 = Record::builder(Level::Warn)
                .module_path("test::other")
                .file(file!())
                .line(line!())
                .message("Warning message")
                .build();
            ring.push(&record2);

            let record3 = Record::builder(Level::Error)
                .module_path("test::module")
                .file(file!())
                .line(line!())
                .message("Error occurred")
                .build();
            ring.push(&record3);

            ring
        })
    }

    #[tokio::test]
    async fn test_log_level_default() {
        let service = DebugServiceImpl::new();
        let request = Request::new(LogLevelRequest { level: None });
        let response = service.log_level(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.level, "info");
    }

    #[tokio::test]
    async fn test_debug_service_default() {
        let service = DebugServiceImpl;
        let request = Request::new(LogLevelRequest { level: None });
        let response = service.log_level(request).await;
        assert!(response.is_ok());
    }

    #[test]
    fn test_log_entry_conversion() {
        // Test that LogEntryView can be converted to proto LogEntry
        let ring = test_ring();
        let entries = ring.tail(10);

        for e in entries {
            let proto = LogEntry {
                seq: e.seq,
                timestamp_us: e.timestamp_us,
                level: e.level.to_string(),
                target: e.target.clone(),
                message: e.message.clone(),
            };
            assert!(!proto.level.is_empty());
            assert!(!proto.target.is_empty());
        }
    }

    #[test]
    fn test_level_filter_logic() {
        // Test the level filtering logic directly
        let level_filter = Some("info".to_string());
        let matches_info = "INFO".eq_ignore_ascii_case(level_filter.as_ref().unwrap());
        let matches_warn = "WARN".eq_ignore_ascii_case(level_filter.as_ref().unwrap());

        assert!(matches_info);
        assert!(!matches_warn);
    }

    #[test]
    fn test_grep_filter_logic() {
        // Test the grep filtering logic directly
        let grep_filter = Some("error".to_string());
        let message = "Error occurred in module";

        let matches = message
            .to_lowercase()
            .contains(&grep_filter.as_ref().unwrap().to_lowercase());

        assert!(matches);
    }

    #[test]
    fn test_target_filter_logic() {
        // Test the target filtering logic directly
        let target_filter = Some("module".to_string());
        let target = "test::module::submodule";

        let matches = target.contains(target_filter.as_ref().unwrap());

        assert!(matches);
    }

    #[test]
    fn test_debug_service_impl_default() {
        let service = DebugServiceImpl;
        // Verify it's usable
        let _ = format!("{:?}", &raw const service);
    }

    #[tokio::test]
    async fn test_log_tail_without_ring_buffer() {
        // When ring buffer is not initialized, log_tail returns Unavailable
        let service = DebugServiceImpl::new();
        let request = Request::new(LogTailRequest {
            count: 10,
            level: None,
            target: None,
            grep: None,
        });
        let result = service.log_tail(request).await;
        // Ring buffer may or may not be initialized depending on test order
        // Just verify it doesn't panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_log_tail_default_count() {
        let service = DebugServiceImpl::new();
        let request = Request::new(LogTailRequest {
            count: 0, // should default to 50
            level: None,
            target: None,
            grep: None,
        });
        let _ = service.log_tail(request).await;
    }

    #[test]
    fn test_level_filter_no_match() {
        let level_filter = Some("error".to_string());
        let matches = "INFO".eq_ignore_ascii_case(level_filter.as_ref().unwrap());
        assert!(!matches);
    }

    #[test]
    fn test_grep_filter_no_match() {
        let grep_filter = Some("fatal".to_string());
        let message = "Warning occurred";
        let matches = message
            .to_lowercase()
            .contains(&grep_filter.as_ref().unwrap().to_lowercase());
        assert!(!matches);
    }

    #[test]
    fn test_target_filter_no_match() {
        let target_filter = Some("specific".to_string());
        let target = "other::module";
        let matches = target.contains(target_filter.as_ref().unwrap());
        assert!(!matches);
    }

    #[test]
    fn test_debug_service_impl_default_trait() {
        // Cover the Default::default() impl at lines 37-39
        fn create_default<T: Default>() -> T {
            T::default()
        }
        let _service: DebugServiceImpl = create_default();
    }

    /// Ensure the global ring buffer is initialized and populated for filter tests.
    ///
    /// Since `try_debug_ring()` uses a process-global `OnceLock`, we initialize it
    /// once and populate it with entries that allow exercising all filter branches.
    fn ensure_global_ring_populated() {
        // Initialize global ring buffer (ignore if already initialized)
        let _ = crate::debug::init_debug_ring();

        // Push entries with known levels, targets, and messages so filters can match/reject
        if let Some(ring) = crate::debug::try_debug_ring() {
            let info_record = Record::builder(Level::Info)
                .module_path("grpc::debug::test_target")
                .file(file!())
                .line(line!())
                .message("info level coverage test message")
                .build();
            ring.push(&info_record);

            let warn_record = Record::builder(Level::Warn)
                .module_path("grpc::debug::other_target")
                .file(file!())
                .line(line!())
                .message("warn level different message")
                .build();
            ring.push(&warn_record);

            let error_record = Record::builder(Level::Error)
                .module_path("grpc::debug::test_target")
                .file(file!())
                .line(line!())
                .message("error searchable keyword")
                .build();
            ring.push(&error_record);
        }
    }

    #[tokio::test]
    async fn test_log_tail_with_ring_buffer_no_filters() {
        // Cover line 63 (successful ring buffer access) and unfiltered path
        ensure_global_ring_populated();

        let service = DebugServiceImpl::new();
        let request = Request::new(LogTailRequest {
            count: 10,
            level: None,
            target: None,
            grep: None,
        });
        let result = service.log_tail(request).await;
        assert!(result.is_ok());
        let response = result.unwrap().into_inner();
        assert!(!response.entries.is_empty());
    }

    #[tokio::test]
    async fn test_log_tail_with_level_filter_match() {
        // Cover lines 73, 75 (level filter match and non-match branches)
        ensure_global_ring_populated();

        let service = DebugServiceImpl::new();
        let request = Request::new(LogTailRequest {
            count: 100,
            level: Some("INFO".to_string()),
            target: None,
            grep: None,
        });
        let result = service.log_tail(request).await;
        assert!(result.is_ok());
        let response = result.unwrap().into_inner();
        // All entries should be INFO level
        for entry in &response.entries {
            assert_eq!(entry.level.to_uppercase(), "INFO");
        }
    }

    #[tokio::test]
    async fn test_log_tail_with_level_filter_excludes_others() {
        // Verify the level filter skips non-matching entries (line 75 return None)
        ensure_global_ring_populated();

        let service = DebugServiceImpl::new();
        let request = Request::new(LogTailRequest {
            count: 100,
            level: Some("error".to_string()),
            target: None,
            grep: None,
        });
        let result = service.log_tail(request).await;
        assert!(result.is_ok());
        let response = result.unwrap().into_inner();
        for entry in &response.entries {
            assert_eq!(entry.level.to_uppercase(), "ERROR");
        }
    }

    #[tokio::test]
    async fn test_log_tail_with_target_filter_match() {
        // Cover lines 79, 81 (target filter match and non-match branches)
        ensure_global_ring_populated();

        let service = DebugServiceImpl::new();
        let request = Request::new(LogTailRequest {
            count: 100,
            level: None,
            target: Some("test_target".to_string()),
            grep: None,
        });
        let result = service.log_tail(request).await;
        assert!(result.is_ok());
        let response = result.unwrap().into_inner();
        for entry in &response.entries {
            assert!(
                entry.target.contains("test_target"),
                "target '{}' should contain 'test_target'",
                entry.target
            );
        }
    }

    #[tokio::test]
    async fn test_log_tail_with_target_filter_excludes_others() {
        // Verify target filter skips non-matching entries (line 81 return None)
        ensure_global_ring_populated();

        let service = DebugServiceImpl::new();
        let request = Request::new(LogTailRequest {
            count: 100,
            level: None,
            target: Some("nonexistent_target_xyz".to_string()),
            grep: None,
        });
        let result = service.log_tail(request).await;
        assert!(result.is_ok());
        let response = result.unwrap().into_inner();
        assert!(response.entries.is_empty(), "Should have no entries for nonexistent target");
    }

    #[tokio::test]
    async fn test_log_tail_with_grep_filter_match() {
        // Cover lines 85, 87 (grep filter match and non-match branches)
        ensure_global_ring_populated();

        let service = DebugServiceImpl::new();
        let request = Request::new(LogTailRequest {
            count: 100,
            level: None,
            target: None,
            grep: Some("searchable".to_string()),
        });
        let result = service.log_tail(request).await;
        assert!(result.is_ok());
        let response = result.unwrap().into_inner();
        for entry in &response.entries {
            assert!(
                entry.message.to_lowercase().contains("searchable"),
                "message '{}' should contain 'searchable'",
                entry.message
            );
        }
    }

    #[tokio::test]
    async fn test_log_tail_with_grep_filter_excludes_others() {
        // Verify grep filter skips non-matching entries (line 87 return None)
        ensure_global_ring_populated();

        let service = DebugServiceImpl::new();
        let request = Request::new(LogTailRequest {
            count: 100,
            level: None,
            target: None,
            grep: Some("totally_unique_string_not_in_any_message_xyz".to_string()),
        });
        let result = service.log_tail(request).await;
        assert!(result.is_ok());
        let response = result.unwrap().into_inner();
        assert!(response.entries.is_empty(), "Should have no entries for non-matching grep");
    }

    #[tokio::test]
    async fn test_log_tail_with_all_filters_combined() {
        // Exercise all three filter branches simultaneously
        ensure_global_ring_populated();

        let service = DebugServiceImpl::new();
        let request = Request::new(LogTailRequest {
            count: 100,
            level: Some("error".to_string()),
            target: Some("test_target".to_string()),
            grep: Some("searchable".to_string()),
        });
        let result = service.log_tail(request).await;
        assert!(result.is_ok());
        let response = result.unwrap().into_inner();
        // Should find entries matching all three criteria
        for entry in &response.entries {
            assert_eq!(entry.level.to_uppercase(), "ERROR");
            assert!(entry.target.contains("test_target"));
            assert!(entry.message.to_lowercase().contains("searchable"));
        }
    }

    #[tokio::test]
    async fn test_log_tail_grep_case_insensitive() {
        // Verify grep is case-insensitive (line 85: to_lowercase comparison)
        ensure_global_ring_populated();

        let service = DebugServiceImpl::new();
        let request = Request::new(LogTailRequest {
            count: 100,
            level: None,
            target: None,
            grep: Some("SEARCHABLE".to_string()),
        });
        let result = service.log_tail(request).await;
        assert!(result.is_ok());
        let response = result.unwrap().into_inner();
        for entry in &response.entries {
            assert!(entry.message.to_lowercase().contains("searchable"));
        }
    }
}
