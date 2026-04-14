use super::*;

// ========================================================================
// parse_set_expr tests
// ========================================================================

#[test]
fn test_parse_empty_is_list_changed() {
    assert_eq!(parse_set_expr(""), SetAction::ListChanged);
}

#[test]
fn test_parse_whitespace_only_is_list_changed() {
    assert_eq!(parse_set_expr("   "), SetAction::ListChanged);
}

#[test]
fn test_parse_all_is_list_all() {
    assert_eq!(parse_set_expr("all"), SetAction::ListAll);
}

#[test]
fn test_parse_query() {
    assert_eq!(
        parse_set_expr("number?"),
        SetAction::Show {
            name: "number".to_string()
        }
    );
}

#[test]
fn test_parse_toggle() {
    assert_eq!(
        parse_set_expr("number!"),
        SetAction::Toggle {
            name: "number".to_string()
        }
    );
}

#[test]
fn test_parse_reset() {
    assert_eq!(
        parse_set_expr("number&"),
        SetAction::Reset {
            name: "number".to_string()
        }
    );
}

#[test]
fn test_parse_assign() {
    assert_eq!(
        parse_set_expr("tabstop=8"),
        SetAction::Assign {
            name: "tabstop".to_string(),
            raw_value: "8".to_string()
        }
    );
}

#[test]
fn test_parse_assign_string_value() {
    assert_eq!(
        parse_set_expr("theme=dark"),
        SetAction::Assign {
            name: "theme".to_string(),
            raw_value: "dark".to_string()
        }
    );
}

#[test]
fn test_parse_assign_empty_value() {
    assert_eq!(
        parse_set_expr("opt="),
        SetAction::Assign {
            name: "opt".to_string(),
            raw_value: String::new()
        }
    );
}

#[test]
fn test_parse_no_prefix() {
    assert_eq!(
        parse_set_expr("nonumber"),
        SetAction::SetBool {
            name: "number".to_string(),
            value: false,
        }
    );
}

#[test]
fn test_parse_bare_name_bool_true() {
    assert_eq!(
        parse_set_expr("number"),
        SetAction::SetBool {
            name: "number".to_string(),
            value: true,
        }
    );
}

#[test]
fn test_parse_no_alone_is_set_bool_false() {
    // "no" has strip_prefix("no") -> "" which is empty, so falls through
    // to bare name: SetBool { name: "no", value: true }
    assert_eq!(
        parse_set_expr("no"),
        SetAction::SetBool {
            name: "no".to_string(),
            value: true,
        }
    );
}

#[test]
fn test_parse_with_leading_whitespace() {
    assert_eq!(
        parse_set_expr("  number  "),
        SetAction::SetBool {
            name: "number".to_string(),
            value: true,
        }
    );
}

// ========================================================================
// parse_value_for_type tests
// ========================================================================

#[test]
fn test_parse_value_bool_true_variants() {
    let expected = OptionValue::bool(false);
    assert_eq!(parse_value_for_type("true", &expected, "opt").unwrap(), OptionValue::bool(true));
    assert_eq!(parse_value_for_type("1", &expected, "opt").unwrap(), OptionValue::bool(true));
    assert_eq!(parse_value_for_type("on", &expected, "opt").unwrap(), OptionValue::bool(true));
}

#[test]
fn test_parse_value_bool_false_variants() {
    let expected = OptionValue::bool(true);
    assert_eq!(
        parse_value_for_type("false", &expected, "opt").unwrap(),
        OptionValue::bool(false)
    );
    assert_eq!(parse_value_for_type("0", &expected, "opt").unwrap(), OptionValue::bool(false));
    assert_eq!(parse_value_for_type("off", &expected, "opt").unwrap(), OptionValue::bool(false));
}

#[test]
fn test_parse_value_bool_invalid() {
    let expected = OptionValue::bool(false);
    let result = parse_value_for_type("maybe", &expected, "opt");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("expected bool"));
}

#[test]
fn test_parse_value_integer() {
    let expected = OptionValue::int(0);
    assert_eq!(parse_value_for_type("42", &expected, "opt").unwrap(), OptionValue::int(42));
}

#[test]
fn test_parse_value_negative_integer() {
    let expected = OptionValue::int(0);
    assert_eq!(parse_value_for_type("-5", &expected, "opt").unwrap(), OptionValue::int(-5));
}

#[test]
fn test_parse_value_integer_invalid() {
    let expected = OptionValue::int(0);
    let result = parse_value_for_type("abc", &expected, "opt");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("expected integer"));
}

#[test]
fn test_parse_value_string() {
    let expected = OptionValue::string("default");
    assert_eq!(
        parse_value_for_type("hello", &expected, "opt").unwrap(),
        OptionValue::string("hello")
    );
}

#[test]
fn test_parse_value_choice() {
    let choices = vec!["light".into(), "dark".into()];
    let expected = OptionValue::choice("light", choices.clone());
    assert_eq!(
        parse_value_for_type("dark", &expected, "opt").unwrap(),
        OptionValue::choice("dark", choices)
    );
}

// ========================================================================
// Command trait tests
// ========================================================================

#[test]
fn test_set_command_id() {
    let cmd = SetCommand;
    assert_eq!(cmd.id().name(), "set");
    assert_eq!(cmd.id().module().as_str(), "commands");
}

#[test]
fn test_set_command_names() {
    let cmd = SetCommand;
    let names = cmd.names();
    assert_eq!(names.len(), 1);
    assert!(names.contains(&"set"));
}

#[test]
fn test_set_command_description() {
    let cmd = SetCommand;
    let desc = cmd.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("Set"));
}

#[test]
fn test_set_command_args() {
    let cmd = SetCommand;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "expr");
    assert_eq!(args[0].kind, ArgKind::Rest);
    assert!(!args[0].required);
}

#[test]
fn test_set_command_complete_returns_empty() {
    let cmd = SetCommand;
    let completions = cmd.complete("num");
    assert!(completions.is_empty());
}

#[test]
fn test_set_command_debug() {
    let cmd = SetCommand;
    let debug = format!("{cmd:?}");
    assert!(debug.contains("SetCommand"));
}

#[test]
fn test_set_command_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SetCommand>();
}

#[test]
fn test_set_command_clone_copy() {
    let cmd = SetCommand;
    let copy = cmd;
    assert_eq!(cmd.id(), copy.id());
}

// ========================================================================
// Execute tests (using TestSessionRuntime)
// ========================================================================

/// Helper: register a boolean option for testing.
fn register_bool_option(harness: &reovim_driver_text_session::testing::TestSessionRuntime) {
    harness
        .kernel()
        .options
        .register(
            reovim_kernel::api::v1::OptionSpec::new(
                "number",
                "Show line numbers",
                OptionValue::bool(false),
            )
            .with_short("nu")
            .with_scope(reovim_kernel::api::v1::OptionScope::Window),
        )
        .unwrap();
}

/// Helper: register an integer option for testing.
fn register_int_option(harness: &reovim_driver_text_session::testing::TestSessionRuntime) {
    harness
        .kernel()
        .options
        .register(reovim_kernel::api::v1::OptionSpec::new(
            "tabstop",
            "Number of spaces per tab",
            OptionValue::int(4),
        ))
        .unwrap();
}

/// Helper: register a string option for testing.
fn register_string_option(harness: &reovim_driver_text_session::testing::TestSessionRuntime) {
    harness
        .kernel()
        .options
        .register(reovim_kernel::api::v1::OptionSpec::new(
            "theme",
            "Color theme",
            OptionValue::string("default"),
        ))
        .unwrap();
}

/// Helper: register a choice option for testing.
fn register_choice_option(harness: &reovim_driver_text_session::testing::TestSessionRuntime) {
    harness
        .kernel()
        .options
        .register(reovim_kernel::api::v1::OptionSpec::new(
            "background",
            "Background style",
            OptionValue::choice("light", vec!["light".into(), "dark".into()]),
        ))
        .unwrap();
}

/// Helper: register an integer option with constraints for testing.
fn register_constrained_int_option(
    harness: &reovim_driver_text_session::testing::TestSessionRuntime,
) {
    harness
        .kernel()
        .options
        .register(
            reovim_kernel::api::v1::OptionSpec::new(
                "scrolloff",
                "Scroll offset",
                OptionValue::int(0),
            )
            .with_constraint(reovim_kernel::api::v1::OptionConstraint::min(0)),
        )
        .unwrap();
}

fn make_ctx(expr: &str) -> CommandContext {
    let mut ctx = CommandContext::new();
    ctx.set("expr", reovim_subsys_command_types::ArgValue::String(expr.to_string()));
    ctx
}

// --- Set bool true ---

#[test]
fn test_execute_set_bool_true() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("number");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        // Verify option was set
        let val = runtime
            .kernel()
            .options
            .get("number", OptionScopeId::Global);
        assert_eq!(val, Some(OptionValue::bool(true)));
    });
}

// --- Set bool false ---

#[test]
fn test_execute_set_bool_false() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);
    // Set to true first
    harness
        .kernel()
        .options
        .set("number", OptionValue::bool(true), OptionScopeId::Global)
        .unwrap();

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("nonumber");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        let val = runtime
            .kernel()
            .options
            .get("number", OptionScopeId::Global);
        assert_eq!(val, Some(OptionValue::bool(false)));
    });
}

// --- Toggle ---

#[test]
fn test_execute_toggle() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("number!");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        let val = runtime
            .kernel()
            .options
            .get("number", OptionScopeId::Global);
        assert_eq!(val, Some(OptionValue::bool(true)));
    });
}

// --- Assign integer ---

#[test]
fn test_execute_assign_integer() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_int_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("tabstop=8");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        let val = runtime
            .kernel()
            .options
            .get("tabstop", OptionScopeId::Global);
        assert_eq!(val, Some(OptionValue::int(8)));
    });
}

// --- Assign string ---

#[test]
fn test_execute_assign_string() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_string_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("theme=monokai");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        let val = runtime.kernel().options.get("theme", OptionScopeId::Global);
        assert_eq!(val, Some(OptionValue::string("monokai")));
    });
}

// --- Assign choice ---

#[test]
fn test_execute_assign_choice() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_choice_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("background=dark");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        let val = runtime
            .kernel()
            .options
            .get("background", OptionScopeId::Global);
        assert_eq!(val, Some(OptionValue::choice("dark", vec!["light".into(), "dark".into()])));
    });
}

// --- Assign bool via = ---

#[test]
fn test_execute_assign_bool_via_equals() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("number=true");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        let val = runtime
            .kernel()
            .options
            .get("number", OptionScopeId::Global);
        assert_eq!(val, Some(OptionValue::bool(true)));
    });
}

// --- Reset ---

#[test]
fn test_execute_reset() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_int_option(&harness);
    // Set non-default value
    harness
        .kernel()
        .options
        .set("tabstop", OptionValue::int(8), OptionScopeId::Global)
        .unwrap();

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("tabstop&");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        // Should be back to default (4)
        let val = runtime
            .kernel()
            .options
            .get("tabstop", OptionScopeId::Global);
        assert_eq!(val, Some(OptionValue::int(4)));
    });
}

// --- Reset option already at default ---

#[test]
fn test_execute_reset_already_default() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_int_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("tabstop&");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });
}

// --- Query ---

#[test]
fn test_execute_query() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("number?");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });
}

// --- List changed ---

#[test]
fn test_execute_list_changed() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = CommandContext::new(); // no args
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });
}

// --- List all ---

#[test]
fn test_execute_list_all() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("all");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });
}

// --- Short alias ---

#[test]
fn test_execute_short_alias() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("nu"); // short for "number"
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        let val = runtime
            .kernel()
            .options
            .get("number", OptionScopeId::Global);
        assert_eq!(val, Some(OptionValue::bool(true)));
    });
}

#[test]
fn test_execute_short_alias_query() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("nu?");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });
}

// --- Bare name on non-bool shows value ---

#[test]
fn test_execute_bare_name_non_bool_shows_value() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_int_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("tabstop"); // bare name on integer -> show
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
        // Should not error — shows value instead of trying to set bool
    });
}

// --- Error: unknown option ---

#[test]
fn test_execute_unknown_option() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("nonexistent");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_execute_unknown_option_query() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("nonexistent?");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_execute_unknown_option_toggle() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("nonexistent!");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_execute_unknown_option_assign() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("nonexistent=5");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_execute_unknown_option_reset() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("nonexistent&");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

// --- Error: type mismatch ---

#[test]
fn test_execute_type_mismatch_toggle_on_int() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_int_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("tabstop!");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_execute_type_mismatch_assign_string_to_int() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_int_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("tabstop=abc");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_execute_type_mismatch_set_bool_on_int() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_int_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        // "notabstop" -> SetBool { name: "tabstop", value: false }
        let ctx = make_ctx("notabstop");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

// --- Error: constraint violation ---

#[test]
fn test_execute_constraint_violation() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_constrained_int_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("scrolloff=-1");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

// --- EventBus emission verification ---

#[test]
fn test_event_emitted_on_set() {
    use {
        reovim_driver_text_session::testing::TestSessionRuntime,
        std::sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);

    let event_received = Arc::new(AtomicBool::new(false));
    let event_data = Arc::new(Mutex::new(None::<OptionChanged>));

    let flag = Arc::clone(&event_received);
    let data = Arc::clone(&event_data);
    let _sub = harness
        .kernel()
        .event_bus
        .subscribe::<OptionChanged, _>(0, move |event| {
            flag.store(true, Ordering::SeqCst);
            *data.lock().unwrap() = Some(event.clone());
            reovim_kernel::api::v1::EventResult::Handled
        });

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("number");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });

    assert!(event_received.load(Ordering::SeqCst), "OptionChanged event should be emitted");

    let event = event_data.lock().unwrap().clone().unwrap();
    assert_eq!(event.name, "number");
    assert_eq!(event.new_value, "true");
    assert_eq!(event.source, ChangeSource::UserCommand);
    assert_eq!(event.scope, OptionScopeId::Global);
}

#[test]
fn test_event_emitted_on_toggle() {
    use {
        reovim_driver_text_session::testing::TestSessionRuntime,
        std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);

    let event_received = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&event_received);
    let _sub = harness
        .kernel()
        .event_bus
        .subscribe::<OptionChanged, _>(0, move |_event| {
            flag.store(true, Ordering::SeqCst);
            reovim_kernel::api::v1::EventResult::Handled
        });

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("number!");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });

    assert!(
        event_received.load(Ordering::SeqCst),
        "OptionChanged event should be emitted on toggle"
    );
}

#[test]
fn test_event_emitted_on_reset() {
    use {
        reovim_driver_text_session::testing::TestSessionRuntime,
        std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_int_option(&harness);
    harness
        .kernel()
        .options
        .set("tabstop", OptionValue::int(8), OptionScopeId::Global)
        .unwrap();

    let event_received = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&event_received);
    let _sub = harness
        .kernel()
        .event_bus
        .subscribe::<OptionReset, _>(0, move |_event| {
            flag.store(true, Ordering::SeqCst);
            reovim_kernel::api::v1::EventResult::Handled
        });

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("tabstop&");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });

    assert!(
        event_received.load(Ordering::SeqCst),
        "OptionReset event should be emitted on reset"
    );
}

// --- No event on error ---

#[test]
fn test_no_event_on_error() {
    use {
        reovim_driver_text_session::testing::TestSessionRuntime,
        std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");

    let event_received = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&event_received);
    let _sub = harness
        .kernel()
        .event_bus
        .subscribe::<OptionChanged, _>(0, move |_event| {
            flag.store(true, Ordering::SeqCst);
            reovim_kernel::api::v1::EventResult::Handled
        });

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("nonexistent");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });

    assert!(!event_received.load(Ordering::SeqCst), "No event should be emitted on error");
}

// --- StateChanges verification ---

#[test]
fn test_state_changes_recorded_on_set() {
    use reovim_driver_text_session::{ChangeTracker, testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("number");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        let changes = runtime.take_changes();
        assert!(changes.option_changed);
        assert_eq!(changes.options_changed.len(), 1);
        assert_eq!(changes.options_changed[0].name, "number");
        assert_eq!(changes.options_changed[0].value, OptionValue::bool(true));
    });
}

#[test]
fn test_state_changes_recorded_on_toggle() {
    use reovim_driver_text_session::{ChangeTracker, testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("number!");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        let changes = runtime.take_changes();
        assert!(changes.option_changed);
        assert_eq!(changes.options_changed.len(), 1);
        assert_eq!(changes.options_changed[0].name, "number");
    });
}

#[test]
fn test_state_changes_recorded_on_reset() {
    use reovim_driver_text_session::{ChangeTracker, testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_int_option(&harness);
    harness
        .kernel()
        .options
        .set("tabstop", OptionValue::int(8), OptionScopeId::Global)
        .unwrap();

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("tabstop&");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        let changes = runtime.take_changes();
        assert!(changes.option_changed);
        assert_eq!(changes.options_changed.len(), 1);
        assert_eq!(changes.options_changed[0].name, "tabstop");
        // Reset records the default value
        assert_eq!(changes.options_changed[0].value, OptionValue::int(4));
    });
}

// --- old_value conversion ---

#[test]
fn test_old_value_uses_default_when_none() {
    use {
        reovim_driver_text_session::testing::TestSessionRuntime,
        std::sync::{Arc, Mutex},
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);

    let event_data = Arc::new(Mutex::new(None::<OptionChanged>));
    let data = Arc::clone(&event_data);
    let _sub = harness
        .kernel()
        .event_bus
        .subscribe::<OptionChanged, _>(0, move |event| {
            *data.lock().unwrap() = Some(event.clone());
            reovim_kernel::api::v1::EventResult::Handled
        });

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        // First set — old_value should be None (default "false")
        let ctx = make_ctx("number");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });

    let event = event_data.lock().unwrap().clone().unwrap();
    assert_eq!(event.old_value, "false"); // default display
    assert_eq!(event.new_value, "true");
}

#[test]
fn test_old_value_uses_previous_when_some() {
    use {
        reovim_driver_text_session::testing::TestSessionRuntime,
        std::sync::{Arc, Mutex},
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_int_option(&harness);
    // Set to 8 first
    harness
        .kernel()
        .options
        .set("tabstop", OptionValue::int(8), OptionScopeId::Global)
        .unwrap();

    let event_data = Arc::new(Mutex::new(None::<OptionChanged>));
    let data = Arc::clone(&event_data);
    let _sub = harness
        .kernel()
        .event_bus
        .subscribe::<OptionChanged, _>(0, move |event| {
            *data.lock().unwrap() = Some(event.clone());
            reovim_kernel::api::v1::EventResult::Handled
        });

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("tabstop=2");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });

    let event = event_data.lock().unwrap().clone().unwrap();
    assert_eq!(event.old_value, "8"); // previous explicit value
    assert_eq!(event.new_value, "2");
}

// --- List with no options registered ---

#[test]
fn test_execute_list_all_empty() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("all");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });
}

#[test]
fn test_execute_list_changed_empty() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = CommandContext::new();
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });
}

// --- List changed with changed options ---

#[test]
fn test_execute_list_changed_with_changes() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);
    harness
        .kernel()
        .options
        .set("number", OptionValue::bool(true), OptionScopeId::Global)
        .unwrap();

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = CommandContext::new();
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });
}

// --- Assign bool invalid value ---

#[test]
fn test_execute_assign_bool_invalid() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_bool_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("number=maybe");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

// --- EventBus emission on assign ---

#[test]
fn test_event_emitted_on_assign() {
    use {
        reovim_driver_text_session::testing::TestSessionRuntime,
        std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_int_option(&harness);

    let event_received = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&event_received);
    let _sub = harness
        .kernel()
        .event_bus
        .subscribe::<OptionChanged, _>(0, move |_event| {
            flag.store(true, Ordering::SeqCst);
            reovim_kernel::api::v1::EventResult::Handled
        });

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("tabstop=8");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });

    assert!(
        event_received.load(Ordering::SeqCst),
        "OptionChanged event should be emitted on assign"
    );
}

// --- StateChanges on assign ---

#[test]
fn test_state_changes_recorded_on_assign() {
    use reovim_driver_text_session::{ChangeTracker, testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    register_int_option(&harness);

    harness.with_runtime(|runtime| {
        let cmd = SetCommand;
        let ctx = make_ctx("tabstop=8");
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        let changes = runtime.take_changes();
        assert!(changes.option_changed);
        assert_eq!(changes.options_changed[0].name, "tabstop");
        assert_eq!(changes.options_changed[0].value, OptionValue::int(8));
    });
}

// --- SetAction Debug + PartialEq ---

#[test]
fn test_set_action_debug() {
    let action = SetAction::ListChanged;
    let debug = format!("{action:?}");
    assert!(debug.contains("ListChanged"));
}

#[test]
fn test_set_action_clone() {
    let action = SetAction::Show {
        name: "number".to_string(),
    };
    let cloned = action.clone();
    assert_eq!(action, cloned);
}
