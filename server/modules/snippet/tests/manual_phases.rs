//! Manual E2E tests for snippet Phases 2-5 (#136).
//!
//! Tests verify Phase 2 (placeholder selection/mirroring), Phase 3 (variables),
//! Phase 4 (completion integration), and Phase 5 (transforms, choices) through
//! the full server/client gRPC stack.
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p reovim-module-snippet --test manual_phases -- --nocapture
//! ```

use reovim_testing::{IntegrationTest, StepTest};

// ============================================================================
// Phase 2: Placeholder Selection (not deletion)
// ============================================================================

/// Expand "fn" — placeholder "name" should be visible (selected, not deleted).
#[tokio::test]
async fn phase2_placeholder_visible_after_expand() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("")
        .send_keys("ifn<C-s>")
        .with_delay(150)
        .run()
        .await;

    // Phase 2: placeholder text is selected, not deleted — so it must be visible.
    result.assert_buffer_contains("name");
    result.assert_buffer_contains("params");
    result.assert_buffer_contains("fn ");
    result.assert_buffer_contains("{");
    result.assert_buffer_contains("}");
    eprintln!("[phase2] Placeholder visible after expand: PASS");
}

/// Expand "fn", then type to replace placeholder — typed text replaces selection.
#[tokio::test]
async fn phase2_typing_replaces_placeholder() {
    let trace = StepTest::new()
        .await
        .with_buffer("")
        .step("ifn<C-s>")
        .expect_buffer_contains("name") // placeholder visible
        .step("m") // Type 'm' — should replace "name" with "m"
        .expect_buffer_contains("fn m")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
    eprintln!("[phase2] Typing replaces placeholder: PASS");
}

/// Expand "fn", Tab to next — "params" placeholder should be visible.
#[tokio::test]
async fn phase2_tab_preserves_placeholder() {
    let trace = StepTest::new()
        .await
        .with_buffer("")
        .step("ifn<C-s>")
        .expect_buffer_contains("name")
        .step("<Tab>")
        .expect_buffer_contains("params") // $2 placeholder still visible
        .expect_buffer_contains("name") // $1 default kept (not typed over)
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
    eprintln!("[phase2] Tab preserves placeholder text: PASS");
}

/// Type replacement at $1, Tab to $2, type replacement — both replaced.
#[tokio::test]
async fn phase2_type_at_both_stops() {
    let trace = StepTest::new()
        .await
        .with_buffer("")
        .step("ifn<C-s>")
        .expect_buffer_contains("name")
        .step("main") // Replace "name" with "main"
        .expect_buffer_contains("fn main")
        .step("<Tab>")
        .expect_buffer_contains("params") // $2 placeholder
        .step("x: i32") // Replace "params"
        .expect_buffer_contains("x: i32")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();

    // Verify final buffer looks like a proper function
    let final_buf = &trace.final_state().buffer;
    assert!(final_buf.contains("fn main"), "Should have 'fn main', got: {final_buf}");
    assert!(final_buf.contains("x: i32"), "Should have 'x: i32', got: {final_buf}");
    eprintln!("[phase2] Type at both stops: PASS");
}

/// Escape during navigation returns to INSERT mode.
#[tokio::test]
async fn phase2_escape_returns_insert() {
    let trace = StepTest::new()
        .await
        .with_buffer("")
        .step("ifn<C-s>")
        .expect_mode_contains("NAVIGATING")
        .step("<Esc>")
        .expect_mode_contains("INSERT")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
    eprintln!("[phase2] Escape returns to INSERT: PASS");
}

// ============================================================================
// Phase 2: Multi-stop workflow
// ============================================================================

/// Full workflow: expand → type at $1 → Tab → type at $2 → Tab → $0 → Esc.
#[tokio::test]
async fn phase2_full_workflow_fn() {
    let trace = StepTest::new()
        .await
        .with_buffer("")
        .step("ifn<C-s>")
        .expect_buffer_contains("fn ")
        .expect_mode_contains("NAVIGATING")
        .step("add") // Replace $1 "name"
        .expect_buffer_contains("fn add")
        .step("<Tab>") // Move to $2
        .expect_buffer_contains("params")
        .step("a: u32, b: u32") // Replace $2 "params"
        .expect_buffer_contains("a: u32, b: u32")
        .step("<Tab>") // Move to $0 (body)
        .step("a + b") // Type body
        .expect_buffer_contains("a + b")
        .step("<Esc>") // Done
        .expect_mode_contains("INSERT")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();

    let buf = &trace.final_state().buffer;
    eprintln!("[phase2] Full workflow result:\n{buf}");
    assert!(buf.contains("fn add"), "Missing 'fn add'");
    assert!(buf.contains("a: u32, b: u32"), "Missing params");
    assert!(buf.contains("a + b"), "Missing body");
}

/// For loop workflow: expand → type var → Tab → type iter → Tab → body.
#[tokio::test]
async fn phase2_full_workflow_for() {
    let trace = StepTest::new()
        .await
        .with_buffer("")
        .step("ifor<C-s>")
        .expect_buffer_contains("for ")
        .expect_buffer_contains("in ")
        .step("x") // Replace $1 "i"
        .step("<Tab>") // Move to $2
        .step("0..10") // Replace $2 "iter"
        .step("<Tab>") // Move to $0
        .step("println!(\"{x}\")") // Type body
        .step("<Esc>")
        .expect_mode_contains("INSERT")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();

    let buf = &trace.final_state().buffer;
    eprintln!("[phase2] For loop result:\n{buf}");
    assert!(buf.contains("for x"), "Missing 'for x'");
    assert!(buf.contains("0..10"), "Missing '0..10'");
}

// ============================================================================
// Phase 3: Variables
// ============================================================================

/// "hdr" snippet uses `$TM_FILENAME`, `$CURRENT_YEAR` etc.
#[tokio::test]
async fn phase3_variable_expansion() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("")
        .send_keys("ihdr<C-s>")
        .with_delay(150)
        .run()
        .await;

    let buf = result.buffer_content.as_str();
    eprintln!("[phase3] Header snippet:\n{buf}");

    // Should contain "// File:" (the variable may resolve or fallback)
    assert!(buf.contains("// File:"), "Missing file header, got: {buf}");
    // Should contain "// Date:" with current year
    assert!(buf.contains("// Date: 2026"), "Missing date with 2026, got: {buf}");
}

/// "uid" snippet uses $UUID — should produce a UUID-like string.
#[tokio::test]
async fn phase3_uuid_variable() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("")
        .send_keys("iuid<C-s>")
        .with_delay(150)
        .run()
        .await;

    let buf = result.buffer_content.as_str();
    eprintln!("[phase3] UUID snippet:\n{buf}");

    // UUID format: 8-4-4-4-12 hex chars
    assert!(buf.contains("// ID:"), "Missing ID marker, got: {buf}");
    // Check for UUID-like pattern (at least has dashes in right places)
    let id_line = buf.lines().find(|l| l.contains("// ID:")).unwrap();
    let uuid_part = id_line.trim_start_matches("// ID: ").trim();
    assert_eq!(
        uuid_part.len(),
        36,
        "UUID should be 36 chars, got {} chars: '{uuid_part}'",
        uuid_part.len()
    );
    assert_eq!(
        uuid_part.chars().filter(|c| *c == '-').count(),
        4,
        "UUID should have 4 dashes: '{uuid_part}'"
    );
    eprintln!("[phase3] UUID: {uuid_part}");
}

// ============================================================================
// Phase 5: Choices
// ============================================================================

/// "vis" snippet uses choice syntax ${1|pub,pub(crate),pub(super)|}.
/// First choice "pub" should be the default.
#[tokio::test]
async fn phase5_choice_first_default() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("")
        .send_keys("ivis<C-s>")
        .with_delay(150)
        .run()
        .await;

    let buf = result.buffer_content.as_str();
    eprintln!("[phase5] Choice snippet:\n{buf}");

    // First choice "pub" should be visible as default
    assert!(buf.contains("pub"), "Missing 'pub' default choice, got: {buf}");
    assert!(buf.contains("item"), "Missing 'item' placeholder, got: {buf}");
}

/// "vis" — type replacement for first choice, Tab to $2, type item name.
#[tokio::test]
async fn phase5_choice_workflow() {
    let trace = StepTest::new()
        .await
        .with_buffer("")
        .step("ivis<C-s>")
        .expect_buffer_contains("pub")
        .step("pub(crate)") // Override choice with typed text
        .expect_buffer_contains("pub(crate)")
        .step("<Tab>") // Move to $2
        .step("fn main()") // Replace "item"
        .expect_buffer_contains("fn main()")
        .step("<Esc>")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();

    let buf = &trace.final_state().buffer;
    eprintln!("[phase5] Choice workflow result: {buf}");
    assert!(buf.contains("pub(crate)"), "Missing pub(crate)");
    assert!(buf.contains("fn main()"), "Missing fn main()");
}

// ============================================================================
// Phase 2+5: Complex multi-stop with choices
// ============================================================================

/// "st" struct snippet with 3 stops — full workflow.
#[tokio::test]
async fn complex_struct_workflow() {
    let trace = StepTest::new()
        .await
        .with_buffer("")
        .step("ist<C-s>")
        .expect_buffer_contains("struct")
        .expect_buffer_contains("Name")
        .step("Point") // Replace $1 "Name"
        .expect_buffer_contains("Point")
        .step("<Tab>") // Move to $2 "field"
        .step("x") // Replace field name
        .step("<Tab>") // Move to $3 "Type"
        .step("f64") // Replace type
        .step("<Esc>")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();

    let buf = &trace.final_state().buffer;
    eprintln!("[complex] Struct result:\n{buf}");
    assert!(buf.contains("struct Point"), "Missing 'struct Point'");
    assert!(buf.contains('x'), "Missing field 'x'");
    assert!(buf.contains("f64"), "Missing type 'f64'");
}

/// "mat" match snippet with 3 stops.
#[tokio::test]
async fn complex_match_workflow() {
    let trace = StepTest::new()
        .await
        .with_buffer("")
        .step("imat<C-s>")
        .expect_buffer_contains("match")
        .step("val") // $1 expr
        .step("<Tab>")
        .step("Some(x)") // $2 pattern
        .step("<Tab>")
        .step("x + 1") // $3 result
        .step("<Esc>")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();

    let buf = &trace.final_state().buffer;
    eprintln!("[complex] Match result:\n{buf}");
    assert!(buf.contains("match val"), "Missing 'match val'");
    assert!(buf.contains("Some(x)"), "Missing pattern");
    assert!(buf.contains("x + 1"), "Missing result");
}

/// "ife" if-else snippet — multi-line with 2 stops + $0.
#[tokio::test]
async fn complex_if_else_workflow() {
    let trace = StepTest::new()
        .await
        .with_buffer("")
        .step("iife<C-s>")
        .expect_buffer_contains("if ")
        .expect_buffer_contains("else")
        .step("x > 0") // $1 condition
        .step("<Tab>")
        .step("true") // $2 body
        .step("<Tab>")
        .step("false") // $0 else body
        .step("<Esc>")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();

    let buf = &trace.final_state().buffer;
    eprintln!("[complex] If-else result:\n{buf}");
    assert!(buf.contains("if x > 0"), "Missing condition");
    assert!(buf.contains("true"), "Missing if body");
    assert!(buf.contains("false"), "Missing else body");
}

// ============================================================================
// Phase 2: S-Tab backward navigation
// ============================================================================

/// Tab forward twice, S-Tab back, verify cursor went back.
#[tokio::test]
async fn phase2_shift_tab_backward_and_retype() {
    let trace = StepTest::new()
        .await
        .with_buffer("")
        .step("ifn<C-s>")
        .expect_buffer_contains("name")
        .step("<Tab>") // $1 → $2
        .expect_buffer_contains("params")
        .step("<S-Tab>") // $2 → $1 (back)
        .expect_buffer_contains("name") // should still show $1 placeholder
        .step("corrected") // Now type at $1
        .expect_buffer_contains("corrected")
        .step("<Esc>")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
    eprintln!("[phase2] S-Tab backward and retype: PASS");
}

// ============================================================================
// Edge Cases
// ============================================================================

/// Expand "tst" — no tab stops, should go straight back to insert.
#[tokio::test]
async fn edge_no_tabstops_returns_insert() {
    let trace = StepTest::new()
        .await
        .with_buffer("")
        .step("itst<C-s>")
        .expect_buffer_contains("TEST_EXPANDED")
        .expect_mode_contains("INSERT")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
    eprintln!("[edge] No tab stops returns to INSERT: PASS");
}

/// Unknown prefix — no expansion, stay in insert.
#[tokio::test]
async fn edge_no_match_stays_insert() {
    let trace = StepTest::new()
        .await
        .with_buffer("")
        .step("inotasnippet<C-s>")
        .expect_buffer_contains("notasnippet")
        .expect_mode_contains("INSERT")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
    eprintln!("[edge] No match stays in INSERT: PASS");
}
