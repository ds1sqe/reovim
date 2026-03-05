//! Command handlers for the explorer sidebar.
//!
//! Provides all command handlers: toggle/close, navigation (cursor, scroll,
//! expand/collapse, open, goto-parent), file operations (create file/dir,
//! rename, delete with confirmation), and clipboard (yank-path).

use {
    reovim_driver_command::CommandHandler,
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_session::{
        BufferApi, ClipboardApi, ExtensionApi, ModeApi, SessionRuntime, TransitionContext,
        WindowApi,
    },
    reovim_kernel::api::v1::CommandId,
    std::path::{Path, PathBuf},
};

use crate::{ids, modes::ExplorerMode, state::ExplorerState, tree::FileTree};

/// Vim normal mode ID constant.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn vim_normal() -> reovim_kernel::api::v1::ModeId {
    reovim_kernel::api::v1::ModeId::with_discriminant(
        reovim_kernel::api::v1::ModuleId::new("vim"),
        "NORMAL",
        0,
    )
}

// ============================================================================
// Toggle / Close
// ============================================================================

/// Toggle the explorer sidebar visibility.
#[derive(Debug, Clone, Copy, Default)]
pub struct Toggle;

impl reovim_driver_command::Command for Toggle {
    fn id(&self) -> CommandId {
        ids::TOGGLE
    }

    fn description(&self) -> &'static str {
        "Toggle file explorer"
    }
}

impl CommandHandler for Toggle {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();

        if state.active {
            state.active = false;
            state.reset_snapshot_generation();
            runtime.set_mode(vim_normal(), TransitionContext::new());
        } else {
            state.active = true;

            // Initialize tree if not yet loaded.
            if state.tree.is_none() {
                let root = if state.root_path.as_os_str().is_empty() {
                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                } else {
                    state.root_path.clone()
                };

                if let Some(vfs) = args.vfs() {
                    match FileTree::new(root.clone(), vfs.as_ref()) {
                        Ok(tree) => {
                            state.root_path = root;
                            state.tree = Some(tree);
                            state.invalidate_tree_cache();
                        }
                        Err(e) => {
                            state.message = Some(format!("Failed to load tree: {e}"));
                        }
                    }
                }
            }

            runtime.set_mode(ExplorerMode::BROWSE_ID, TransitionContext::new());
        }

        CommandResult::Success
    }
}

/// Close the explorer sidebar.
#[derive(Debug, Clone, Copy, Default)]
pub struct Close;

impl reovim_driver_command::Command for Close {
    fn id(&self) -> CommandId {
        ids::CLOSE
    }

    fn description(&self) -> &'static str {
        "Close file explorer"
    }
}

impl CommandHandler for Close {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        state.active = false;
        state.reset_snapshot_generation();

        runtime.set_mode(vim_normal(), TransitionContext::new());

        CommandResult::Success
    }
}

// ============================================================================
// Navigation
// ============================================================================

/// Move cursor up in the tree.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorUp;

impl reovim_driver_command::Command for CursorUp {
    fn id(&self) -> CommandId {
        ids::CURSOR_UP
    }

    fn description(&self) -> &'static str {
        "Move cursor up"
    }
}

impl CommandHandler for CursorUp {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        let count = state.node_count();
        if state.cursor_index > 0 {
            state.cursor_index -= 1;
        }
        state.update_scroll_with_count(count);
        CommandResult::Success
    }
}

/// Move cursor down in the tree.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorDown;

impl reovim_driver_command::Command for CursorDown {
    fn id(&self) -> CommandId {
        ids::CURSOR_DOWN
    }

    fn description(&self) -> &'static str {
        "Move cursor down"
    }
}

impl CommandHandler for CursorDown {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        let count = state.node_count();
        if count > 0 && state.cursor_index < count - 1 {
            state.cursor_index += 1;
        }
        state.update_scroll_with_count(count);
        CommandResult::Success
    }
}

/// Go to the first item in the tree.
#[derive(Debug, Clone, Copy, Default)]
pub struct GotoFirst;

impl reovim_driver_command::Command for GotoFirst {
    fn id(&self) -> CommandId {
        ids::GOTO_FIRST
    }

    fn description(&self) -> &'static str {
        "Go to first item"
    }
}

impl CommandHandler for GotoFirst {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        let count = state.node_count();
        state.cursor_index = 0;
        state.update_scroll_with_count(count);
        CommandResult::Success
    }
}

/// Go to the last item in the tree.
#[derive(Debug, Clone, Copy, Default)]
pub struct GotoLast;

impl reovim_driver_command::Command for GotoLast {
    fn id(&self) -> CommandId {
        ids::GOTO_LAST
    }

    fn description(&self) -> &'static str {
        "Go to last item"
    }
}

impl CommandHandler for GotoLast {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        let count = state.node_count();
        if count > 0 {
            state.cursor_index = count - 1;
        }
        state.update_scroll_with_count(count);
        CommandResult::Success
    }
}

/// Expand a directory node.
#[derive(Debug, Clone, Copy, Default)]
pub struct Expand;

impl reovim_driver_command::Command for Expand {
    fn id(&self) -> CommandId {
        ids::EXPAND
    }

    fn description(&self) -> &'static str {
        "Expand directory"
    }
}

impl CommandHandler for Expand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        let Some(tree) = &state.tree else {
            return CommandResult::Success;
        };

        let nodes = tree.flatten(state.show_hidden);
        let Some(node) = nodes.get(state.cursor_index) else {
            return CommandResult::Success;
        };

        if !node.is_dir() || node.is_expanded() {
            return CommandResult::Success;
        }

        let path = node.path.clone();
        let Some(vfs) = args.vfs() else {
            return CommandResult::error("VFS not available");
        };

        let state = runtime.ext_mut::<ExplorerState>();
        if let Some(tree) = &mut state.tree
            && let Err(e) = tree.expand(&path, vfs.as_ref())
        {
            state.message = Some(format!("Expand failed: {e}"));
        }
        state.invalidate_tree_cache();

        CommandResult::Success
    }
}

/// Collapse a directory node or go to parent.
#[derive(Debug, Clone, Copy, Default)]
pub struct Collapse;

impl reovim_driver_command::Command for Collapse {
    fn id(&self) -> CommandId {
        ids::COLLAPSE
    }

    fn description(&self) -> &'static str {
        "Collapse directory or go to parent"
    }
}

impl CommandHandler for Collapse {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        let Some(tree) = &state.tree else {
            return CommandResult::Success;
        };

        let nodes = tree.flatten(state.show_hidden);
        let Some(node) = nodes.get(state.cursor_index) else {
            return CommandResult::Success;
        };

        if node.is_dir() && node.is_expanded() {
            let path = node.path.clone();
            let state = runtime.ext_mut::<ExplorerState>();
            if let Some(tree) = &mut state.tree {
                tree.collapse(&path);
            }
            state.invalidate_tree_cache();
        } else if node.depth > 0 {
            let target_depth = node.depth - 1;
            let cursor = state.cursor_index;
            for i in (0..cursor).rev() {
                if let Some(n) = nodes.get(i)
                    && n.depth == target_depth
                {
                    let state = runtime.ext_mut::<ExplorerState>();
                    state.cursor_index = i;
                    state.update_scroll();
                    return CommandResult::Success;
                }
            }
        }

        CommandResult::Success
    }
}

/// Open the selected file or toggle directory.
#[derive(Debug, Clone, Copy, Default)]
pub struct Open;

impl reovim_driver_command::Command for Open {
    fn id(&self) -> CommandId {
        ids::OPEN
    }

    fn description(&self) -> &'static str {
        "Open file or toggle directory"
    }
}

impl CommandHandler for Open {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        let Some(tree) = &state.tree else {
            return CommandResult::Success;
        };

        let nodes = tree.flatten(state.show_hidden);
        let Some(node) = nodes.get(state.cursor_index) else {
            return CommandResult::Success;
        };

        let path = node.path.clone();
        let is_dir = node.is_dir();
        let Some(vfs) = args.vfs() else {
            return CommandResult::error("VFS not available");
        };

        if is_dir {
            let state = runtime.ext_mut::<ExplorerState>();
            if let Some(tree) = &mut state.tree
                && let Err(e) = tree.toggle(&path, vfs.as_ref())
            {
                state.message = Some(format!("Toggle failed: {e}"));
            }
            state.invalidate_tree_cache();
        } else {
            match vfs.read(&path) {
                Ok(content) => {
                    let text = String::from_utf8_lossy(&content);
                    let buf_id = runtime.create_buffer(Some(&path.to_string_lossy()), &text);
                    runtime.set_active_buffer(Some(buf_id));
                    runtime.record_buffer_modified(buf_id);

                    if let Some(window) = runtime.active_window() {
                        let _ = runtime.set_window_buffer(window, buf_id);
                    }

                    let state = runtime.ext_mut::<ExplorerState>();
                    state.active = false;
                    state.reset_snapshot_generation();
                    runtime.set_mode(vim_normal(), TransitionContext::new());
                }
                Err(e) => {
                    let state = runtime.ext_mut::<ExplorerState>();
                    state.message = Some(format!("Open failed: {e}"));
                }
            }
        }

        CommandResult::Success
    }
}

/// Navigate to the parent directory.
#[derive(Debug, Clone, Copy, Default)]
pub struct GotoParent;

impl reovim_driver_command::Command for GotoParent {
    fn id(&self) -> CommandId {
        ids::GOTO_PARENT
    }

    fn description(&self) -> &'static str {
        "Go to parent directory"
    }
}

impl CommandHandler for GotoParent {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        let Some(tree) = &state.tree else {
            return CommandResult::Success;
        };

        let nodes = tree.flatten(state.show_hidden);
        let Some(node) = nodes.get(state.cursor_index) else {
            return CommandResult::Success;
        };

        if node.depth == 0 {
            return CommandResult::Success;
        }

        let target_depth = node.depth - 1;
        let cursor = state.cursor_index;
        for i in (0..cursor).rev() {
            if let Some(n) = nodes.get(i)
                && n.depth == target_depth
            {
                let state = runtime.ext_mut::<ExplorerState>();
                state.cursor_index = i;
                state.update_scroll();
                return CommandResult::Success;
            }
        }

        CommandResult::Success
    }
}

/// Toggle display of hidden files.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleHidden;

impl reovim_driver_command::Command for ToggleHidden {
    fn id(&self) -> CommandId {
        ids::TOGGLE_HIDDEN
    }

    fn description(&self) -> &'static str {
        "Toggle hidden files"
    }
}

impl CommandHandler for ToggleHidden {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        state.show_hidden = !state.show_hidden;
        state.invalidate_tree_cache();
        state.update_scroll();
        CommandResult::Success
    }
}

/// Refresh the tree from filesystem.
#[derive(Debug, Clone, Copy, Default)]
pub struct Refresh;

impl reovim_driver_command::Command for Refresh {
    fn id(&self) -> CommandId {
        ids::REFRESH
    }

    fn description(&self) -> &'static str {
        "Refresh file tree"
    }
}

impl CommandHandler for Refresh {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(vfs) = args.vfs() else {
            return CommandResult::error("VFS not available");
        };

        let state = runtime.ext_mut::<ExplorerState>();
        if let Some(tree) = &mut state.tree
            && let Err(e) = tree.refresh(vfs.as_ref())
        {
            state.message = Some(format!("Refresh failed: {e}"));
        }
        state.invalidate_tree_cache();
        state.update_scroll();
        CommandResult::Success
    }
}

// ============================================================================
// File Operations (Phase 5)
// ============================================================================

/// Begin creating a new file (enter input mode).
#[derive(Debug, Clone, Copy, Default)]
pub struct CreateFile;

impl reovim_driver_command::Command for CreateFile {
    fn id(&self) -> CommandId {
        ids::CREATE_FILE
    }

    fn description(&self) -> &'static str {
        "Create new file"
    }
}

impl CommandHandler for CreateFile {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        state.input_mode = crate::state::ExplorerInputMode::CreateFile;
        state.input_buffer.clear();
        state.message = None;
        runtime.set_mode(ExplorerMode::INPUT_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Begin creating a new directory (enter input mode).
#[derive(Debug, Clone, Copy, Default)]
pub struct CreateDir;

impl reovim_driver_command::Command for CreateDir {
    fn id(&self) -> CommandId {
        ids::CREATE_DIR
    }

    fn description(&self) -> &'static str {
        "Create new directory"
    }
}

impl CommandHandler for CreateDir {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        state.input_mode = crate::state::ExplorerInputMode::CreateDir;
        state.input_buffer.clear();
        state.message = None;
        runtime.set_mode(ExplorerMode::INPUT_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Begin renaming the selected item (enter input mode).
#[derive(Debug, Clone, Copy, Default)]
pub struct Rename;

impl reovim_driver_command::Command for Rename {
    fn id(&self) -> CommandId {
        ids::RENAME
    }

    fn description(&self) -> &'static str {
        "Rename selected item"
    }
}

impl CommandHandler for Rename {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        let Some(tree) = &state.tree else {
            return CommandResult::Success;
        };

        let nodes = tree.flatten(state.show_hidden);
        let Some(node) = nodes.get(state.cursor_index) else {
            return CommandResult::Success;
        };

        // Pre-fill buffer with current name.
        let name = node.name.clone();
        let state = runtime.ext_mut::<ExplorerState>();
        state.input_mode = crate::state::ExplorerInputMode::Rename;
        state.input_buffer = name;
        state.message = None;
        runtime.set_mode(ExplorerMode::INPUT_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Begin deleting the selected item (enter confirm mode).
#[derive(Debug, Clone, Copy, Default)]
pub struct Delete;

impl reovim_driver_command::Command for Delete {
    fn id(&self) -> CommandId {
        ids::DELETE
    }

    fn description(&self) -> &'static str {
        "Delete selected item"
    }
}

impl CommandHandler for Delete {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        state.input_mode = crate::state::ExplorerInputMode::ConfirmDelete;
        state.input_buffer.clear();
        state.message = None;
        runtime.set_mode(ExplorerMode::INPUT_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Confirm the current input operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConfirmInput;

impl reovim_driver_command::Command for ConfirmInput {
    fn id(&self) -> CommandId {
        ids::CONFIRM_INPUT
    }

    fn description(&self) -> &'static str {
        "Confirm input operation"
    }
}

impl CommandHandler for ConfirmInput {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        let input_mode = state.input_mode;
        let input = state.input_buffer.clone();

        let Some(tree) = &state.tree else {
            return cancel_input(runtime);
        };

        // Determine the parent path for the operation.
        let nodes = tree.flatten(state.show_hidden);
        let cursor_path = nodes.get(state.cursor_index).map(|n| n.path.clone());

        let Some(vfs) = args.vfs() else {
            return CommandResult::error("VFS not available");
        };

        match input_mode {
            crate::state::ExplorerInputMode::CreateFile => {
                let parent = cursor_dir_path(&nodes, state.cursor_index);
                let target = parent.join(&input);
                let state = runtime.ext_mut::<ExplorerState>();
                if let Err(e) = vfs.write(&target, &[]) {
                    state.message = Some(format!("Create file failed: {e}"));
                } else if let Some(tree) = &mut state.tree {
                    let _ = tree.refresh(vfs.as_ref());
                }
                state.invalidate_tree_cache();
            }
            crate::state::ExplorerInputMode::CreateDir => {
                let parent = cursor_dir_path(&nodes, state.cursor_index);
                let target = parent.join(&input);
                let state = runtime.ext_mut::<ExplorerState>();
                if let Err(e) = vfs.create_dir(&target) {
                    state.message = Some(format!("Create dir failed: {e}"));
                } else if let Some(tree) = &mut state.tree {
                    let _ = tree.refresh(vfs.as_ref());
                }
                state.invalidate_tree_cache();
            }
            crate::state::ExplorerInputMode::Rename => {
                if let Some(old_path) = cursor_path {
                    let parent = old_path
                        .parent()
                        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
                    let new_path = parent.join(&input);
                    let state = runtime.ext_mut::<ExplorerState>();
                    if let Err(e) = vfs.rename(&old_path, &new_path) {
                        state.message = Some(format!("Rename failed: {e}"));
                    } else if let Some(tree) = &mut state.tree {
                        let _ = tree.refresh(vfs.as_ref());
                    }
                    state.invalidate_tree_cache();
                }
            }
            crate::state::ExplorerInputMode::ConfirmDelete => {
                if input == "y"
                    && let Some(path) = cursor_path
                {
                    let state = runtime.ext_mut::<ExplorerState>();
                    if let Err(e) = vfs.delete(&path) {
                        state.message = Some(format!("Delete failed: {e}"));
                    } else if let Some(tree) = &mut state.tree {
                        let _ = tree.refresh(vfs.as_ref());
                    }
                    state.invalidate_tree_cache();
                    state.update_scroll();
                }
            }
            crate::state::ExplorerInputMode::None => {}
        }

        cancel_input(runtime)
    }
}

/// Cancel the current input operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct CancelInput;

impl reovim_driver_command::Command for CancelInput {
    fn id(&self) -> CommandId {
        ids::CANCEL_INPUT
    }

    fn description(&self) -> &'static str {
        "Cancel input operation"
    }
}

impl CommandHandler for CancelInput {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        cancel_input(runtime)
    }
}

/// Delete the last character from the input buffer.
#[derive(Debug, Clone, Copy, Default)]
pub struct InputBackspace;

impl reovim_driver_command::Command for InputBackspace {
    fn id(&self) -> CommandId {
        ids::INPUT_BACKSPACE
    }

    fn description(&self) -> &'static str {
        "Delete character from input"
    }
}

impl CommandHandler for InputBackspace {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        state.input_buffer.pop();
        CommandResult::Success
    }
}

// ============================================================================
// Clipboard
// ============================================================================

/// Copy the selected item's path to clipboard.
#[derive(Debug, Clone, Copy, Default)]
pub struct YankPath;

impl reovim_driver_command::Command for YankPath {
    fn id(&self) -> CommandId {
        ids::YANK_PATH
    }

    fn description(&self) -> &'static str {
        "Copy path to clipboard"
    }
}

impl CommandHandler for YankPath {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ExplorerState>();
        let Some(tree) = &state.tree else {
            return CommandResult::Success;
        };

        let nodes = tree.flatten(state.show_hidden);
        let Some(node) = nodes.get(state.cursor_index) else {
            return CommandResult::Success;
        };

        let path_str = node.path.to_string_lossy().to_string();
        let copied = runtime.copy_to_clipboard(&path_str);
        let state = runtime.ext_mut::<ExplorerState>();
        if copied {
            state.message = Some(format!("Copied: {path_str}"));
        } else {
            state.message = Some("Clipboard not available".to_owned());
        }

        CommandResult::Success
    }
}

/// Shared helper: cancel input mode and return to browse.
#[cfg_attr(coverage_nightly, coverage(off))]
fn cancel_input(runtime: &mut SessionRuntime<'_>) -> CommandResult {
    let state = runtime.ext_mut::<ExplorerState>();
    state.input_mode = crate::state::ExplorerInputMode::None;
    state.input_buffer.clear();
    runtime.set_mode(ExplorerMode::BROWSE_ID, TransitionContext::new());
    CommandResult::Success
}

/// Get the directory path at the cursor position.
///
/// If the cursor is on a directory, returns that directory's path.
/// If the cursor is on a file, returns the parent directory's path.
fn cursor_dir_path(nodes: &[&crate::tree::node::FileNode], cursor_index: usize) -> PathBuf {
    nodes.get(cursor_index).map_or_else(
        || PathBuf::from("."),
        |n| {
            if n.is_dir() {
                n.path.clone()
            } else {
                n.path
                    .parent()
                    .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
            }
        },
    )
}

/// Collect all command handlers for registration.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(Toggle),
        Box::new(Close),
        Box::new(CursorUp),
        Box::new(CursorDown),
        Box::new(GotoFirst),
        Box::new(GotoLast),
        Box::new(Expand),
        Box::new(Collapse),
        Box::new(Open),
        Box::new(GotoParent),
        Box::new(ToggleHidden),
        Box::new(Refresh),
        Box::new(CreateFile),
        Box::new(CreateDir),
        Box::new(Rename),
        Box::new(Delete),
        Box::new(ConfirmInput),
        Box::new(CancelInput),
        Box::new(InputBackspace),
        Box::new(YankPath),
    ]
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_command::Command};

    #[test]
    fn toggle_metadata() {
        let cmd = Toggle;
        assert_eq!(cmd.id(), ids::TOGGLE);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn close_metadata() {
        let cmd = Close;
        assert_eq!(cmd.id(), ids::CLOSE);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn cursor_up_metadata() {
        let cmd = CursorUp;
        assert_eq!(cmd.id(), ids::CURSOR_UP);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn cursor_down_metadata() {
        let cmd = CursorDown;
        assert_eq!(cmd.id(), ids::CURSOR_DOWN);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn goto_first_metadata() {
        let cmd = GotoFirst;
        assert_eq!(cmd.id(), ids::GOTO_FIRST);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn goto_last_metadata() {
        let cmd = GotoLast;
        assert_eq!(cmd.id(), ids::GOTO_LAST);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn expand_metadata() {
        let cmd = Expand;
        assert_eq!(cmd.id(), ids::EXPAND);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn collapse_metadata() {
        let cmd = Collapse;
        assert_eq!(cmd.id(), ids::COLLAPSE);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn open_metadata() {
        let cmd = Open;
        assert_eq!(cmd.id(), ids::OPEN);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn goto_parent_metadata() {
        let cmd = GotoParent;
        assert_eq!(cmd.id(), ids::GOTO_PARENT);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn toggle_hidden_metadata() {
        let cmd = ToggleHidden;
        assert_eq!(cmd.id(), ids::TOGGLE_HIDDEN);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn refresh_metadata() {
        let cmd = Refresh;
        assert_eq!(cmd.id(), ids::REFRESH);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn create_file_metadata() {
        let cmd = CreateFile;
        assert_eq!(cmd.id(), ids::CREATE_FILE);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn create_dir_metadata() {
        let cmd = CreateDir;
        assert_eq!(cmd.id(), ids::CREATE_DIR);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn rename_metadata() {
        let cmd = Rename;
        assert_eq!(cmd.id(), ids::RENAME);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn delete_metadata() {
        let cmd = Delete;
        assert_eq!(cmd.id(), ids::DELETE);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn confirm_input_metadata() {
        let cmd = ConfirmInput;
        assert_eq!(cmd.id(), ids::CONFIRM_INPUT);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn cancel_input_metadata() {
        let cmd = CancelInput;
        assert_eq!(cmd.id(), ids::CANCEL_INPUT);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn input_backspace_metadata() {
        let cmd = InputBackspace;
        assert_eq!(cmd.id(), ids::INPUT_BACKSPACE);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn yank_path_metadata() {
        let cmd = YankPath;
        assert_eq!(cmd.id(), ids::YANK_PATH);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn command_handlers_count() {
        let handlers = command_handlers();
        assert_eq!(handlers.len(), 20);
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

    #[test]
    fn toggle_debug() {
        let debug = format!("{Toggle:?}");
        assert!(debug.contains("Toggle"));
    }

    #[test]
    fn close_debug() {
        let debug = format!("{Close:?}");
        assert!(debug.contains("Close"));
    }

    #[test]
    fn cursor_up_debug() {
        let debug = format!("{CursorUp:?}");
        assert!(debug.contains("CursorUp"));
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn all_default_constructable() {
        let _ = Toggle::default();
        let _ = Close::default();
        let _ = CursorUp::default();
        let _ = CursorDown::default();
        let _ = GotoFirst::default();
        let _ = GotoLast::default();
        let _ = Expand::default();
        let _ = Collapse::default();
        let _ = Open::default();
        let _ = GotoParent::default();
        let _ = ToggleHidden::default();
        let _ = Refresh::default();
        let _ = CreateFile::default();
        let _ = CreateDir::default();
        let _ = Rename::default();
        let _ = Delete::default();
        let _ = ConfirmInput::default();
        let _ = CancelInput::default();
        let _ = InputBackspace::default();
        let _ = YankPath::default();
    }

    #[test]
    fn cursor_dir_path_on_dir() {
        use {crate::tree::node::FileNode, reovim_driver_vfs::MockVfs, std::path::Path};

        let vfs = MockVfs::new();
        vfs.add_dir("/root");
        vfs.add_dir("/root/src");
        let mut root = FileNode::from_path(Path::new("/root"), &vfs, 0).unwrap();
        root.set_expanded(true);
        root.load_children(&vfs).unwrap();

        let nodes: Vec<&FileNode> = vec![&root];
        let result = cursor_dir_path(&nodes, 0);
        assert_eq!(result, PathBuf::from("/root"));
    }

    #[test]
    fn cursor_dir_path_on_file() {
        use {
            crate::tree::node::{FileNode, NodeType},
            std::path::PathBuf,
        };

        let file_node = FileNode {
            name: "main.rs".to_string(),
            path: PathBuf::from("/root/main.rs"),
            node_type: NodeType::File { size: 100 },
            depth: 1,
            is_hidden: false,
        };

        let nodes: Vec<&FileNode> = vec![&file_node];
        let result = cursor_dir_path(&nodes, 0);
        assert_eq!(result, PathBuf::from("/root"));
    }

    #[test]
    fn cursor_dir_path_out_of_bounds() {
        let nodes: Vec<&crate::tree::node::FileNode> = vec![];
        let result = cursor_dir_path(&nodes, 5);
        assert_eq!(result, PathBuf::from("."));
    }
}
