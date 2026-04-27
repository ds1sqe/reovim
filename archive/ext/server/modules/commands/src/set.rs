//! Set command - modify editor options at runtime.
//!
//! Implements the `:set` ex-command with vim-style syntax for querying,
//! setting, toggling, and resetting editor options.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_text_session::SessionRuntime,
    reovim_kernel::api::v1::{
        CommandId, ModuleId, OptionScopeId, OptionValue,
        events::kernel::{ChangeSource, OptionChanged, OptionReset},
    },
};

const COMMANDS_MODULE: ModuleId = ModuleId::new("commands");

/// Set command - modify editor options.
///
/// Supports vim-style syntax:
/// - `:set option` - Set boolean true (or show value for non-bool)
/// - `:set nooption` - Set boolean false
/// - `:set option!` - Toggle boolean
/// - `:set option?` - Query current value
/// - `:set option=value` - Set value
/// - `:set option&` - Reset to default
/// - `:set all` - List all options
/// - `:set` - List changed options
#[derive(Debug, Clone, Copy)]
pub struct SetCommand;

impl Command for SetCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "set")
    }

    fn description(&self) -> &'static str {
        "Set editor options. Use :set option=value, :set option, :set nooption, etc."
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "expr",
            ArgKind::Rest,
            "Option expression",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["set"]
    }
}

// ============================================================================
// Parsing
// ============================================================================

/// Parsed action from a `:set` expression.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SetAction {
    /// `:set` (no args) - list all non-default options.
    ListChanged,
    /// `:set all` - list all options.
    ListAll,
    /// `:set option?` - query current value.
    Show { name: String },
    /// `:set option` (bool) or `:set nooption` - set boolean.
    SetBool { name: String, value: bool },
    /// `:set option!` - toggle boolean.
    Toggle { name: String },
    /// `:set option=value` - assign a value.
    Assign { name: String, raw_value: String },
    /// `:set option&` - reset to default.
    Reset { name: String },
}

/// Parse a `:set` expression into a `SetAction`.
///
/// This performs structural parsing only. The ambiguous "bare name" case
/// (`:set option` — is it set-bool-true or show-for-non-bool?) is resolved
/// as `SetBool` here; the caller checks the option type and converts to
/// `Show` if the option is non-boolean.
fn parse_set_expr(expr: &str) -> SetAction {
    let expr = expr.trim();

    if expr.is_empty() {
        return SetAction::ListChanged;
    }

    if expr == "all" {
        return SetAction::ListAll;
    }

    // :set option?
    if let Some(name) = expr.strip_suffix('?') {
        return SetAction::Show {
            name: name.to_string(),
        };
    }

    // :set option!
    if let Some(name) = expr.strip_suffix('!') {
        return SetAction::Toggle {
            name: name.to_string(),
        };
    }

    // :set option&
    if let Some(name) = expr.strip_suffix('&') {
        return SetAction::Reset {
            name: name.to_string(),
        };
    }

    // :set option=value
    if let Some((name, value)) = expr.split_once('=') {
        return SetAction::Assign {
            name: name.to_string(),
            raw_value: value.to_string(),
        };
    }

    // :set nooption
    if let Some(name) = expr.strip_prefix("no")
        && !name.is_empty()
    {
        return SetAction::SetBool {
            name: name.to_string(),
            value: false,
        };
    }

    // :set option (bare name — assume bool true; caller refines if non-bool)
    SetAction::SetBool {
        name: expr.to_string(),
        value: true,
    }
}

/// Parse a raw string value into an `OptionValue` matching the expected type.
fn parse_value_for_type(
    raw: &str,
    expected: &OptionValue,
    name: &str,
) -> Result<OptionValue, String> {
    match expected {
        OptionValue::Bool(_) => match raw {
            "true" | "1" | "on" => Ok(OptionValue::bool(true)),
            "false" | "0" | "off" => Ok(OptionValue::bool(false)),
            _ => Err(format!(
                "invalid value for '{name}': expected bool (true/false/1/0/on/off), got '{raw}'"
            )),
        },
        OptionValue::Integer(_) => {
            let val: i64 = raw.parse().map_err(|_| {
                format!("invalid value for '{name}': expected integer, got '{raw}'")
            })?;
            Ok(OptionValue::int(val))
        }
        OptionValue::String(_) => Ok(OptionValue::string(raw)),
        OptionValue::Choice { choices, .. } => Ok(OptionValue::choice(raw, choices.clone())),
    }
}

// ============================================================================
// Execution
// ============================================================================

impl CommandHandler for SetCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        let expr = ctx.string("expr").unwrap_or("");
        let action = parse_set_expr(expr);
        let scope = OptionScopeId::Global;

        match action {
            SetAction::ListChanged => execute_list_changed(runtime, scope),
            SetAction::ListAll => execute_list_all(runtime, scope),
            SetAction::Show { name } => execute_show(runtime, &name, scope),
            SetAction::SetBool { name, value } => execute_set_bool(runtime, &name, value, scope),
            SetAction::Toggle { name } => execute_toggle(runtime, &name, scope),
            SetAction::Assign { name, raw_value } => {
                execute_assign(runtime, &name, &raw_value, scope)
            }
            SetAction::Reset { name } => execute_reset(runtime, &name, scope),
        }
    }
}

/// Execute `:set` (no args) — list options with non-default values.
fn execute_list_changed(runtime: &SessionRuntime<'_>, scope: OptionScopeId) -> CommandResult {
    let kernel = runtime.kernel();
    let options = &kernel.options;
    let names = options.list_all();

    let mut lines = Vec::new();
    for name in &names {
        // list_all() returns registered names, so get_spec/get are guaranteed Some.
        let (spec, current) = guard_spec_and_value(options, name, scope);
        if current != spec.default {
            lines.push(format!("  {name}={current}"));
        }
    }

    log_option_list("Changed options", "No changed options", &lines);
    CommandResult::Success
}

/// Execute `:set all` — list all options.
fn execute_list_all(runtime: &SessionRuntime<'_>, scope: OptionScopeId) -> CommandResult {
    let kernel = runtime.kernel();
    let options = &kernel.options;
    let names = options.list_all();

    let mut lines = Vec::new();
    for name in &names {
        // list_all() returns registered names, so get is guaranteed Some.
        let value = guard_get_value(options, name, scope);
        lines.push(format!("  {name}={value}"));
    }

    log_option_list("All options", "No options registered", &lines);
    CommandResult::Success
}

/// Execute `:set option?` — show the current value.
fn execute_show(runtime: &SessionRuntime<'_>, name: &str, scope: OptionScopeId) -> CommandResult {
    let kernel = runtime.kernel();
    let options = &kernel.options;

    let Some(full_name) = options.resolve_name(name) else {
        return CommandResult::Error(format!("Unknown option: {name}"));
    };

    // resolve_name succeeded, so get is guaranteed Some.
    let value = guard_get_value(options, &full_name, scope);
    log_option_value(&full_name, &value);
    CommandResult::Success
}

/// Log an option list (tracing output only).
#[cfg_attr(coverage_nightly, coverage(off))]
fn log_option_list(header: &str, empty_msg: &str, lines: &[String]) {
    if lines.is_empty() {
        tracing::info!("{empty_msg}");
    } else {
        tracing::info!("{header}:\n{}", lines.join("\n"));
    }
}

/// Log a single option value (tracing output only).
#[cfg_attr(coverage_nightly, coverage(off))]
fn log_option_value(name: &str, value: &OptionValue) {
    tracing::info!("  {name}={value}");
}

// =============================================================================
// Defensive guards for unreachable branches
//
// After `resolve_name()` / `list_all()` confirms an option exists, `get_spec()`,
// `get()`, and `reset()` are guaranteed to succeed. These helpers isolate the
// unreachable `None`/`Err` branches so MC/DC coverage is not penalized.
// =============================================================================

use reovim_kernel::api::v1::OptionRegistry;

/// Get spec for a resolved option name (guaranteed `Some` after `resolve_name`).
#[cfg_attr(coverage_nightly, coverage(off))]
fn guard_get_spec(options: &OptionRegistry, full_name: &str) -> reovim_kernel::api::v1::OptionSpec {
    options
        .get_spec(full_name)
        .expect("get_spec must succeed after resolve_name")
}

/// Get value for a resolved option name (guaranteed `Some` after `resolve_name`).
#[cfg_attr(coverage_nightly, coverage(off))]
fn guard_get_value(options: &OptionRegistry, full_name: &str, scope: OptionScopeId) -> OptionValue {
    options
        .get(full_name, scope)
        .expect("get must succeed after resolve_name")
}

/// Get spec and value together for a known-registered option name.
#[cfg_attr(coverage_nightly, coverage(off))]
fn guard_spec_and_value(
    options: &OptionRegistry,
    name: &str,
    scope: OptionScopeId,
) -> (reovim_kernel::api::v1::OptionSpec, OptionValue) {
    let spec = options
        .get_spec(name)
        .expect("get_spec must succeed for list_all name");
    let value = options
        .get(name, scope)
        .expect("get must succeed for list_all name");
    (spec, value)
}

/// Reset a resolved option (guaranteed `Ok` after `resolve_name`).
#[cfg_attr(coverage_nightly, coverage(off))]
fn guard_reset(
    options: &OptionRegistry,
    full_name: &str,
    scope: OptionScopeId,
) -> Option<OptionValue> {
    options
        .reset(full_name, scope)
        .expect("reset must succeed after resolve_name")
}

/// Execute `:set option` or `:set nooption` — set a boolean option.
///
/// For bare names (`:set option`), if the option is non-boolean, falls
/// back to showing the current value instead of setting.
fn execute_set_bool(
    runtime: &mut SessionRuntime<'_>,
    name: &str,
    value: bool,
    scope: OptionScopeId,
) -> CommandResult {
    let kernel = runtime.kernel();
    let options = &kernel.options;

    let Some(full_name) = options.resolve_name(name) else {
        return CommandResult::Error(format!("Unknown option: {name}"));
    };

    // resolve_name succeeded, so get_spec is guaranteed Some.
    let spec = guard_get_spec(options, &full_name);

    // If setting to true via bare name and option is non-bool, show value instead
    if value && !matches!(spec.default, OptionValue::Bool(_)) {
        return execute_show(runtime, &full_name, scope);
    }

    let new_value = OptionValue::bool(value);
    match options.set(&full_name, new_value.clone(), scope) {
        Ok(result) => {
            let old_display = result
                .old_value
                .as_ref()
                .map_or_else(|| spec.default.to_string(), ToString::to_string);

            kernel.event_bus.emit(OptionChanged {
                name: full_name.clone(),
                old_value: old_display,
                new_value: result.new_value.to_string(),
                source: ChangeSource::UserCommand,
                scope,
            });

            runtime.record_global_option_change(&full_name, new_value);
            CommandResult::Success
        }
        Err(e) => CommandResult::Error(format!("{e}")),
    }
}

/// Execute `:set option!` — toggle a boolean option.
fn execute_toggle(
    runtime: &mut SessionRuntime<'_>,
    name: &str,
    scope: OptionScopeId,
) -> CommandResult {
    let kernel = runtime.kernel();
    let options = &kernel.options;

    let Some(full_name) = options.resolve_name(name) else {
        return CommandResult::Error(format!("Unknown option: {name}"));
    };

    // Get old value before toggle for the event
    let old_value = options.get(&full_name, scope);
    let old_display = old_value
        .as_ref()
        .map_or_else(String::new, ToString::to_string);

    match options.toggle(&full_name, scope) {
        Ok(new_bool) => {
            let new_value = OptionValue::bool(new_bool);

            kernel.event_bus.emit(OptionChanged {
                name: full_name.clone(),
                old_value: old_display,
                new_value: new_value.to_string(),
                source: ChangeSource::UserCommand,
                scope,
            });

            runtime.record_global_option_change(&full_name, new_value);
            CommandResult::Success
        }
        Err(e) => CommandResult::Error(format!("{e}")),
    }
}

/// Execute `:set option=value` — assign a typed value.
fn execute_assign(
    runtime: &mut SessionRuntime<'_>,
    name: &str,
    raw_value: &str,
    scope: OptionScopeId,
) -> CommandResult {
    let kernel = runtime.kernel();
    let options = &kernel.options;

    let Some(full_name) = options.resolve_name(name) else {
        return CommandResult::Error(format!("Unknown option: {name}"));
    };

    // resolve_name succeeded, so get_spec is guaranteed Some.
    let spec = guard_get_spec(options, &full_name);

    let new_value = match parse_value_for_type(raw_value, &spec.default, &full_name) {
        Ok(v) => v,
        Err(msg) => return CommandResult::Error(msg),
    };

    match options.set(&full_name, new_value.clone(), scope) {
        Ok(result) => {
            let old_display = result
                .old_value
                .as_ref()
                .map_or_else(|| spec.default.to_string(), ToString::to_string);

            kernel.event_bus.emit(OptionChanged {
                name: full_name.clone(),
                old_value: old_display,
                new_value: result.new_value.to_string(),
                source: ChangeSource::UserCommand,
                scope,
            });

            runtime.record_global_option_change(&full_name, new_value);
            CommandResult::Success
        }
        Err(e) => CommandResult::Error(format!("{e}")),
    }
}

/// Execute `:set option&` — reset to default.
fn execute_reset(
    runtime: &mut SessionRuntime<'_>,
    name: &str,
    scope: OptionScopeId,
) -> CommandResult {
    let kernel = runtime.kernel();
    let options = &kernel.options;

    let Some(full_name) = options.resolve_name(name) else {
        return CommandResult::Error(format!("Unknown option: {name}"));
    };

    // resolve_name succeeded, so get_spec and reset are guaranteed to succeed.
    let spec = guard_get_spec(options, &full_name);
    let old_value = guard_reset(options, &full_name, scope);

    let old_display = old_value
        .as_ref()
        .map_or_else(|| spec.default.to_string(), ToString::to_string);

    kernel.event_bus.emit(OptionReset {
        name: full_name.clone(),
        old_value: old_display,
        default_value: spec.default.to_string(),
        scope,
    });

    runtime.record_global_option_change(&full_name, spec.default);
    CommandResult::Success
}

#[cfg(test)]
#[path = "set_tests.rs"]
mod tests;
