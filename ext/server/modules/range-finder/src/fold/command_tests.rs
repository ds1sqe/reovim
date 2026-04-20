use std::ops::Range;

use {
    reovim_driver_text_session::testing::TestSessionRuntime,
    reovim_driver_text_syntax::{Annotation, FoldKind, FoldRange, SyntaxDriver, SyntaxEdit},
    reovim_kernel::api::v1::BufferId,
};

use super::*;

// =========================================================================
// Metadata
// =========================================================================

#[test]
fn test_fold_toggle_command_metadata() {
    let cmd = FoldToggleCommand;
    assert_eq!(cmd.id(), ids::FOLD_TOGGLE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_fold_open_command_metadata() {
    let cmd = FoldOpenCommand;
    assert_eq!(cmd.id(), ids::FOLD_OPEN);
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_fold_close_command_metadata() {
    let cmd = FoldCloseCommand;
    assert_eq!(cmd.id(), ids::FOLD_CLOSE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_fold_open_all_command_metadata() {
    let cmd = FoldOpenAllCommand;
    assert_eq!(cmd.id(), ids::FOLD_OPEN_ALL);
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_fold_close_all_command_metadata() {
    let cmd = FoldCloseAllCommand;
    assert_eq!(cmd.id(), ids::FOLD_CLOSE_ALL);
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_all_commands_count() {
    let commands = all_commands();
    assert_eq!(commands.len(), 5);
}

#[test]
fn test_all_commands_unique_ids() {
    let commands = all_commands();
    let ids: Vec<CommandId> = commands.iter().map(|c| c.id()).collect();
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            assert_ne!(ids[i], ids[j]);
        }
    }
}

// =========================================================================
// Execute tests — early returns
// =========================================================================

#[test]
fn test_fold_toggle_no_buffer() {
    let cmd = FoldToggleCommand;
    let mut harness = TestSessionRuntime::new();
    let args = CommandContext::new();
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_fold_toggle_no_window() {
    let cmd = FoldToggleCommand;
    let mut harness = TestSessionRuntime::new();
    let mut args = CommandContext::new();
    args.set_buffer_id(BufferId::from_raw(0));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_fold_open_no_buffer() {
    let cmd = FoldOpenCommand;
    let mut harness = TestSessionRuntime::new();
    let args = CommandContext::new();
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_fold_open_no_window() {
    let cmd = FoldOpenCommand;
    let mut harness = TestSessionRuntime::new();
    let mut args = CommandContext::new();
    args.set_buffer_id(BufferId::from_raw(0));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_fold_close_no_buffer() {
    let cmd = FoldCloseCommand;
    let mut harness = TestSessionRuntime::new();
    let args = CommandContext::new();
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_fold_close_no_window() {
    let cmd = FoldCloseCommand;
    let mut harness = TestSessionRuntime::new();
    let mut args = CommandContext::new();
    args.set_buffer_id(BufferId::from_raw(0));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_fold_open_all_no_buffer() {
    let cmd = FoldOpenAllCommand;
    let mut harness = TestSessionRuntime::new();
    let args = CommandContext::new();
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_fold_close_all_no_buffer() {
    let cmd = FoldCloseAllCommand;
    let mut harness = TestSessionRuntime::new();
    let args = CommandContext::new();
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

// =========================================================================
// Execute tests — happy path (no syntax driver → empty folds)
// =========================================================================

#[test]
fn test_fold_toggle_empty_folds() {
    let cmd = FoldToggleCommand;
    let mut harness = TestSessionRuntime::with_buffer("line0\nline1\nline2");
    let mut args = CommandContext::new();
    args.set_buffer_id(BufferId::from_raw(0));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_fold_open_empty_folds() {
    let cmd = FoldOpenCommand;
    let mut harness = TestSessionRuntime::with_buffer("line0\nline1\nline2");
    let mut args = CommandContext::new();
    args.set_buffer_id(BufferId::from_raw(0));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_fold_close_empty_folds() {
    let cmd = FoldCloseCommand;
    let mut harness = TestSessionRuntime::with_buffer("line0\nline1\nline2");
    let mut args = CommandContext::new();
    args.set_buffer_id(BufferId::from_raw(0));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_fold_open_all_empty_folds() {
    let cmd = FoldOpenAllCommand;
    let mut harness = TestSessionRuntime::with_buffer("line0\nline1\nline2");
    let mut args = CommandContext::new();
    args.set_buffer_id(BufferId::from_raw(0));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_fold_close_all_empty_folds() {
    let cmd = FoldCloseAllCommand;
    let mut harness = TestSessionRuntime::with_buffer("line0\nline1\nline2");
    let mut args = CommandContext::new();
    args.set_buffer_id(BufferId::from_raw(0));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

// =========================================================================
// Execute tests — with fold ranges (via mock SyntaxDriver)
// =========================================================================

/// Mock syntax driver that returns configurable fold ranges.
struct MockFoldDriver {
    folds: Vec<FoldRange>,
}

impl MockFoldDriver {
    fn new(folds: Vec<FoldRange>) -> Self {
        Self { folds }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl SyntaxDriver for MockFoldDriver {
    fn language(&self) -> &'static str {
        "mock"
    }

    fn parse(&mut self, _content: &str) {}

    fn update(&mut self, _content: &str, _edit: &SyntaxEdit) {}

    fn highlights(&self, _byte_range: Range<usize>) -> Vec<Annotation> {
        Vec::new()
    }

    fn folds(&self) -> Vec<FoldRange> {
        self.folds.clone()
    }

    fn is_parsed(&self) -> bool {
        true
    }
}

/// Register a mock syntax driver with fold ranges for buffer 0.
fn register_mock_folds(harness: &mut TestSessionRuntime, folds: Vec<FoldRange>) {
    harness.with_runtime(|rt| {
        let syntax = rt.ext_mut::<SyntaxSessionState>();
        let driver = MockFoldDriver::new(folds);
        syntax.set(BufferId::from_raw(0), Box::new(driver));
    });
}

#[test]
fn test_fold_toggle_with_fold_at_cursor() {
    let cmd = FoldToggleCommand;
    let mut harness = TestSessionRuntime::with_buffer("fn main() {\n  body\n}");
    let folds = vec![FoldRange::new(0, 2, FoldKind::Function, "fn main() {")];
    register_mock_folds(&mut harness, folds);

    let mut args = CommandContext::new();
    args.set_buffer_id(BufferId::from_raw(0));
    // Cursor at line 0 is inside the fold range (0..2).
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));

    // Verify fold was toggled (collapsed after toggle).
    harness.with_runtime(|rt| {
        let state = rt.ext_mut::<FoldSessionState>();
        let fold = state.get_or_insert(BufferId::from_raw(0));
        assert!(fold.has_collapsed());
    });
}

#[test]
fn test_fold_open_with_fold_at_cursor() {
    let cmd = FoldOpenCommand;
    let mut harness = TestSessionRuntime::with_buffer("fn main() {\n  body\n}");
    let folds = vec![FoldRange::new(0, 2, FoldKind::Function, "fn main() {")];
    register_mock_folds(&mut harness, folds);

    let mut args = CommandContext::new();
    args.set_buffer_id(BufferId::from_raw(0));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_fold_close_with_fold_at_cursor() {
    let cmd = FoldCloseCommand;
    let mut harness = TestSessionRuntime::with_buffer("fn main() {\n  body\n}");
    let folds = vec![FoldRange::new(0, 2, FoldKind::Function, "fn main() {")];
    register_mock_folds(&mut harness, folds);

    let mut args = CommandContext::new();
    args.set_buffer_id(BufferId::from_raw(0));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));

    // Verify fold was closed.
    harness.with_runtime(|rt| {
        let state = rt.ext_mut::<FoldSessionState>();
        let fold = state.get_or_insert(BufferId::from_raw(0));
        assert!(fold.has_collapsed());
    });
}

#[test]
fn test_fold_open_all_with_folds() {
    let cmd = FoldOpenAllCommand;
    let mut harness = TestSessionRuntime::with_buffer("fn a() {\n}\nfn b() {\n}");
    let folds = vec![
        FoldRange::new(0, 1, FoldKind::Function, "fn a() {"),
        FoldRange::new(2, 3, FoldKind::Function, "fn b() {"),
    ];
    register_mock_folds(&mut harness, folds);

    let mut args = CommandContext::new();
    args.set_buffer_id(BufferId::from_raw(0));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_fold_close_all_with_folds() {
    let cmd = FoldCloseAllCommand;
    let mut harness = TestSessionRuntime::with_buffer("fn a() {\n}\nfn b() {\n}");
    let folds = vec![
        FoldRange::new(0, 1, FoldKind::Function, "fn a() {"),
        FoldRange::new(2, 3, FoldKind::Function, "fn b() {"),
    ];
    register_mock_folds(&mut harness, folds);

    let mut args = CommandContext::new();
    args.set_buffer_id(BufferId::from_raw(0));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));

    // Verify all folds were closed.
    harness.with_runtime(|rt| {
        let state = rt.ext_mut::<FoldSessionState>();
        let fold = state.get_or_insert(BufferId::from_raw(0));
        assert!(fold.has_collapsed());
    });
}
