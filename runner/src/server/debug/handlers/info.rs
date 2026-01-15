//! Debug info handlers (version, uptime).
//!
//! These handlers provide server runtime information.

use {
    crate::server::rpc::{HandlerFuture, RpcContext},
    reovim_protocol::v1::{UptimeResult, VersionResult},
};

use super::super::infrastructure::{start_time_iso, uptime_human, uptime_seconds};

/// Handle `debug/version` request.
///
/// Returns server version information including git hash and build date.
#[must_use]
pub fn debug_version(_ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let result = VersionResult {
            version: env!("CARGO_PKG_VERSION").to_string(),
            git_hash: option_env!("REOVIM_GIT_HASH").map(ToString::to_string),
            build_date: option_env!("REOVIM_BUILD_DATE").map(ToString::to_string),
            rust_version: option_env!("REOVIM_RUST_VERSION")
                .unwrap_or("unknown")
                .to_string(),
        };

        serde_json::to_value(result)
            .map_err(|e| reovim_protocol::v1::RpcError::internal_error(e.to_string()))
    })
}

/// Handle `debug/uptime` request.
///
/// Returns server uptime in various formats.
#[must_use]
pub fn debug_uptime(_ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let result = UptimeResult {
            uptime_seconds: uptime_seconds(),
            uptime_human: uptime_human(),
            start_time: start_time_iso(),
        };

        serde_json::to_value(result)
            .map_err(|e| reovim_protocol::v1::RpcError::internal_error(e.to_string()))
    })
}

#[cfg(test)]
mod tests {
    use {super::*, crate::server::rpc::handlers::test_utils::test_ctx};

    #[tokio::test]
    async fn test_debug_version() {
        let ctx = test_ctx();

        let result = debug_version(ctx, serde_json::Value::Null).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("version").is_some());
        assert!(value.get("rust_version").is_some());
    }

    #[tokio::test]
    async fn test_debug_uptime() {
        // Initialize start time for test
        super::super::super::infrastructure::init_server_start();

        let ctx = test_ctx();

        let result = debug_uptime(ctx, serde_json::Value::Null).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("uptime_seconds").is_some());
        assert!(value.get("uptime_human").is_some());
        assert!(value.get("start_time").is_some());
    }
}
