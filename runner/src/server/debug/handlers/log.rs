//! Debug log handlers.
//!
//! These handlers provide log access for debugging.

use reovim_protocol::v1::{
    LogEntryResult, LogLevel, LogLevelParams, LogLevelResult, LogSubscribeParams,
    LogSubscribeResult, LogTailParams, LogTailResult, LogUnsubscribeParams, LogUnsubscribeResult,
    RpcError,
};

use crate::server::rpc::{HandlerFuture, RpcContext};

use super::super::infrastructure::{get_current_level, log_buffer, log_subscribers, set_log_level};

/// Handle `debug/log_level` request.
///
/// Get or set the current log level.
#[must_use]
pub fn debug_log_level(_ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: LogLevelParams = serde_json::from_value(params).unwrap_or_default();

        params.level.map_or_else(
            || {
                // Just return current level
                let level = get_current_level();
                let result = LogLevelResult {
                    level,
                    previous: None,
                };
                serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
            },
            |new_level| {
                // Set the new log level
                match set_log_level(&new_level) {
                    Ok(previous) => {
                        let result = LogLevelResult {
                            level: new_level,
                            previous: Some(previous),
                        };
                        serde_json::to_value(result)
                            .map_err(|e| RpcError::internal_error(e.to_string()))
                    }
                    Err(e) => Err(RpcError::invalid_params(e.to_string())),
                }
            },
        )
    })
}

/// Handle `debug/log_tail` request.
///
/// Get the last N log entries with optional filtering.
#[must_use]
pub fn debug_log_tail(_ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: LogTailParams = serde_json::from_value(params).unwrap_or_default();

        let buffer = log_buffer();

        // Get more entries than requested if filtering (to account for filtered out)
        // Cap at 5000 to prevent memory spikes with aggressive filtering
        let has_filters =
            params.level.is_some() || params.target.is_some() || params.grep.is_some();
        let fetch_count = if has_filters {
            (params.count * 10).min(5000)
        } else {
            params.count
        };
        let mut entries = buffer.tail(fetch_count);

        // Apply filters
        if let Some(ref level_filter) = params.level {
            let min_level = LogLevel::from_str_lossy(level_filter);
            entries.retain(|e| LogLevel::from_str_lossy(&e.level) >= min_level);
        }
        if let Some(ref target_filter) = params.target {
            entries.retain(|e| e.target.contains(target_filter));
        }
        if let Some(ref grep) = params.grep {
            let grep_lower = grep.to_lowercase();
            entries.retain(|e| e.message.to_lowercase().contains(&grep_lower));
        }

        // Limit to requested count after filtering
        entries.truncate(params.count);

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

/// Handle `debug/log_subscribe` request.
///
/// Subscribe to real-time log streaming.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // Handler signature is fixed by HandlerFn type
pub fn debug_log_subscribe(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    let client = ctx.client;
    Box::pin(async move {
        let params: LogSubscribeParams = serde_json::from_value(params).unwrap_or_default();

        let level_filter = params.level.map(|s| LogLevel::from_str_lossy(&s));

        let subscription_id = log_subscribers().subscribe(&client, level_filter);

        let result = LogSubscribeResult { subscription_id };

        serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
    })
}

/// Handle `debug/log_unsubscribe` request.
///
/// Unsubscribe from log streaming.
#[must_use]
pub fn debug_log_unsubscribe(_ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: LogUnsubscribeParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        let success = log_subscribers().unsubscribe(params.subscription_id);

        let result = LogUnsubscribeResult { success };

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

    #[tokio::test]
    async fn test_debug_log_tail_with_level_filter() {
        let ctx = test_ctx();

        let result = debug_log_tail(ctx, serde_json::json!({ "count": 50, "level": "warn" })).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("entries").is_some());
    }

    #[tokio::test]
    async fn test_debug_log_tail_with_target_filter() {
        let ctx = test_ctx();

        let result =
            debug_log_tail(ctx, serde_json::json!({ "count": 50, "target": "runner" })).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("entries").is_some());
    }

    #[tokio::test]
    async fn test_debug_log_tail_with_grep_filter() {
        let ctx = test_ctx();

        let result = debug_log_tail(ctx, serde_json::json!({ "count": 50, "grep": "error" })).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("entries").is_some());
    }

    #[tokio::test]
    async fn test_debug_log_tail_with_combined_filters() {
        let ctx = test_ctx();

        let result = debug_log_tail(
            ctx,
            serde_json::json!({
                "count": 50,
                "level": "info",
                "target": "runner",
                "grep": "test"
            }),
        )
        .await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("entries").is_some());
    }

    #[tokio::test]
    async fn test_debug_log_tail_boundary_count_zero() {
        let ctx = test_ctx();

        let result = debug_log_tail(ctx, serde_json::json!({ "count": 0 })).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        let entries = value.get("entries").unwrap().as_array().unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn test_debug_log_tail_boundary_count_one() {
        let ctx = test_ctx();

        let result = debug_log_tail(ctx, serde_json::json!({ "count": 1 })).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        let entries = value.get("entries").unwrap().as_array().unwrap();
        assert!(entries.len() <= 1);
    }

    #[tokio::test]
    async fn test_debug_log_subscribe() {
        let ctx = test_ctx();

        let result = debug_log_subscribe(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("subscription_id").is_some());
    }

    #[tokio::test]
    async fn test_debug_log_subscribe_with_level() {
        let ctx = test_ctx();

        let result = debug_log_subscribe(ctx, serde_json::json!({ "level": "warn" })).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        let sub_id = value.get("subscription_id").unwrap().as_u64().unwrap();
        assert!(sub_id > 0);
    }

    #[tokio::test]
    async fn test_debug_log_unsubscribe_valid() {
        let ctx = test_ctx();

        // First subscribe
        let result = debug_log_subscribe(ctx.clone(), serde_json::json!({})).await;
        let sub_id = result
            .unwrap()
            .get("subscription_id")
            .unwrap()
            .as_u64()
            .unwrap();

        // Then unsubscribe
        let result =
            debug_log_unsubscribe(ctx, serde_json::json!({ "subscription_id": sub_id })).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("success").unwrap().as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_debug_log_unsubscribe_invalid() {
        let ctx = test_ctx();

        let result =
            debug_log_unsubscribe(ctx, serde_json::json!({ "subscription_id": 99999 })).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(!value.get("success").unwrap().as_bool().unwrap());
    }
}
