//! Command handlers for the microscope fuzzy finder.
//!
//! Commands for opening pickers, navigating items, and selecting results.

use {
    reovim_driver_command::{CommandHandler, CommandQueryProvider},
    reovim_driver_picker::{PickerAction, PickerContext, PickerRegistry, push_items},
    reovim_driver_session::{BufferApi, ExtensionApi, ModeApi, SessionRuntime, TransitionContext},
    reovim_kernel::api::v1::{CommandId, OptionScopeId},
    reovim_subsys_command_types::{CommandContext, CommandResult},
};

use crate::{ids, modes::MicroscopeMode, state::MicroscopeState};

// ============================================================================
// Open picker commands
// ============================================================================

/// Open the file picker.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenFiles;

impl reovim_driver_command::Command for OpenFiles {
    fn id(&self) -> CommandId {
        ids::OPEN_FILES
    }

    fn description(&self) -> &'static str {
        "Open file picker"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for OpenFiles {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        open_picker(runtime, "files", "Files", "> ")
    }
}

/// Open the buffer picker.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenBuffers;

impl reovim_driver_command::Command for OpenBuffers {
    fn id(&self) -> CommandId {
        ids::OPEN_BUFFERS
    }

    fn description(&self) -> &'static str {
        "Open buffer picker"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for OpenBuffers {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        open_picker(runtime, "buffers", "Buffers", "> ")
    }
}

/// Open the grep picker.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenGrep;

impl reovim_driver_command::Command for OpenGrep {
    fn id(&self) -> CommandId {
        ids::OPEN_GREP
    }

    fn description(&self) -> &'static str {
        "Open grep picker"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for OpenGrep {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        open_picker(runtime, "grep", "Grep", "rg> ")
    }
}

/// Open the option picker.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenOptions;

impl reovim_driver_command::Command for OpenOptions {
    fn id(&self) -> CommandId {
        ids::OPEN_OPTIONS
    }

    fn description(&self) -> &'static str {
        "Open option picker"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for OpenOptions {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        open_picker(runtime, "options", "Options", "> ")
    }
}

/// Open the command picker.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenCommands;

impl reovim_driver_command::Command for OpenCommands {
    fn id(&self) -> CommandId {
        ids::OPEN_COMMANDS
    }

    fn description(&self) -> &'static str {
        "Open command picker"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for OpenCommands {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        open_picker(runtime, "commands", "Commands", "> ")
    }
}

// ============================================================================
// Navigation commands
// ============================================================================

/// Move selection to the next item.
#[derive(Debug, Clone, Copy, Default)]
pub struct NextItem;

impl reovim_driver_command::Command for NextItem {
    fn id(&self) -> CommandId {
        ids::NEXT_ITEM
    }

    fn description(&self) -> &'static str {
        "Select next picker item"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for NextItem {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<MicroscopeState>();
        if !state.active {
            return CommandResult::Success;
        }
        if state.matched_count > 0 {
            state.selected = (state.selected + 1) % state.matched_count as usize;
            state.update_preview();
        }
        CommandResult::Success
    }
}

/// Move selection to the previous item.
#[derive(Debug, Clone, Copy, Default)]
pub struct PrevItem;

impl reovim_driver_command::Command for PrevItem {
    fn id(&self) -> CommandId {
        ids::PREV_ITEM
    }

    fn description(&self) -> &'static str {
        "Select previous picker item"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for PrevItem {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<MicroscopeState>();
        if !state.active {
            return CommandResult::Success;
        }
        if state.matched_count > 0 {
            let count = state.matched_count as usize;
            state.selected = (state.selected + count - 1) % count;
            state.update_preview();
        }
        CommandResult::Success
    }
}

/// Select the currently highlighted item.
#[derive(Debug, Clone, Copy, Default)]
pub struct SelectItem;

impl reovim_driver_command::Command for SelectItem {
    fn id(&self) -> CommandId {
        ids::SELECT_ITEM
    }

    fn description(&self) -> &'static str {
        "Select current picker item"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for SelectItem {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Extract action and picker before closing (close_picker clears state).
        let services = runtime.kernel().services.clone();
        let (action, picker_name) = {
            let state = runtime.ext_mut::<MicroscopeState>();
            if !state.active || state.full_items.is_empty() {
                close_picker(runtime);
                return CommandResult::Success;
            }
            let selected = state.selected.min(state.full_items.len() - 1);
            let item = &state.full_items[selected];
            let name = state.picker_name.clone();

            let action = services
                .get::<PickerRegistry>()
                .and_then(|registry| registry.get(&name))
                .map_or(PickerAction::Close, |picker| picker.on_select(item));
            (action, name)
        };

        close_picker(runtime);

        // Generic dispatch: each picker handles its own action via execute().
        if let Some(registry) = services.get::<PickerRegistry>()
            && let Some(picker) = registry.get(&picker_name)
        {
            picker.execute(action, runtime);
        }
        CommandResult::Success
    }
}

/// Close the picker without selecting.
#[derive(Debug, Clone, Copy, Default)]
pub struct Close;

impl reovim_driver_command::Command for Close {
    fn id(&self) -> CommandId {
        ids::CLOSE
    }

    fn description(&self) -> &'static str {
        "Close picker"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for Close {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        close_picker(runtime);
        CommandResult::Success
    }
}

/// Delete the character before the cursor in the query.
#[derive(Debug, Clone, Copy, Default)]
pub struct Backspace;

impl reovim_driver_command::Command for Backspace {
    fn id(&self) -> CommandId {
        ids::BACKSPACE
    }

    fn description(&self) -> &'static str {
        "Delete character before cursor in picker query"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for Backspace {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<MicroscopeState>();
        if !state.active || state.cursor == 0 {
            return CommandResult::Success;
        }
        let byte_pos = state
            .query
            .char_indices()
            .nth(state.cursor - 1)
            .map_or(0, |(i, _)| i);
        let next_byte = state
            .query
            .char_indices()
            .nth(state.cursor)
            .map_or(state.query.len(), |(i, _)| i);
        state.query.drain(byte_pos..next_byte);
        state.cursor -= 1;
        state.refresh_engine();
        CommandResult::Success
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Open a picker by name, activating microscope mode.
#[cfg_attr(coverage_nightly, coverage(off))]
fn open_picker(
    runtime: &mut SessionRuntime<'_>,
    picker_name: &str,
    title: &str,
    prompt: &str,
) -> CommandResult {
    // Clone the Arc<ServiceRegistry> to drop the &runtime borrow.
    let services = runtime.kernel().services.clone();

    // Collect buffer info from kernel (needed by buffer picker).
    let buffer_ids = runtime.kernel().buffers.list();
    let buffers: Vec<_> = buffer_ids
        .into_iter()
        .map(|id| {
            let name = runtime
                .buffer_file_path(id)
                .unwrap_or_else(|| format!("[{}]", id.as_usize()));
            let modified = runtime.is_buffer_modified(id).unwrap_or(false);
            reovim_driver_picker::BufferInfo {
                id: id.as_usize(),
                name,
                modified,
            }
        })
        .collect();

    // Collect command info from CommandQueryProvider (needed by command picker).
    let commands: Vec<_> = services
        .get::<CommandQueryProvider>()
        .map_or_else(Vec::new, |svc| {
            svc.list_all()
                .iter()
                .map(|c| reovim_driver_picker::CommandInfo {
                    qualified_name: c.id.to_string(),
                    description: c.description.clone(),
                })
                .collect()
        });

    // Collect option info from OptionRegistry (needed by option picker).
    let options: Vec<_> = {
        let registry = &runtime.kernel().options;
        registry
            .list_all()
            .iter()
            .filter_map(|name| {
                let spec = registry.get_spec(name)?;
                let current = registry
                    .get(name, OptionScopeId::Global)
                    .unwrap_or_else(|| spec.default.clone());
                Some(reovim_driver_picker::OptionInfo {
                    name: name.clone(),
                    short_form: spec.short_form.as_ref().map(ToString::to_string),
                    description: spec.description.to_string(),
                    type_name: current.type_name().to_string(),
                    current_value: current.to_string(),
                    default_value: spec.default.to_string(),
                    constraint: format_constraint(&spec.constraint),
                    scope: spec.scope.display_name().to_string(),
                    owner: spec.owner().map(|m| m.as_str().to_string()),
                    choices: if let reovim_kernel::api::v1::OptionValue::Choice {
                        choices, ..
                    } = &spec.default
                    {
                        Some(choices.clone())
                    } else {
                        None
                    },
                })
            })
            .collect()
    };

    // Fetch items from the picker.
    let items = services
        .get::<PickerRegistry>()
        .and_then(|registry| registry.get(picker_name))
        .map_or_else(Vec::new, |picker| {
            if picker.is_static() {
                let ctx = PickerContext {
                    cwd: std::env::current_dir().unwrap_or_default(),
                    query: String::new(),
                    buffers,
                    commands,
                    options,
                };
                picker.items(&ctx, &services)
            } else {
                Vec::new()
            }
        });

    let state = runtime.ext_mut::<MicroscopeState>();
    state.active = true;
    state.query.clear();
    state.cursor = 0;
    state.selected = 0;
    state.scroll_offset = 0;
    picker_name.clone_into(&mut state.picker_name);
    title.clone_into(&mut state.picker_title);
    prompt.clone_into(&mut state.prompt);
    state.preview = None;
    state.services = Some(services);

    // Feed items into the engine.
    state.engine.restart();
    if !items.is_empty() {
        let injector = state.engine.injector();
        push_items(&injector, items);
    }
    state.refresh_engine();

    runtime.push_mode(MicroscopeMode::PICKER_ID, TransitionContext::new());
    CommandResult::Success
}

/// Format an `OptionConstraint` as a human-readable string.
///
/// Returns `None` for unconstrained options.
#[cfg_attr(coverage_nightly, coverage(off))]
fn format_constraint(c: &reovim_kernel::api::v1::OptionConstraint) -> Option<String> {
    match (c.min, c.max, c.min_length, c.max_length) {
        (Some(min), Some(max), _, _) => Some(format!("{min}..{max}")),
        (Some(min), None, _, _) => Some(format!(">={min}")),
        (None, Some(max), _, _) => Some(format!("<={max}")),
        (_, _, Some(min_len), Some(max_len)) => Some(format!("len {min_len}..{max_len}")),
        (_, _, Some(min_len), None) => Some(format!("len >={min_len}")),
        (_, _, None, Some(max_len)) => Some(format!("len <={max_len}")),
        _ => None,
    }
}

/// Close the picker and return to the previous mode via pop.
#[cfg_attr(coverage_nightly, coverage(off))]
fn close_picker(runtime: &mut SessionRuntime<'_>) {
    let state = runtime.ext_mut::<MicroscopeState>();
    state.active = false;
    state.full_items.clear();
    state.items.clear();
    state.engine.restart();
    state.services = None;

    let _ = runtime.pop_mode(None);
}

/// Collect all command handlers for registration.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(OpenFiles),
        Box::new(OpenBuffers),
        Box::new(OpenGrep),
        Box::new(OpenCommands),
        Box::new(OpenOptions),
        Box::new(NextItem),
        Box::new(PrevItem),
        Box::new(SelectItem),
        Box::new(Close),
        Box::new(Backspace),
    ]
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
