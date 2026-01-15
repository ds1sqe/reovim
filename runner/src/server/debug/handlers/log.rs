//! Debug log handlers.
//!
//! These handlers provide log access for debugging.

use reovim_protocol::v1::{
    LogEntryResult, LogLevelParams, LogLevelResult, LogTailParams, LogTailResult, RpcError,
};

use crate::server::rpc::{HandlerFuture, RpcContext};

use super::super::infrastructure::{current_log_level, log_buffer};

/// Handle `debug/log_level` request.
///
/// Get or set the current log level.
#[must_use]
pub fn debug_log_level(_ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: LogLevelParams = serde_json::from_value(params).unwrap_or_default();

        // For now, we only support getting the level (setting requires tracing reload)
        let level = current_log_level();

        let result = if params.level.is_some() {
            // Setting level is not yet implemented
            LogLevelResult {
                level: level.clone(),
                previous: Some(level),
            }
        } else {
            LogLevelResult {
                level,
                previous: None,
            }
        };

        serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
    })
}

/// Handle `debug/log_tail` request.
///
/// Get the last N log entries.
#[must_use]
pub fn debug_log_tail(_ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: LogTailParams = serde_json::from_value(params).unwrap_or_default();

        let buffer = log_buffer();
        let entries = buffer.tail(params.count);

        let entries: Vec<LogEntryResult> = entries
            .into_iter()
            .map(|e| LogEntryResult {
                timestamp: e.timestamp_iso(),
                level: e.level,
                target: e.target,
                message: e.message,
            })
            .collect();

        let result = LogTailResult {
            entries,
            overflow_count: buffer.overflow_count(),
        };

        serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
    })
}

#[cfg(test)]
mod tests {
    use {super::*, crate::server::rpc::handlers::test_utils::test_ctx};

    #[tokio::test]
    async fn test_debug_log_level_get() {
        let ctx = test_ctx();

        let result = debug_log_level(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("level").is_some());
    }

    #[tokio::test]
    async fn test_debug_log_tail() {
        let ctx = test_ctx();

        let result = debug_log_tail(ctx, serde_json::json!({ "count": 10 })).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("entries").is_some());
        assert!(value.get("overflow_count").is_some());
    }
}
