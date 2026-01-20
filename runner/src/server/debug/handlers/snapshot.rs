//! Visual snapshot handler.
//!
//! Provides a comprehensive state dump for AI/tooling integration.

use {
    reovim_kernel::api::v1::{
        MarksSnapshot, RegistersSnapshot, YankTypeSnapshot, snapshot_kernel_state, snapshot_marks,
        snapshot_mode_stack, snapshot_registers,
    },
    reovim_protocol::v1::{
        MarkEntry, MarksResult, MetricEntry, MetricsResult, ModeStackResult, Position,
        RegisterEntry, RegistersResult, RpcError, SnapshotBufferInfo, SnapshotBuffersSection,
        SnapshotEditorSection, SnapshotServerSection, SnapshotUiSection, SnapshotVimSection,
        VisualSnapshotResult, YankType,
    },
};

use crate::server::rpc::{HandlerFuture, RpcContext};

use super::super::infrastructure::{
    snapshot_handler_metrics, start_time_iso, total_requests, uptime_seconds,
};

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert kernel `YankTypeSnapshot` to protocol `YankType`.
const fn to_protocol_yank_type(yt: YankTypeSnapshot) -> YankType {
    match yt {
        YankTypeSnapshot::Characterwise => YankType::Characterwise,
        YankTypeSnapshot::Linewise => YankType::Linewise,
    }
}

/// Convert kernel `RegistersSnapshot` to protocol `RegistersResult`.
fn registers_to_result(snapshot: RegistersSnapshot) -> RegistersResult {
    RegistersResult {
        unnamed: RegisterEntry {
            name: snapshot.unnamed.name.to_string(),
            content: snapshot.unnamed.text.clone(),
            content_length: snapshot.unnamed.text.len(),
            yank_type: to_protocol_yank_type(snapshot.unnamed.yank_type),
        },
        named: snapshot
            .named
            .into_iter()
            .map(|r| RegisterEntry {
                name: r.name.to_string(),
                content: r.text.clone(),
                content_length: r.text.len(),
                yank_type: to_protocol_yank_type(r.yank_type),
            })
            .collect(),
    }
}

/// Convert kernel `MarksSnapshot` to protocol `MarksResult`.
fn marks_to_result(snapshot: MarksSnapshot) -> MarksResult {
    MarksResult {
        local: snapshot
            .local
            .into_iter()
            .map(|m| MarkEntry {
                name: m.name,
                position: Position::new(m.position.line, m.position.column),
                buffer_id: None,
            })
            .collect(),
        global: snapshot
            .global
            .into_iter()
            .map(|m| MarkEntry {
                name: m.name,
                position: Position::new(m.position.line, m.position.column),
                buffer_id: m.buffer_id.map(reovim_kernel::api::v1::BufferId::as_usize),
            })
            .collect(),
        special: snapshot
            .special
            .into_iter()
            .map(|m| MarkEntry {
                name: m.name,
                position: Position::new(m.position.line, m.position.column),
                buffer_id: m.buffer_id.map(reovim_kernel::api::v1::BufferId::as_usize),
            })
            .collect(),
    }
}

/// Build metrics result from infrastructure data.
fn build_metrics() -> MetricsResult {
    let handler_snapshots = snapshot_handler_metrics();
    MetricsResult {
        uptime_seconds: uptime_seconds(),
        total_requests: total_requests(),
        counters: vec![
            MetricEntry {
                name: "total_rpc_requests".to_string(),
                value: total_requests(),
            },
            MetricEntry {
                name: "handler_count".to_string(),
                value: handler_snapshots.len() as u64,
            },
        ],
    }
}

// ============================================================================
// Handler
// ============================================================================

/// Handle `debug/visual_snapshot` request.
///
/// Returns a comprehensive state dump for AI/tooling.
#[must_use]
pub fn debug_visual_snapshot(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let session_id = ctx.session.id().to_string();
        let client_count = ctx.session.clients().len();
        let timestamp = start_time_iso();

        // Gather all state information
        let (registers_result, marks_result, mode_stack_result, cursor_pos, buffers_section) = ctx
            .session
            .with_state(|state| {
                // Registers
                let registers_result = {
                    let registers = state.app.kernel.registers.read();
                    registers_to_result(snapshot_registers(&registers))
                };

                // Marks
                let marks_result = {
                    let marks = state.app.kernel.marks.read();
                    marks_to_result(snapshot_marks(&marks))
                };

                // Mode stack (from driver_session SSOT)
                let mode_snapshot = snapshot_mode_stack(state.mode_stack());
                let mode_stack_result = ModeStackResult {
                    current: mode_snapshot.current,
                    stack: mode_snapshot.stack,
                    depth: mode_snapshot.depth,
                };

                // Cursor position (use driver_session as SSOT for active_buffer)
                let cursor_pos = state
                    .session_active_buffer()
                    .and_then(|id| state.app.kernel.buffers.get(id))
                    .map_or_else(
                        || Position::new(0, 0),
                        |buffer_arc: std::sync::Arc<
                            reovim_arch::sync::RwLock<reovim_kernel::api::v1::Buffer>,
                        >| {
                            let pos = buffer_arc.read().position();
                            Position::new(pos.line, pos.column)
                        },
                    );

                // Buffers
                let kernel_snapshot = snapshot_kernel_state(&state.app.kernel);
                let buffer_infos: Vec<SnapshotBufferInfo> = kernel_snapshot
                    .buffer_ids
                    .iter()
                    .filter_map(|id| {
                        state.app.kernel.buffers.get(*id).map(|buffer_arc| {
                            let buffer = buffer_arc.read();
                            let line_count = buffer.line_count();
                            let preview: Vec<String> = (0..std::cmp::min(5, line_count))
                                .filter_map(|i| buffer.line(i).map(ToString::to_string))
                                .collect();
                            SnapshotBufferInfo {
                                id: id.as_usize(),
                                file_path: buffer.file_path().map(ToString::to_string),
                                modified: buffer.is_modified(),
                                line_count,
                                preview,
                            }
                        })
                    })
                    .collect();

                let buffers_section = SnapshotBuffersSection {
                    count: kernel_snapshot.buffer_count,
                    // Use driver_session as SSOT for active_buffer
                    active_id: state
                        .session_active_buffer()
                        .map(reovim_kernel::api::v1::BufferId::as_usize),
                    buffers: buffer_infos,
                };

                (registers_result, marks_result, mode_stack_result, cursor_pos, buffers_section)
            })
            .await;

        // Build result
        let result = VisualSnapshotResult {
            schema_version: "1.0.0".to_string(),
            timestamp,
            server: SnapshotServerSection {
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_seconds: uptime_seconds(),
                session_id,
                client_count,
            },
            editor: SnapshotEditorSection {
                mode: mode_stack_result.current.clone(),
                cursor: cursor_pos,
                selection: None,
            },
            buffers: buffers_section,
            ui: SnapshotUiSection {
                width: 80,
                height: 24,
                ascii_art: "[No screen rendering available in headless mode]".to_string(),
            },
            vim: SnapshotVimSection {
                registers: registers_result,
                marks: marks_result,
                mode_stack: mode_stack_result,
            },
            metrics: build_metrics(),
        };

        serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
    })
}

#[cfg(test)]
mod tests {
    use {super::*, crate::server::rpc::handlers::test_utils::test_ctx};

    #[tokio::test]
    async fn test_debug_visual_snapshot() {
        // Initialize infrastructure
        super::super::super::infrastructure::init_server_start();

        let ctx = test_ctx();

        let result = debug_visual_snapshot(ctx, serde_json::Value::Null).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("schema_version").is_some());
        assert!(value.get("timestamp").is_some());
        assert!(value.get("server").is_some());
        assert!(value.get("editor").is_some());
        assert!(value.get("buffers").is_some());
        assert!(value.get("ui").is_some());
        assert!(value.get("vim").is_some());
        assert!(value.get("metrics").is_some());
    }
}
