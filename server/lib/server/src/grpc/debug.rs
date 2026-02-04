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

        let ring = try_debug_ring()
            .ok_or_else(|| Status::unavailable("Debug ring buffer not initialized"))?;

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
}
