//! CLI command implementations.
//!
//! Each function executes a CLI command and formats the output.

use std::fmt::Write;

use crate::{GrpcClient, GrpcClientError, OutputFormat};

/// Send keys to the editor.
///
/// # Errors
///
/// Returns an error if the gRPC call fails.
pub async fn keys(
    client: &mut GrpcClient,
    keys: &str,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let response = client.send_keys(keys).await?;

    let status_str = match response.status {
        1 => "executed",
        2 => "pending",
        3 => "not_found",
        _ => "unknown",
    };

    match format {
        OutputFormat::Plain => {
            if response.ok {
                Ok(format!("OK (status: {status_str})"))
            } else {
                Ok(format!("Failed (status: {status_str})"))
            }
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "ok": response.ok,
                "status": status_str,
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

/// Get current editor mode.
///
/// # Errors
///
/// Returns an error if the gRPC call fails.
pub async fn mode(
    client: &mut GrpcClient,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let response = client.get_mode().await?;

    match format {
        OutputFormat::Plain => Ok(format!(
            "{} ({}{})",
            response.display,
            response.name,
            if response.is_insert { ", insert" } else { "" }
        )),
        OutputFormat::Json => {
            let json = serde_json::json!({
                "name": response.name,
                "display": response.display,
                "is_insert": response.is_insert,
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

/// Get cursor position.
///
/// # Errors
///
/// Returns an error if the gRPC call fails.
pub async fn cursor(
    client: &mut GrpcClient,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let response = client.get_cursor().await?;

    let (line, column) = response
        .position
        .map_or((0, 0), |pos| (pos.line, pos.column));

    match format {
        OutputFormat::Plain => Ok(format!("{}:{}", line + 1, column + 1)), // 1-indexed for display
        OutputFormat::Json => {
            let json = serde_json::json!({
                "window_id": response.window_id,
                "line": line,
                "column": column,
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

/// List open buffers.
///
/// # Errors
///
/// Returns an error if the gRPC call fails.
pub async fn buffers(
    client: &mut GrpcClient,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let response = client.list_buffers().await?;

    match format {
        OutputFormat::Plain => {
            if response.buffers.is_empty() {
                return Ok("No buffers".to_string());
            }

            let mut output = String::new();
            for buf in &response.buffers {
                let modified = if buf.modified { " [+]" } else { "" };
                let _ = writeln!(
                    output,
                    "{}: {} ({} lines){}",
                    buf.id, buf.name, buf.line_count, modified
                );
            }
            Ok(output.trim_end().to_string())
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "buffers": response.buffers.iter().map(|b| serde_json::json!({
                    "id": b.id,
                    "name": b.name,
                    "path": b.path,
                    "line_count": b.line_count,
                    "modified": b.modified,
                })).collect::<Vec<_>>(),
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

/// Get buffer content.
///
/// # Errors
///
/// Returns an error if the gRPC call fails.
pub async fn buffer(
    client: &mut GrpcClient,
    id: Option<u64>,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let response = client.get_buffer_content(id).await?;

    match format {
        OutputFormat::Plain => {
            if response.lines.is_empty() {
                return Ok("(empty buffer)".to_string());
            }
            Ok(response.lines.join("\n"))
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "buffer_id": response.buffer_id,
                "lines": response.lines,
                "start_line": response.start_line,
                "total_lines": response.total_lines,
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

/// Ping the server.
///
/// # Errors
///
/// Returns an error if the gRPC call fails.
pub async fn ping(
    client: &mut GrpcClient,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let response = client.ping().await?;

    match format {
        OutputFormat::Plain => Ok(response.pong),
        OutputFormat::Json => {
            let json = serde_json::json!({
                "pong": response.pong,
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

/// Get server version and info.
///
/// # Errors
///
/// Returns an error if the gRPC call fails.
pub async fn version(
    client: &mut GrpcClient,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let response = client.info().await?;

    match format {
        OutputFormat::Plain => Ok(format!(
            "reovim {} (uptime: {}s, buffers: {}, clients: {}, modules: {})",
            response.version,
            response.uptime_secs,
            response.buffer_count,
            response.client_count,
            response.module_count
        )),
        OutputFormat::Json => {
            let json = serde_json::json!({
                "version": response.version,
                "uptime_secs": response.uptime_secs,
                "buffer_count": response.buffer_count,
                "client_count": response.client_count,
                "module_count": response.module_count,
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

/// Get register contents.
///
/// # Arguments
///
/// * `name` - Optional register name. If None, returns all non-empty registers.
///
/// # Errors
///
/// Returns an error if the gRPC call fails.
pub async fn registers(
    client: &mut GrpcClient,
    name: Option<String>,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let names = name.map_or_else(Vec::new, |n| vec![n]);
    let response = client.get_registers(names).await?;

    match format {
        OutputFormat::Plain => {
            if response.registers.is_empty() {
                return Ok("No registers set".to_string());
            }

            let mut output = String::new();
            for reg in &response.registers {
                // Truncate content for display (max 50 chars)
                let display_content = if reg.content.len() > 50 {
                    format!("{}...", &reg.content[..47])
                } else {
                    reg.content.clone()
                };
                // Escape newlines for single-line display
                let escaped = display_content.replace('\n', "\\n");
                let _ = writeln!(output, "\"{}: {} [{}]", reg.name, escaped, reg.yank_type);
            }
            Ok(output.trim_end().to_string())
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "registers": response.registers.iter().map(|r| serde_json::json!({
                    "name": r.name,
                    "content_type": r.content_type,
                    "content": r.content,
                    "yank_type": r.yank_type,
                })).collect::<Vec<_>>(),
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

/// Capture TUI screen content.
///
/// Requests a screen capture from the connected headless TUI via the server relay.
///
/// # Arguments
///
/// * `capture_format` - Capture format: `plain_text`, `raw_ansi`, or `cell_grid`
///
/// # Errors
///
/// Returns an error if the gRPC call fails, no TUI is connected, or capture times out.
pub async fn capture(
    client: &mut GrpcClient,
    capture_format: &str,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let response = client.get_screen_content(capture_format).await?;

    match format {
        OutputFormat::Plain => {
            // For plain output, just return the raw content
            Ok(response.content)
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "width": response.width,
                "height": response.height,
                "format": response.format,
                "content": response.content,
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

// =============================================================================
// Presence Commands (Phase 15)
// =============================================================================

/// Join the presence session.
///
/// # Errors
///
/// Returns an error if the gRPC call fails.
pub async fn presence_join(
    client: &mut GrpcClient,
    client_type: &str,
    name: &str,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let response = client.presence_join(client_type, name).await?;

    match format {
        OutputFormat::Plain => Ok(format!(
            "Joined as client {} ({})\nPeers: {}",
            response.client_id,
            name,
            response.peers.len()
        )),
        OutputFormat::Json => {
            let json = serde_json::json!({
                "client_id": response.client_id,
                "peers": response.peers.iter().map(|p| serde_json::json!({
                    "client_id": p.client_id,
                    "client_type": p.client_type,
                    "display_name": p.display_name,
                    "sync_mode": p.sync_mode,
                })).collect::<Vec<_>>(),
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

/// Leave the presence session.
///
/// # Errors
///
/// Returns an error if the gRPC call fails.
pub async fn presence_leave(
    client: &mut GrpcClient,
    client_id: u64,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let response = client.presence_leave(client_id).await?;

    match format {
        OutputFormat::Plain => {
            if response.ok {
                Ok("Left session".to_string())
            } else {
                Ok("Client not found".to_string())
            }
        }
        OutputFormat::Json => {
            let json = serde_json::json!({ "ok": response.ok });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

/// List all connected clients.
///
/// # Errors
///
/// Returns an error if the gRPC call fails.
pub async fn presence_list(
    client: &mut GrpcClient,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let response = client.presence_list().await?;

    match format {
        OutputFormat::Plain => {
            if response.clients.is_empty() {
                return Ok("No clients connected".to_string());
            }

            let mut output = format!("Connected clients: {}\n", response.clients.len());
            for c in &response.clients {
                let sync_mode_str = match c.sync_mode {
                    0 => "independent",
                    1 => "follow",
                    2 => "present",
                    _ => "unknown",
                };
                let cursor_str = c
                    .cursor
                    .as_ref()
                    .map_or_else(|| "?:?".to_string(), |p| format!("{}:{}", p.line, p.column));
                let _ = writeln!(
                    output,
                    "  {} ({}) - {} @ {} [{}]",
                    c.client_id, c.display_name, c.client_type, cursor_str, sync_mode_str
                );
            }
            Ok(output.trim_end().to_string())
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "clients": response.clients.iter().map(|c| serde_json::json!({
                    "client_id": c.client_id,
                    "client_type": c.client_type,
                    "display_name": c.display_name,
                    "buffer_id": c.buffer_id,
                    "cursor": c.cursor.as_ref().map(|p| serde_json::json!({
                        "line": p.line,
                        "column": p.column,
                    })),
                    "mode": c.mode,
                    "sync_mode": c.sync_mode,
                    "follow_target": c.follow_target,
                    "joined_at_ms": c.joined_at_ms,
                })).collect::<Vec<_>>(),
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

/// Update presence state.
///
/// # Errors
///
/// Returns an error if the gRPC call fails.
pub async fn presence_update(
    client: &mut GrpcClient,
    client_id: u64,
    buffer_id: Option<u64>,
    cursor_line: Option<u64>,
    cursor_column: Option<u64>,
    mode: Option<String>,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let response = client
        .presence_update(client_id, buffer_id, cursor_line, cursor_column, mode)
        .await?;

    match format {
        OutputFormat::Plain => {
            if response.ok {
                Ok("Presence updated".to_string())
            } else {
                Ok("Update failed (client not found?)".to_string())
            }
        }
        OutputFormat::Json => {
            let json = serde_json::json!({ "ok": response.ok });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

/// Set sync mode for a client.
///
/// # Arguments
///
/// * `sync_mode` - 0 = Independent, 1 = Follow, 2 = Present
///
/// # Errors
///
/// Returns an error if the gRPC call fails.
pub async fn presence_set_sync_mode(
    client: &mut GrpcClient,
    client_id: u64,
    sync_mode: i32,
    follow_target: Option<u64>,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let response = client
        .presence_set_sync_mode(client_id, sync_mode, follow_target)
        .await?;

    let mode_str = match sync_mode {
        0 => "independent",
        1 => "follow",
        2 => "present",
        _ => "unknown",
    };

    match format {
        OutputFormat::Plain => {
            if response.ok {
                Ok(format!("Set sync mode to {mode_str}"))
            } else {
                Ok("Failed to set sync mode".to_string())
            }
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "ok": response.ok,
                "mode": mode_str,
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests that don't require a running server

    #[test]
    fn test_output_format_eq() {
        assert_eq!(OutputFormat::Plain, OutputFormat::Plain);
        assert_eq!(OutputFormat::Json, OutputFormat::Json);
        assert_ne!(OutputFormat::Plain, OutputFormat::Json);
    }
}
