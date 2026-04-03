#![allow(
    clippy::significant_drop_tightening,
    clippy::uninlined_format_args,
    clippy::drop_non_drop
)]
use super::super::*;

use std::sync::Arc;

use {
    reovim_kernel::{
        api::v1::{BufferId, RwLock},
        testing::create_test_context,
    },
    reovim_provider_text::Buffer,
    reovim_types_text::{HistoryRing, Position, Register, RegisterBank},
};

// ============================================================================
// Operator trait: id, is_text_modifying
// ============================================================================

#[test]
fn lowercase_operator_id() {
    let op = LowercaseOperator;
    assert_eq!(op.id(), "lowercase");
}

#[test]
fn lowercase_operator_is_text_modifying() {
    let op = LowercaseOperator;
    assert!(op.is_text_modifying());
}

#[test]
fn uppercase_operator_id() {
    let op = UppercaseOperator;
    assert_eq!(op.id(), "uppercase");
}

#[test]
fn uppercase_operator_is_text_modifying() {
    let op = UppercaseOperator;
    assert!(op.is_text_modifying());
}

#[test]
fn toggle_case_operator_id() {
    let op = ToggleCaseOperator;
    assert_eq!(op.id(), "toggle-case");
}

#[test]
fn toggle_case_operator_is_text_modifying() {
    let op = ToggleCaseOperator;
    assert!(op.is_text_modifying());
}

// ============================================================================
// toggle_case helper function (exercised indirectly via ToggleCaseOperator)
// ============================================================================

#[test]
fn toggle_case_function_via_operator() {
    // We test the toggle_case function by executing the operator on a buffer.
    let ctx = create_test_context();
    let buffer = Buffer::from_string("Hello World");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let op = ToggleCaseOperator;
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut op_ctx = OperatorContext {
        kernel: &ctx,
        registers: &mut registers,
        clipboard_history: &mut clipboard_history,
        buffer_id,
        register: Register::Default,
        count: 1,
        cursor_position: Position::new(0, 0),
        cursor_after: None,
    };
    let range = Range::new(Position::new(0, 0), Position::new(0, 5));
    let result = op.execute(&mut op_ctx, range);
    assert!(result.is_ok());

    let buf = ctx.buffers.get(buffer_id).unwrap();
    assert_eq!(buf.read().line(0).unwrap(), "hELLO World");
}

#[test]
fn toggle_case_all_uppercase() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("ABC");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let op = ToggleCaseOperator;
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut op_ctx = OperatorContext {
        kernel: &ctx,
        registers: &mut registers,
        clipboard_history: &mut clipboard_history,
        buffer_id,
        register: Register::Default,
        count: 1,
        cursor_position: Position::new(0, 0),
        cursor_after: None,
    };
    let range = Range::new(Position::new(0, 0), Position::new(0, 3));
    op.execute(&mut op_ctx, range).unwrap();

    let buf = ctx.buffers.get(buffer_id).unwrap();
    assert_eq!(buf.read().line(0).unwrap(), "abc");
}

#[test]
fn toggle_case_non_alphabetic() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("123!@#");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let op = ToggleCaseOperator;
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut op_ctx = OperatorContext {
        kernel: &ctx,
        registers: &mut registers,
        clipboard_history: &mut clipboard_history,
        buffer_id,
        register: Register::Default,
        count: 1,
        cursor_position: Position::new(0, 0),
        cursor_after: None,
    };
    let range = Range::new(Position::new(0, 0), Position::new(0, 6));
    op.execute(&mut op_ctx, range).unwrap();

    // Non-alphabetic chars remain unchanged; cursor_after set to start
    let buf = ctx.buffers.get(buffer_id).unwrap();
    assert_eq!(buf.read().line(0).unwrap(), "123!@#");
}

// ============================================================================
// Lowercase operator
// ============================================================================

#[test]
fn lowercase_single_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("HELLO WORLD");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let op = LowercaseOperator;
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut op_ctx = OperatorContext {
        kernel: &ctx,
        registers: &mut registers,
        clipboard_history: &mut clipboard_history,
        buffer_id,
        register: Register::Default,
        count: 1,
        cursor_position: Position::new(0, 0),
        cursor_after: None,
    };
    let range = Range::new(Position::new(0, 0), Position::new(0, 5));
    op.execute(&mut op_ctx, range).unwrap();

    let buf = ctx.buffers.get(buffer_id).unwrap();
    assert_eq!(buf.read().line(0).unwrap(), "hello WORLD");
}

#[test]
fn lowercase_already_lowercase() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let op = LowercaseOperator;
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut op_ctx = OperatorContext {
        kernel: &ctx,
        registers: &mut registers,
        clipboard_history: &mut clipboard_history,
        buffer_id,
        register: Register::Default,
        count: 1,
        cursor_position: Position::new(0, 0),
        cursor_after: None,
    };
    let range = Range::new(Position::new(0, 0), Position::new(0, 5));
    op.execute(&mut op_ctx, range).unwrap();

    // No change; cursor_after should be set
    assert_eq!(op_ctx.cursor_after, Some(Position::new(0, 0)));
    let buf = ctx.buffers.get(buffer_id).unwrap();
    assert_eq!(buf.read().line(0).unwrap(), "hello");
}

// ============================================================================
// Uppercase operator
// ============================================================================

#[test]
fn uppercase_single_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let op = UppercaseOperator;
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut op_ctx = OperatorContext {
        kernel: &ctx,
        registers: &mut registers,
        clipboard_history: &mut clipboard_history,
        buffer_id,
        register: Register::Default,
        count: 1,
        cursor_position: Position::new(0, 0),
        cursor_after: None,
    };
    let range = Range::new(Position::new(0, 0), Position::new(0, 5));
    op.execute(&mut op_ctx, range).unwrap();

    let buf = ctx.buffers.get(buffer_id).unwrap();
    assert_eq!(buf.read().line(0).unwrap(), "HELLO world");
}

// ============================================================================
// Buffer not found error
// ============================================================================

#[test]
fn execute_buffer_not_found() {
    let ctx = create_test_context();
    let op = LowercaseOperator;
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut op_ctx = OperatorContext {
        kernel: &ctx,
        registers: &mut registers,
        clipboard_history: &mut clipboard_history,
        buffer_id: BufferId::from_raw(999),
        register: Register::Default,
        count: 1,
        cursor_position: Position::new(0, 0),
        cursor_after: None,
    };
    let range = Range::new(Position::new(0, 0), Position::new(0, 5));
    let result = op.execute(&mut op_ctx, range);
    assert!(result.is_err());
}

// ============================================================================
// Linewise operations
// ============================================================================

#[test]
fn lowercase_linewise() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("HELLO\nWORLD\nFOO");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let op = LowercaseOperator;
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut op_ctx = OperatorContext {
        kernel: &ctx,
        registers: &mut registers,
        clipboard_history: &mut clipboard_history,
        buffer_id,
        register: Register::Default,
        count: 1,
        cursor_position: Position::new(0, 0),
        cursor_after: None,
    };
    let range = Range::linewise(Position::new(0, 0), Position::new(1, 0));
    op.execute(&mut op_ctx, range).unwrap();

    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf_ref = buf.read();
    assert_eq!(buf_ref.line(0).unwrap(), "hello");
    assert_eq!(buf_ref.line(1).unwrap(), "world");
    // Third line should be unchanged
    assert_eq!(buf_ref.line(2).unwrap(), "FOO");
}

// ============================================================================
// Multi-line characterwise
// ============================================================================

#[test]
fn uppercase_multiline_characterwise() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld\nfoo");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let op = UppercaseOperator;
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut op_ctx = OperatorContext {
        kernel: &ctx,
        registers: &mut registers,
        clipboard_history: &mut clipboard_history,
        buffer_id,
        register: Register::Default,
        count: 1,
        cursor_position: Position::new(0, 0),
        cursor_after: None,
    };
    // Multi-line characterwise: from (0,2) to (2,2)
    let range = Range::new(Position::new(0, 2), Position::new(2, 2));
    op.execute(&mut op_ctx, range).unwrap();

    let buf = ctx.buffers.get(buffer_id).unwrap();
    let text = buf.read().content();
    // Chars from col 2 on line 0 to col 2 on line 2 should be uppercased
    assert!(text.starts_with("he"));
    assert!(text.contains("LLO"));
}
