//! CLI command implementations.
//!
//! Each function corresponds to a CLI subcommand.

use serde_json::{Value, json};

use crate::client::common::rpc::{RpcClient, RpcClientError};

/// Inject key sequence.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_keys(client: &mut RpcClient, keys: &str) -> Result<Value, RpcClientError> {
    client.call("input/keys", json!({ "keys": keys })).await
}

/// Get current mode.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_mode(client: &mut RpcClient) -> Result<Value, RpcClientError> {
    client.call("state/mode", json!({})).await
}

/// Get cursor position.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_cursor(client: &mut RpcClient) -> Result<Value, RpcClientError> {
    client.call("state/cursor", json!({})).await
}

/// Get screen dimensions.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_screen(client: &mut RpcClient) -> Result<Value, RpcClientError> {
    client.call("state/screen", json!({})).await
}

/// Get rendered screen content.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_screen_content(
    client: &mut RpcClient,
    format: &str,
) -> Result<Value, RpcClientError> {
    client
        .call("state/screen_content", json!({ "format": format }))
        .await
}

/// List all buffers.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_buffer_list(client: &mut RpcClient) -> Result<Value, RpcClientError> {
    client.call("buffer/list", json!({})).await
}

/// Get buffer content.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_buffer_content(
    client: &mut RpcClient,
    buffer_id: Option<u64>,
) -> Result<Value, RpcClientError> {
    let params = buffer_id.map_or_else(|| json!({}), |id| json!({ "buffer_id": id }));
    client.call("buffer/get_content", params).await
}

/// Set buffer content.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_buffer_set_content(
    client: &mut RpcClient,
    content: &str,
    buffer_id: Option<u64>,
) -> Result<Value, RpcClientError> {
    let mut params = json!({ "content": content });
    if let Some(id) = buffer_id {
        params["buffer_id"] = json!(id);
    }
    client.call("buffer/set_content", params).await
}

/// Open a file.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_buffer_open(client: &mut RpcClient, path: &str) -> Result<Value, RpcClientError> {
    client
        .call("buffer/open_file", json!({ "path": path }))
        .await
}

/// Resize the editor.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_resize(
    client: &mut RpcClient,
    width: u64,
    height: u64,
) -> Result<Value, RpcClientError> {
    client
        .call("editor/resize", json!({ "width": width, "height": height }))
        .await
}

/// Quit the editor gracefully.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_quit(client: &mut RpcClient) -> Result<Value, RpcClientError> {
    client.call("editor/quit", json!({})).await
}

/// Kill the server (force shutdown).
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_kill(client: &mut RpcClient) -> Result<Value, RpcClientError> {
    client.call("server/kill", json!({})).await
}

/// Send raw JSON-RPC request.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_raw(client: &mut RpcClient, json_str: &str) -> Result<Value, RpcClientError> {
    client.send_raw(json_str).await
}

/// Get selection state.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_selection(
    client: &mut RpcClient,
    buffer_id: Option<u64>,
) -> Result<Value, RpcClientError> {
    let params = buffer_id.map_or_else(|| json!({}), |id| json!({ "buffer_id": id }));
    client.call("state/selection", params).await
}

/// List modules.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_module_list(client: &mut RpcClient) -> Result<Value, RpcClientError> {
    client.call("module/list", json!({})).await
}

/// Load a module.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_module_load(client: &mut RpcClient, path: &str) -> Result<Value, RpcClientError> {
    client.call("module/load", json!({ "path": path })).await
}

/// Unload a module.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_module_unload(client: &mut RpcClient, id: &str) -> Result<Value, RpcClientError> {
    client.call("module/unload", json!({ "id": id })).await
}

/// Reload a module.
///
/// # Errors
///
/// Returns error if RPC call fails.
pub async fn cmd_module_reload(client: &mut RpcClient, id: &str) -> Result<Value, RpcClientError> {
    client.call("module/reload", json!({ "id": id })).await
}
