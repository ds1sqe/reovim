use super::*;

struct TestHandler;

#[cfg_attr(coverage_nightly, coverage(off))]
impl RpcHandler for TestHandler {
    fn method(&self) -> &'static str {
        "test/echo"
    }

    fn handle(&self, params: &Value, _ctx: &RpcHandlerContext) -> RpcResult {
        RpcResult::Success(params.clone())
    }

    fn description(&self) -> &'static str {
        "Echo back the params"
    }
}

#[test]
fn test_rpc_result_variants() {
    let success = RpcResult::success(serde_json::json!({ "value": 42 }));
    assert!(matches!(success, RpcResult::Success(_)));

    let ok = RpcResult::ok();
    assert!(matches!(ok, RpcResult::Success(_)));

    let error = RpcResult::error(-1, "test error");
    assert!(matches!(error, RpcResult::Error { code: -1, .. }));

    let invalid = RpcResult::invalid_params("missing field");
    assert!(matches!(invalid, RpcResult::Error { code: -32602, .. }));

    let internal = RpcResult::internal_error("something went wrong");
    assert!(matches!(internal, RpcResult::Error { code: -32603, .. }));
}

#[test]
fn test_handler_trait() {
    let handler = TestHandler;
    assert_eq!(handler.method(), "test/echo");
    assert_eq!(handler.description(), "Echo back the params");

    let ctx = RpcHandlerContext::new(BufferId::from_raw(0));
    let result = handler.handle(&serde_json::json!({"test": 1}), &ctx);
    assert!(matches!(result, RpcResult::Success(_)));
}

#[test]
fn test_handler_debug() {
    let handler: &dyn RpcHandler = &TestHandler;
    let debug = format!("{handler:?}");
    assert!(debug.contains("test/echo"));
}

#[test]
fn test_handler_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TestHandler>();
}

#[test]
fn test_rpc_handler_context_new() {
    let ctx = RpcHandlerContext::new(BufferId::from_raw(42));
    assert_eq!(ctx.active_buffer_id(), BufferId::from_raw(42));
}

#[test]
fn test_rpc_handler_context_active_buffer_id() {
    let ctx = RpcHandlerContext::new(BufferId::from_raw(0));
    assert_eq!(ctx.active_buffer_id(), BufferId::from_raw(0));

    let ctx2 = RpcHandlerContext::new(BufferId::from_raw(99));
    assert_eq!(ctx2.active_buffer_id(), BufferId::from_raw(99));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_handler_default_description() {
    struct NoDescHandler;
    impl RpcHandler for NoDescHandler {
        fn method(&self) -> &'static str {
            "test/no_desc"
        }
        fn handle(&self, _params: &Value, _ctx: &RpcHandlerContext) -> RpcResult {
            RpcResult::ok()
        }
        // description() not overridden - uses default
    }

    let handler = NoDescHandler;
    assert_eq!(handler.description(), "No description available");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_rpc_result_success_value() {
    let result = RpcResult::success(serde_json::json!({"key": "val"}));
    match result {
        RpcResult::Success(v) => assert_eq!(v["key"], "val"),
        RpcResult::Error { .. } => panic!("Expected Success"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_rpc_result_ok_content() {
    let result = RpcResult::ok();
    match result {
        RpcResult::Success(v) => {
            assert_eq!(v["ok"], true);
        }
        RpcResult::Error { .. } => panic!("Expected Success"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_rpc_result_error_content() {
    let result = RpcResult::error(-32000, "custom");
    match result {
        RpcResult::Error { code, message } => {
            assert_eq!(code, -32000);
            assert_eq!(message, "custom");
        }
        RpcResult::Success(_) => panic!("Expected Error"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_rpc_result_invalid_params_content() {
    let result = RpcResult::invalid_params("field 'x' missing");
    match result {
        RpcResult::Error { code, message } => {
            assert_eq!(code, crate::INVALID_PARAMS);
            assert_eq!(message, "field 'x' missing");
        }
        RpcResult::Success(_) => panic!("Expected Error"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_rpc_result_internal_error_content() {
    let result = RpcResult::internal_error("panic in handler");
    match result {
        RpcResult::Error { code, message } => {
            assert_eq!(code, crate::INTERNAL_ERROR);
            assert_eq!(message, "panic in handler");
        }
        RpcResult::Success(_) => panic!("Expected Error"),
    }
}

#[test]
fn test_rpc_result_debug() {
    let success = RpcResult::success(serde_json::json!(1));
    let debug_str = format!("{success:?}");
    assert!(debug_str.contains("Success"));

    let error = RpcResult::error(-1, "err");
    let debug_str = format!("{error:?}");
    assert!(debug_str.contains("Error"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_handler_handle_returns_echo() {
    let handler = TestHandler;
    let ctx = RpcHandlerContext::new(BufferId::from_raw(0));
    let params = serde_json::json!({"echo": "test"});
    let result = handler.handle(&params, &ctx);
    match result {
        RpcResult::Success(v) => assert_eq!(v["echo"], "test"),
        RpcResult::Error { .. } => panic!("Expected Success"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_rpc_result_error_string_conversion() {
    // Test Into<String> conversion for error message
    let result = RpcResult::error(-100, String::from("owned string"));
    match result {
        RpcResult::Error { code, message } => {
            assert_eq!(code, -100);
            assert_eq!(message, "owned string");
        }
        RpcResult::Success(_) => panic!("Expected Error"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_rpc_result_invalid_params_string_conversion() {
    let result = RpcResult::invalid_params(String::from("owned message"));
    match result {
        RpcResult::Error { code, message } => {
            assert_eq!(code, crate::INVALID_PARAMS);
            assert_eq!(message, "owned message");
        }
        RpcResult::Success(_) => panic!("Expected Error"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_rpc_result_internal_error_string_conversion() {
    let result = RpcResult::internal_error(String::from("owned internal"));
    match result {
        RpcResult::Error { code, message } => {
            assert_eq!(code, crate::INTERNAL_ERROR);
            assert_eq!(message, "owned internal");
        }
        RpcResult::Success(_) => panic!("Expected Error"),
    }
}

#[test]
fn test_handler_debug_format() {
    let handler: &dyn RpcHandler = &TestHandler;
    let debug = format!("{handler:?}");
    assert_eq!(debug, "RpcHandler(test/echo)");
}

#[test]
fn test_rpc_handler_context_field_access() {
    let buf_id = BufferId::from_raw(12345);
    let ctx = RpcHandlerContext::new(buf_id);
    assert_eq!(ctx.active_buffer_id, buf_id);
    assert_eq!(ctx.active_buffer_id(), buf_id);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_rpc_result_success_with_null() {
    let result = RpcResult::success(serde_json::Value::Null);
    match result {
        RpcResult::Success(v) => assert!(v.is_null()),
        RpcResult::Error { .. } => panic!("Expected Success"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_rpc_result_success_with_array() {
    let result = RpcResult::success(serde_json::json!([1, 2, 3]));
    match result {
        RpcResult::Success(v) => {
            assert!(v.is_array());
            assert_eq!(v.as_array().unwrap().len(), 3);
        }
        RpcResult::Error { .. } => panic!("Expected Success"),
    }
}
