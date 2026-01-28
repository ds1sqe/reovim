//! Debug metrics handlers.
//!
//! These handlers provide performance metrics for debugging.

use reovim_protocol::v1::{HandlerStats, HandlersResult, MetricEntry, MetricsResult, RpcError};

use crate::server::rpc::{HandlerFuture, RpcContext};

use super::super::infrastructure::{snapshot_handler_metrics, total_requests, uptime_seconds};

/// Handle `debug/metrics` request.
///
/// Returns performance metrics summary.
#[must_use]
pub fn debug_metrics(_ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let result = MetricsResult {
            uptime_seconds: uptime_seconds(),
            total_requests: total_requests(),
            counters: vec![MetricEntry {
                name: "total_rpc_requests".to_string(),
                value: total_requests(),
            }],
        };

        serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
    })
}

/// Handle `debug/handlers` request.
///
/// Returns per-handler call statistics.
#[must_use]
pub fn debug_handlers(_ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let snapshots = snapshot_handler_metrics();

        let handlers: Vec<HandlerStats> = snapshots
            .into_iter()
            .map(|s| HandlerStats {
                method: s.method,
                call_count: s.call_count,
                total_micros: s.total_micros,
                avg_micros: s.avg_micros,
            })
            .collect();

        let result = HandlersResult { handlers };

        serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
    })
}

#[cfg(test)]
mod tests {
    use {super::*, crate::server::rpc::handlers::test_utils::test_ctx};

    #[tokio::test]
    async fn test_debug_metrics() {
        // Initialize start time for uptime
        super::super::super::infrastructure::init_server_start();

        let ctx = test_ctx();

        let result = debug_metrics(ctx, serde_json::Value::Null).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("uptime_seconds").is_some());
        assert!(value.get("total_requests").is_some());
        assert!(value.get("counters").is_some());
    }

    #[tokio::test]
    async fn test_debug_handlers() {
        let ctx = test_ctx();

        let result = debug_handlers(ctx, serde_json::Value::Null).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("handlers").is_some());
    }
}
