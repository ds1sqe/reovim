//! Debug inspection handlers (kernel state, registers, marks, mode stack).
//!
//! These handlers provide deep state inspection for debugging.

use {
    reovim_kernel::api::v1::{
        YankTypeSnapshot, snapshot_kernel_state, snapshot_marks, snapshot_mode_stack,
        snapshot_registers,
    },
    reovim_protocol::v1::{
        KernelStateResult, MarkEntry, MarksResult, ModeStackResult, RegisterEntry, RegistersResult,
        RpcError, YankType,
    },
};

use crate::server::rpc::{HandlerFuture, RpcContext};

/// Convert kernel `YankTypeSnapshot` to protocol `YankType`.
const fn to_protocol_yank_type(yt: YankTypeSnapshot) -> YankType {
    match yt {
        YankTypeSnapshot::Characterwise => YankType::Characterwise,
        YankTypeSnapshot::Linewise => YankType::Linewise,
    }
}

/// Handle `debug/kernel_state` request.
///
/// Returns kernel state summary including buffer count and event handlers.
#[must_use]
pub fn debug_kernel_state(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let result = ctx
            .session
            .with_state(|state| {
                let snapshot = snapshot_kernel_state(&state.app.kernel);
                let active_buffer = state
                    .app
                    .active_buffer
                    .map(reovim_kernel::api::v1::BufferId::as_usize);

                KernelStateResult {
                    buffer_count: snapshot.buffer_count,
                    active_buffer,
                    buffer_ids: snapshot
                        .buffer_ids
                        .into_iter()
                        .map(reovim_kernel::api::v1::BufferId::as_usize)
                        .collect(),
                    event_handlers: snapshot.event_handlers,
                    event_queue_len: snapshot.event_queue_len,
                }
            })
            .await;

        serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
    })
}

/// Handle `debug/registers` request.
///
/// Returns register contents.
#[must_use]
pub fn debug_registers(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let result = ctx
            .session
            .with_state(|state| {
                let snapshot = {
                    let registers = state.app.kernel.registers.read();
                    snapshot_registers(&registers)
                };

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
            })
            .await;

        serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
    })
}

/// Handle `debug/marks` request.
///
/// Returns mark contents.
#[must_use]
pub fn debug_marks(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let result = ctx
            .session
            .with_state(|state| {
                let snapshot = {
                    let marks = state.app.kernel.marks.read();
                    snapshot_marks(&marks)
                };

                MarksResult {
                    local: snapshot
                        .local
                        .into_iter()
                        .map(|m| MarkEntry {
                            name: m.name,
                            position: reovim_protocol::v1::Position::new(
                                m.position.line,
                                m.position.column,
                            ),
                            buffer_id: None,
                        })
                        .collect(),
                    global: snapshot
                        .global
                        .into_iter()
                        .map(|m| MarkEntry {
                            name: m.name,
                            position: reovim_protocol::v1::Position::new(
                                m.position.line,
                                m.position.column,
                            ),
                            buffer_id: m.buffer_id.map(reovim_kernel::api::v1::BufferId::as_usize),
                        })
                        .collect(),
                    special: snapshot
                        .special
                        .into_iter()
                        .map(|m| MarkEntry {
                            name: m.name,
                            position: reovim_protocol::v1::Position::new(
                                m.position.line,
                                m.position.column,
                            ),
                            buffer_id: m.buffer_id.map(reovim_kernel::api::v1::BufferId::as_usize),
                        })
                        .collect(),
                }
            })
            .await;

        serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
    })
}

/// Handle `debug/mode_stack` request.
///
/// Returns mode stack information.
#[must_use]
pub fn debug_mode_stack(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let result = ctx
            .session
            .with_state(|state| {
                let snapshot = snapshot_mode_stack(&state.app.mode_stack);

                ModeStackResult {
                    current: snapshot.current,
                    stack: snapshot.stack,
                    depth: snapshot.depth,
                }
            })
            .await;

        serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
    })
}

#[cfg(test)]
mod tests {
    use {super::*, crate::server::rpc::handlers::test_utils::test_ctx};

    #[tokio::test]
    async fn test_debug_kernel_state() {
        let ctx = test_ctx();

        let result = debug_kernel_state(ctx, serde_json::Value::Null).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("buffer_count").is_some());
        assert!(value.get("event_handlers").is_some());
        assert!(value.get("event_queue_len").is_some());
    }

    #[tokio::test]
    async fn test_debug_registers() {
        let ctx = test_ctx();

        let result = debug_registers(ctx, serde_json::Value::Null).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("unnamed").is_some());
        assert!(value.get("named").is_some());
    }

    #[tokio::test]
    async fn test_debug_marks() {
        let ctx = test_ctx();

        let result = debug_marks(ctx, serde_json::Value::Null).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("local").is_some());
        assert!(value.get("global").is_some());
        assert!(value.get("special").is_some());
    }

    #[tokio::test]
    async fn test_debug_mode_stack() {
        let ctx = test_ctx();

        let result = debug_mode_stack(ctx, serde_json::Value::Null).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("current").is_some());
        assert!(value.get("stack").is_some());
        assert!(value.get("depth").is_some());
    }
}
