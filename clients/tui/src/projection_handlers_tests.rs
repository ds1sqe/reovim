use {
    super::*,
    reovim_protocol::v3::{DomainDatum, ProjectionUpdatedPayload},
};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn make_projection(tag: &str, bytes: Vec<u8>) -> ProjectionUpdatedPayload {
    ProjectionUpdatedPayload {
        tag: tag.to_string(),
        domain_id: 1,
        window_id: None,
        datum: Some(DomainDatum {
            content: bytes,
            display: None,
        }),
        transient: false,
        version: 1,
        client_id: 1,
    }
}

fn make_projection_no_datum(tag: &str) -> ProjectionUpdatedPayload {
    ProjectionUpdatedPayload {
        tag: tag.to_string(),
        domain_id: 1,
        window_id: None,
        datum: None,
        transient: false,
        version: 0,
        client_id: 1,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 1: text.mode live handler — decodes real UTF-8 mode name end-to-end
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_decode_mode_datum_insert() {
    let p = make_projection("text.mode", b"INSERT".to_vec());
    let decoded = decode_mode_datum(&p);
    assert_eq!(decoded, Some("INSERT".to_string()));
}

#[test]
fn test_apply_mode_to_state_normal() {
    let mut state = crate::TuiCoreState::new(1);
    apply_mode_to_state(&mut state, "NORMAL");
    assert_eq!(state.mode_name, "NORMAL");
    assert_eq!(state.mode_display, "NORMAL");
    assert!(!state.is_insert_mode());
}

#[test]
fn test_apply_mode_to_state_insert() {
    let mut state = crate::TuiCoreState::new(1);
    apply_mode_to_state(&mut state, "INSERT");
    assert_eq!(state.mode_name, "INSERT");
    assert!(state.is_insert_mode());
}

#[test]
fn test_decode_mode_datum_empty_string() {
    let p = make_projection("text.mode", b"".to_vec());
    // Empty bytes: valid UTF-8, decodes to empty string
    assert_eq!(decode_mode_datum(&p), Some(String::new()));
}

#[test]
fn test_decode_mode_datum_none() {
    let p = make_projection_no_datum("text.mode");
    assert_eq!(decode_mode_datum(&p), None);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 2-5: stub handlers return Ok(()) without panic
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_cursor_stub_no_panic() {
    let p = make_projection("text.cursor", vec![0xDE, 0xAD, 0xBE, 0xEF]);
    // Should not panic and should trace at trace level
    handle_cursor_projection::<MockProjectionCtx>(&mut MockProjectionCtx::new(), &p);
}

#[test]
fn test_selection_stub_no_panic() {
    let p = make_projection("text.selection", vec![0x01, 0x02]);
    handle_selection_projection::<MockProjectionCtx>(&mut MockProjectionCtx::new(), &p);
}

#[test]
fn test_buffer_modified_stub_no_panic() {
    let p = make_projection("text.buffer_modified", vec![0x00]);
    handle_buffer_modified_projection::<MockProjectionCtx>(&mut MockProjectionCtx::new(), &p);
}

#[test]
fn test_options_changed_stub_no_panic() {
    let p = make_projection("options.changed", vec![]);
    handle_options_changed_projection::<MockProjectionCtx>(&mut MockProjectionCtx::new(), &p);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 6: unknown tag is silently ignored without panic
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_unknown_tag_ignored() {
    let p = make_projection("mesh.camera", vec![0xFF]);
    let mut ctx = MockProjectionCtx::new();
    // Should return Ok(()) and not panic
    let result = handle_projection_updated(&mut ctx, p).await;
    assert!(result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// Minimal mock context for projection handler unit tests
// ─────────────────────────────────────────────────────────────────────────────

use reovim_client_driver::ClientModule;

struct MockProjectionCtx {
    state: crate::TuiCoreState,
    extensions: Vec<Box<dyn ClientModule>>,
}

impl MockProjectionCtx {
    fn new() -> Self {
        Self {
            state: crate::TuiCoreState::new(1),
            extensions: Vec::new(),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl crate::notification_handler::NotificationContext for MockProjectionCtx {
    fn state_mut(&mut self) -> &mut crate::TuiCoreState {
        &mut self.state
    }

    fn client_mut(&mut self) -> &mut crate::grpc_client::TuiGrpcClient {
        unimplemented!("Mock context doesn't have a real client")
    }

    fn extensions_mut(&mut self) -> &mut [Box<dyn ClientModule>] {
        &mut self.extensions
    }
}
