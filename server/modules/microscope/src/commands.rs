//! Command handlers for the microscope fuzzy finder.
//!
//! Commands for opening pickers, navigating items, and selecting results.

use {
    reovim_driver_command::CommandHandler,
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_picker::{PickerAction, PickerContext, PickerRegistry, push_items},
    reovim_driver_session::{ExtensionApi, ModeApi, SessionRuntime, TransitionContext},
    reovim_kernel::api::v1::CommandId,
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

impl CommandHandler for OpenGrep {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        open_picker(runtime, "grep", "Grep", "rg> ")
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

impl CommandHandler for NextItem {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<MicroscopeState>();
        if !state.active {
            return CommandResult::Success;
        }
        if state.matched_count > 0 {
            state.selected = (state.selected + 1) % state.matched_count as usize;
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
    // Fetch items from the picker before taking &mut state.
    // Clone the Arc<ServiceRegistry> to drop the &runtime borrow.
    let services = runtime.kernel().services.clone();
    let items = services
        .get::<PickerRegistry>()
        .and_then(|registry| registry.get(picker_name))
        .map_or_else(Vec::new, |picker| {
            if picker.is_static() {
                let ctx = PickerContext {
                    cwd: std::env::current_dir().unwrap_or_default(),
                    query: String::new(),
                    buffers: Vec::new(),
                    commands: Vec::new(),
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

    // Feed items into the engine.
    state.engine.restart();
    if !items.is_empty() {
        let injector = state.engine.injector();
        push_items(&injector, items);
    }
    state.refresh_engine();

    runtime.set_mode(MicroscopeMode::PICKER_ID, TransitionContext::new());
    CommandResult::Success
}

/// Close the picker and return to normal mode.
#[cfg_attr(coverage_nightly, coverage(off))]
fn close_picker(runtime: &mut SessionRuntime<'_>) {
    let state = runtime.ext_mut::<MicroscopeState>();
    state.active = false;
    state.full_items.clear();
    state.items.clear();
    state.engine.restart();

    // Return to vim:normal mode (discriminant 0)
    let vim_normal = reovim_kernel::api::v1::ModeId::with_discriminant(
        reovim_kernel::api::v1::ModuleId::new("vim"),
        "NORMAL",
        0,
    );
    runtime.set_mode(vim_normal, TransitionContext::new());
}

/// Collect all command handlers for registration.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(OpenFiles),
        Box::new(OpenBuffers),
        Box::new(OpenGrep),
        Box::new(OpenCommands),
        Box::new(NextItem),
        Box::new(PrevItem),
        Box::new(SelectItem),
        Box::new(Close),
        Box::new(Backspace),
    ]
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_command::Command};

    #[test]
    fn open_files_metadata() {
        let cmd = OpenFiles;
        assert_eq!(cmd.id(), ids::OPEN_FILES);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn open_buffers_metadata() {
        let cmd = OpenBuffers;
        assert_eq!(cmd.id(), ids::OPEN_BUFFERS);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn open_grep_metadata() {
        let cmd = OpenGrep;
        assert_eq!(cmd.id(), ids::OPEN_GREP);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn open_commands_metadata() {
        let cmd = OpenCommands;
        assert_eq!(cmd.id(), ids::OPEN_COMMANDS);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn next_item_metadata() {
        let cmd = NextItem;
        assert_eq!(cmd.id(), ids::NEXT_ITEM);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn prev_item_metadata() {
        let cmd = PrevItem;
        assert_eq!(cmd.id(), ids::PREV_ITEM);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn select_item_metadata() {
        let cmd = SelectItem;
        assert_eq!(cmd.id(), ids::SELECT_ITEM);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn close_metadata() {
        let cmd = Close;
        assert_eq!(cmd.id(), ids::CLOSE);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn backspace_metadata() {
        let cmd = Backspace;
        assert_eq!(cmd.id(), ids::BACKSPACE);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn command_handlers_not_empty() {
        let handlers = command_handlers();
        assert_eq!(handlers.len(), 9);
    }

    #[test]
    fn command_handlers_unique_ids() {
        let handlers = command_handlers();
        let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
        let mut deduped = ids.clone();
        deduped.sort_by_key(CommandId::name);
        deduped.dedup_by_key(|id| id.name());
        assert_eq!(ids.len(), deduped.len());
    }
}
