//! Command implementations for reo-cli

use {
    crate::client::{ClientError, ReoClient},
    serde_json::{Value, json},
};

/// Inject key sequence
pub async fn cmd_keys(client: &mut ReoClient, keys: &str) -> Result<Value, ClientError> {
    client.send("input/keys", json!({ "keys": keys })).await
}

/// Get current mode
pub async fn cmd_mode(client: &mut ReoClient) -> Result<Value, ClientError> {
    client.send("state/mode", json!({})).await
}

/// Get cursor position
pub async fn cmd_cursor(
    client: &mut ReoClient,
    buffer_id: Option<u64>,
) -> Result<Value, ClientError> {
    let params = buffer_id.map_or_else(|| json!({}), |id| json!({ "buffer_id": id }));
    client.send("state/cursor", params).await
}

/// Get selection state
pub async fn cmd_selection(
    client: &mut ReoClient,
    buffer_id: Option<u64>,
) -> Result<Value, ClientError> {
    let params = buffer_id.map_or_else(|| json!({}), |id| json!({ "buffer_id": id }));
    client.send("state/selection", params).await
}

/// Get screen dimensions
pub async fn cmd_screen(client: &mut ReoClient) -> Result<Value, ClientError> {
    client.send("state/screen", json!({})).await
}

/// Get rendered screen content
pub async fn cmd_screen_content(
    client: &mut ReoClient,
    format: &str,
) -> Result<Value, ClientError> {
    client
        .send("state/screen_content", json!({ "format": format }))
        .await
}

/// List all buffers
pub async fn cmd_buffer_list(client: &mut ReoClient) -> Result<Value, ClientError> {
    client.send("buffer/list", json!({})).await
}

/// Get buffer content
pub async fn cmd_buffer_content(
    client: &mut ReoClient,
    buffer_id: Option<u64>,
) -> Result<Value, ClientError> {
    let params = buffer_id.map_or_else(|| json!({}), |id| json!({ "buffer_id": id }));
    client.send("buffer/get_content", params).await
}

/// Set buffer content
pub async fn cmd_buffer_set_content(
    client: &mut ReoClient,
    content: &str,
    buffer_id: Option<u64>,
) -> Result<Value, ClientError> {
    let mut params = json!({ "content": content });
    if let Some(id) = buffer_id {
        params["buffer_id"] = json!(id);
    }
    client.send("buffer/set_content", params).await
}

/// Open a file
pub async fn cmd_buffer_open(client: &mut ReoClient, path: &str) -> Result<Value, ClientError> {
    client
        .send("buffer/open_file", json!({ "path": path }))
        .await
}

/// Resize the editor
pub async fn cmd_resize(
    client: &mut ReoClient,
    width: u64,
    height: u64,
) -> Result<Value, ClientError> {
    client
        .send("editor/resize", json!({ "width": width, "height": height }))
        .await
}

/// Quit the editor
pub async fn cmd_quit(client: &mut ReoClient) -> Result<Value, ClientError> {
    client.send("editor/quit", json!({})).await
}

/// Kill the server (force shutdown)
pub async fn cmd_kill(client: &mut ReoClient) -> Result<Value, ClientError> {
    client.send("server/kill", json!({})).await
}

/// Send raw JSON-RPC request
pub async fn cmd_raw(client: &mut ReoClient, json_str: &str) -> Result<Value, ClientError> {
    client.send_raw(json_str).await
}
