//! Module lifecycle RPC handlers.
//!
//! Handlers for `module/list`, `module/load`, `module/unload`, `module/reload`.
//!
//! # Safety
//!
//! The `module/load` and `module/reload` handlers use unsafe code because
//! loading dynamic modules crosses the FFI boundary. The caller takes
//! responsibility for ensuring ABI compatibility.

use std::path::PathBuf;

use {
    reovim_arch::dirs,
    reovim_kernel::api::v1::{ModuleContext, ModuleId},
    reovim_protocol::v1::{
        ModuleInfo, ModuleListResult, ModuleLoadParams, ModuleLoadResult, ModuleReloadParams,
        ModuleUnloadParams, OkResult, RpcError,
    },
};

use super::super::dispatcher::{HandlerFuture, RpcContext};

/// Handler for `module/list` method.
///
/// Lists all loaded modules with their metadata.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "module/list", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"modules": [...]}}
/// ```
///
/// # Panics
///
/// This function will not panic as `ModuleListResult` serialization is infallible.
#[must_use]
pub fn module_list(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let modules = ctx
            .session
            .with_state(|state| {
                state
                    .module_registry
                    .registered_ids()
                    .iter()
                    .map(|id| {
                        let state_str = state
                            .module_registry
                            .state(id)
                            .map_or_else(|| "Unknown".to_string(), |s| format!("{s:?}"));
                        let path = state.module_registry.module_path(id);
                        let deps = state
                            .module_registry
                            .dependents_of(id)
                            .iter()
                            .map(|d| d.as_str().to_string())
                            .collect();

                        let is_static = path.is_none();
                        ModuleInfo {
                            id: id.as_str().to_string(),
                            name: id.as_str().to_string(), // Use ID as name for now
                            version: "0.1.0".to_string(),
                            state: state_str,
                            path: path.map(|p| p.to_string_lossy().to_string()),
                            is_static,
                            dependencies: deps,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .await;

        Ok(serde_json::to_value(ModuleListResult { modules })
            .expect("ModuleListResult serialization cannot fail"))
    })
}

/// Handler for `module/load` method.
///
/// Loads a dynamic module from a file path.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "module/load", "params": {"path": "/tmp/libexample.so"}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"module": {...}}}
/// ```
///
/// # Safety
///
/// Loading dynamic modules is inherently unsafe as it crosses the FFI boundary.
/// The caller must ensure the shared library is ABI-compatible.
///
/// # Panics
///
/// This function will not panic as `ModuleLoadResult` serialization is infallible.
#[must_use]
#[allow(unsafe_code)]
pub fn module_load(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: ModuleLoadParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        let path = std::path::Path::new(&params.path);

        // SAFETY: Loading dynamic modules is inherently unsafe (FFI boundary).
        // The RPC caller takes responsibility for ABI compatibility.
        let module_id = unsafe {
            ctx.session
                .with_state_mut(|state| state.module_registry.load_dynamic(path))
                .await
        }
        .map_err(|e| RpcError::internal_error(format!("Load failed: {e:?}")))?;

        // Build response with module info
        let info = ctx
            .session
            .with_state(|state| ModuleInfo {
                id: module_id.as_str().to_string(),
                name: module_id.as_str().to_string(),
                version: "0.1.0".to_string(),
                state: state
                    .module_registry
                    .state(&module_id)
                    .map_or_else(|| "Loaded".to_string(), |s| format!("{s:?}")),
                path: Some(params.path.clone()),
                is_static: false,
                dependencies: vec![],
            })
            .await;

        Ok(serde_json::to_value(ModuleLoadResult { module: info })
            .expect("ModuleLoadResult serialization cannot fail"))
    })
}

/// Handler for `module/unload` method.
///
/// Unloads a module by ID. Fails if other modules depend on it.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "module/unload", "params": {"id": "example"}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"ok": true}}
/// ```
///
/// # Panics
///
/// This function will not panic as `OkResult` serialization is infallible.
#[must_use]
pub fn module_unload(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: ModuleUnloadParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        let module_id = ModuleId::from_string(params.id.clone());

        ctx.session
            .with_state_mut(|state| state.module_registry.unload(&module_id))
            .await
            .map_err(|e| RpcError::internal_error(format!("Unload failed: {e:?}")))?;

        Ok(serde_json::to_value(OkResult::new()).expect("OkResult serialization cannot fail"))
    })
}

/// Handler for `module/reload` method.
///
/// Atomically reloads a dynamic module, preserving state if possible.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "module/reload", "params": {"id": "example"}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"ok": true}}
/// ```
///
/// # Safety
///
/// Reloading involves unloading and loading at the FFI boundary.
/// The caller must ensure the new shared library is ABI-compatible.
///
/// # Panics
///
/// This function will not panic as `OkResult` serialization is infallible.
#[must_use]
#[allow(unsafe_code)]
pub fn module_reload(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: ModuleReloadParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        let module_id = ModuleId::from_string(params.id.clone());

        // Build ModuleContext for re-initialization
        // Use default paths; the registry will create per-module subdirectories
        let module_ctx = ctx
            .session
            .with_state(|state| {
                let data_dir = dirs::data_local_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("reovim");
                let cache_dir = dirs::cache_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("reovim");
                ModuleContext::new(state.app.kernel.clone(), data_dir, cache_dir)
            })
            .await;

        // SAFETY: Reloading involves unload + load at FFI boundary.
        // The RPC caller takes responsibility for ABI compatibility.
        unsafe {
            ctx.session
                .with_state_mut(|state| {
                    state.module_registry.reload_atomic(&module_id, &module_ctx)
                })
                .await
        }
        .map_err(|e| RpcError::internal_error(format!("Reload failed: {e:?}")))?;

        Ok(serde_json::to_value(OkResult::new()).expect("OkResult serialization cannot fail"))
    })
}

#[cfg(test)]
mod tests {
    use {super::*, crate::server::rpc::handlers::test_utils::test_ctx};

    #[tokio::test]
    async fn test_module_list_empty() {
        let ctx = test_ctx();

        let result = module_list(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("modules").is_some());
    }

    #[tokio::test]
    async fn test_module_load_invalid_path() {
        let ctx = test_ctx();

        // Non-existent path should fail
        let result = module_load(ctx, serde_json::json!({"path": "/nonexistent/module.so"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_module_unload_nonexistent() {
        let ctx = test_ctx();

        // Unloading non-existent module is idempotent (no-op, succeeds)
        let result = module_unload(ctx, serde_json::json!({"id": "nonexistent"})).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_module_reload_nonexistent() {
        let ctx = test_ctx();

        // Reloading non-existent module should fail
        let result = module_reload(ctx, serde_json::json!({"id": "nonexistent"})).await;
        assert!(result.is_err());
    }
}
