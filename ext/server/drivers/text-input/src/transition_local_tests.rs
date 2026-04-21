//! Tests for `TransitionContext`, `PopResult`, and `ModeTransition`.

use {super::*, reovim_kernel::api::v1::ModuleId, reovim_subsys_command_types::ArgValue};

fn test_command() -> reovim_kernel::api::v1::CommandId {
    reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "delete")
}

fn test_mode() -> reovim_kernel::api::v1::ModeId {
    reovim_kernel::api::v1::ModeId::new(ModuleId::new("test"), "insert")
}

#[test]
fn transition_context_builders_cover_all_fields() {
    let op = test_command();
    let ctx = TransitionContext::new()
        .operator(op.clone())
        .count(2)
        .register('a');
    assert_eq!(ctx.pending_operator, Some(op));
    assert_eq!(ctx.count, Some(2));
    assert_eq!(ctx.register, Some('a'));

    let only_op = TransitionContext::with_operator(test_command());
    assert!(only_op.pending_operator.is_some());
    assert!(only_op.count.is_none());
    assert!(only_op.register.is_none());
}

#[test]
fn pop_result_variants_preserve_payloads() {
    let result = PopResult::Cancelled;
    assert!(matches!(result, PopResult::Cancelled));

    let execute = PopResult::ExecuteCommand {
        command: test_command(),
        args: std::collections::HashMap::from([("count".to_owned(), ArgValue::Count(2))]),
    };
    match execute {
        PopResult::ExecuteCommand { command, args } => {
            assert_eq!(command.name(), "delete");
            assert_eq!(args.get("count"), Some(&ArgValue::Count(2)));
        }
        PopResult::Cancelled | PopResult::Data { .. } => panic!("expected execute"),
    }

    let data = PopResult::Data {
        values: std::collections::HashMap::from([(
            "pattern".to_owned(),
            ArgValue::String("foo".to_owned()),
        )]),
    };
    match data {
        PopResult::Data { values } => {
            assert_eq!(values.get("pattern"), Some(&ArgValue::String("foo".to_owned())));
        }
        PopResult::ExecuteCommand { .. } | PopResult::Cancelled => panic!("expected data"),
    }
}

#[test]
fn mode_transition_variants_store_mode_and_context() {
    let context = TransitionContext::new().count(3);
    let mode = test_mode();

    match (ModeTransition::Push {
        mode: mode.clone(),
        context: context.clone(),
    }) {
        ModeTransition::Push {
            mode: pushed,
            context,
        } => {
            assert_eq!(pushed, mode);
            assert_eq!(context.count, Some(3));
        }
        ModeTransition::Pop { .. } | ModeTransition::Set { .. } => panic!("expected push"),
    }

    match (ModeTransition::Set {
        mode: test_mode(),
        context: TransitionContext::default(),
    }) {
        ModeTransition::Set { mode, context } => {
            assert_eq!(mode.name(), "insert");
            assert!(context.pending_operator.is_none());
        }
        ModeTransition::Push { .. } | ModeTransition::Pop { .. } => panic!("expected set"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn debug_output_mentions_types() {
    assert!(format!("{:?}", TransitionContext::default()).contains("TransitionContext"));
    assert!(format!("{:?}", PopResult::Cancelled).contains("Cancelled"));
}
