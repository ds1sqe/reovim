use std::collections::HashMap;

use {
    super::*,
    reovim_kernel::api::v1::{CommandId, ModuleId},
    reovim_subsys_command_types::ArgValue,
};

fn test_command() -> CommandId {
    CommandId::new(ModuleId::new("test"), "delete")
}

#[test]
fn test_transition_context_new() {
    let ctx = TransitionContext::new();
    assert!(ctx.pending_operator.is_none());
    assert!(ctx.count.is_none());
    assert!(ctx.register.is_none());
}

#[test]
fn test_transition_context_with_operator() {
    let op = test_command();
    let ctx = TransitionContext::with_operator(op.clone());
    assert_eq!(ctx.pending_operator, Some(op));
}

#[test]
fn test_transition_context_builder() {
    let op = test_command();
    let ctx = TransitionContext::new()
        .operator(op.clone())
        .count(2)
        .register('a');

    assert_eq!(ctx.pending_operator, Some(op));
    assert_eq!(ctx.count, Some(2));
    assert_eq!(ctx.register, Some('a'));
}

#[test]
fn test_pop_result_cancelled() {
    let result = PopResult::Cancelled;
    assert!(matches!(result, PopResult::Cancelled));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_pop_result_execute_command() {
    let mut args = HashMap::new();
    args.insert("count".to_string(), ArgValue::Count(2));
    args.insert("register".to_string(), ArgValue::Register('a'));

    let result = PopResult::ExecuteCommand {
        command: test_command(),
        args,
    };

    if let PopResult::ExecuteCommand { command, args } = result {
        assert_eq!(command.name(), "delete");
        assert_eq!(args.get("count"), Some(&ArgValue::Count(2)));
        assert_eq!(args.get("register"), Some(&ArgValue::Register('a')));
    } else {
        panic!("expected ExecuteCommand");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_pop_result_data() {
    let mut values = HashMap::new();
    values.insert("pattern".to_string(), ArgValue::String("foo".to_string()));

    let result = PopResult::Data { values };

    if let PopResult::Data { values } = result {
        assert_eq!(values.get("pattern"), Some(&ArgValue::String("foo".to_string())));
    } else {
        panic!("expected Data");
    }
}

#[test]
fn test_transition_context_default() {
    let ctx = TransitionContext::default();
    assert!(ctx.pending_operator.is_none());
    assert!(ctx.count.is_none());
    assert!(ctx.register.is_none());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_transition_context_debug() {
    let ctx = TransitionContext::new();
    let debug = format!("{ctx:?}");
    assert!(debug.contains("TransitionContext"));
}

#[test]
fn test_transition_context_clone() {
    let op = test_command();
    let ctx = TransitionContext::new().operator(op).count(5).register('b');
    #[allow(clippy::redundant_clone)]
    let cloned = ctx.clone();
    assert_eq!(cloned.count, Some(5));
    assert_eq!(cloned.register, Some('b'));
    assert!(cloned.pending_operator.is_some());
}

#[test]
fn test_transition_context_operator_only() {
    let op = test_command();
    let ctx = TransitionContext::new().operator(op.clone());
    assert_eq!(ctx.pending_operator, Some(op));
    assert!(ctx.count.is_none());
    assert!(ctx.register.is_none());
}

#[test]
fn test_transition_context_count_only() {
    let ctx = TransitionContext::new().count(10);
    assert!(ctx.pending_operator.is_none());
    assert_eq!(ctx.count, Some(10));
    assert!(ctx.register.is_none());
}

#[test]
fn test_transition_context_register_only() {
    let ctx = TransitionContext::new().register('z');
    assert!(ctx.pending_operator.is_none());
    assert!(ctx.count.is_none());
    assert_eq!(ctx.register, Some('z'));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_pop_result_debug() {
    let result = PopResult::Cancelled;
    let debug = format!("{result:?}");
    assert!(debug.contains("Cancelled"));

    let result = PopResult::Data {
        values: HashMap::new(),
    };
    let debug = format!("{result:?}");
    assert!(debug.contains("Data"));
}

#[test]
fn test_pop_result_clone() {
    let result = PopResult::Cancelled;
    #[allow(clippy::redundant_clone)]
    let cloned = result.clone();
    assert!(matches!(cloned, PopResult::Cancelled));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_pop_result_execute_command_with_empty_args() {
    let result = PopResult::ExecuteCommand {
        command: test_command(),
        args: HashMap::new(),
    };

    if let PopResult::ExecuteCommand { command, args } = result {
        assert_eq!(command.name(), "delete");
        assert!(args.is_empty());
    } else {
        panic!("expected ExecuteCommand");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_pop_result_data_empty() {
    let result = PopResult::Data {
        values: HashMap::new(),
    };

    if let PopResult::Data { values } = result {
        assert!(values.is_empty());
    } else {
        panic!("expected Data");
    }
}
