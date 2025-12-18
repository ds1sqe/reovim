//! Command registry for runtime command management

use super::id::CommandId;
use super::traits::CommandTrait;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock};

/// Thread-safe command registry
///
/// The registry stores all registered commands and provides
/// lookup by command ID. It supports runtime registration
/// of new commands for plugin/extension support.
pub struct CommandRegistry {
    commands: RwLock<HashMap<CommandId, Arc<dyn CommandTrait>>>,
}

impl CommandRegistry {
    /// Create a new empty command registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: RwLock::new(HashMap::new()),
        }
    }

    /// Create a registry with all built-in commands registered
    #[must_use]
    pub fn with_defaults() -> Self {
        let registry = Self::new();
        registry.register_builtins();
        registry
    }

    /// Register a command
    ///
    /// # Errors
    /// Returns an error if a command with the same ID is already registered.
    pub fn register<C: CommandTrait + 'static>(&self, cmd: C) -> Result<(), RegistryError> {
        let id = CommandId::new(cmd.name());
        let mut commands = self
            .commands
            .write()
            .map_err(|_| RegistryError::LockPoisoned)?;

        if commands.contains_key(&id) {
            return Err(RegistryError::AlreadyRegistered(id));
        }

        commands.insert(id, Arc::new(cmd));
        drop(commands);
        Ok(())
    }

    /// Register a command, replacing any existing command with the same ID
    pub fn register_or_replace<C: CommandTrait + 'static>(&self, cmd: C) {
        let id = CommandId::new(cmd.name());
        if let Ok(mut commands) = self.commands.write() {
            commands.insert(id, Arc::new(cmd));
        }
    }

    /// Unregister a command by ID
    ///
    /// Returns the removed command if it existed.
    pub fn unregister(&self, id: &CommandId) -> Option<Arc<dyn CommandTrait>> {
        self.commands.write().ok()?.remove(id)
    }

    /// Get a command by ID
    #[must_use]
    pub fn get(&self, id: &CommandId) -> Option<Arc<dyn CommandTrait>> {
        self.commands.read().ok()?.get(id).cloned()
    }

    /// Check if a command is registered
    #[must_use]
    pub fn contains(&self, id: &CommandId) -> bool {
        self.commands
            .read()
            .map(|c| c.contains_key(id))
            .unwrap_or(false)
    }

    /// List all registered command IDs
    #[must_use]
    pub fn list(&self) -> Vec<CommandId> {
        self.commands
            .read()
            .map(|c| c.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the number of registered commands
    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.read().map(|c| c.len()).unwrap_or(0)
    }

    /// Check if the registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Register all built-in commands
    ///
    /// This is called automatically when using `with_defaults()`.
    #[allow(clippy::too_many_lines)]
    fn register_builtins(&self) {
        use super::builtin::{
            // Clipboard
            PasteBeforeCommand, PasteCommand,
            // Command line
            CommandLineBackspaceCommand, CommandLineCancelCommand, CommandLineExecuteCommand,
            // Completion
            CompletionConfirmCommand, CompletionDismissCommand, CompletionNextCommand,
            CompletionPrevCommand, CompletionTriggerCommand,
            // Cursor
            CursorDownCommand, CursorLeftCommand, CursorLineEndCommand, CursorLineStartCommand,
            CursorRightCommand, CursorUpCommand, CursorWordBackwardCommand,
            CursorWordForwardCommand, GotoFirstLineCommand, GotoLastLineCommand,
            // Explorer
            ExplorerCancelInputCommand, ExplorerClearFilterCommand, ExplorerCloseCommand,
            ExplorerCloseParentCommand, ExplorerConfirmInputCommand, ExplorerCreateDirCommand,
            ExplorerCreateFileCommand, ExplorerCursorDownCommand, ExplorerCursorUpCommand,
            ExplorerDeleteCommand, ExplorerFilterCommand, ExplorerFocusEditorCommand,
            ExplorerGoToParentCommand, ExplorerGotoFirstCommand, ExplorerGotoLastCommand,
            ExplorerInputBackspaceCommand, ExplorerOpenNodeCommand, ExplorerPageDownCommand,
            ExplorerPageUpCommand, ExplorerRefreshCommand, ExplorerRenameCommand,
            ExplorerToggleHiddenCommand, ExplorerToggleNodeCommand,
            // Fold
            FoldCloseAllCommand, FoldCloseCommand, FoldOpenAllCommand, FoldOpenCommand,
            FoldToggleCommand,
            // History (Undo/Redo)
            RedoCommand, UndoCommand,
            // Jump
            JumpNewerCommand, JumpOlderCommand,
            // Leap
            LeapBackwardCommand, LeapCancelCommand, LeapForwardCommand,
            // Mode
            EnterCommandModeCommand, EnterInsertModeAfterCommand, EnterInsertModeCommand,
            EnterInsertModeEolCommand, EnterNormalModeCommand, EnterVisualBlockModeCommand,
            EnterVisualModeCommand, OpenLineAboveCommand, OpenLineBelowCommand,
            // Operators
            EnterChangeOperatorCommand, EnterDeleteOperatorCommand, EnterYankOperatorCommand,
            // System
            NoopCommand, QuitCommand,
            // Telescope
            TelescopeBackspaceCommand, TelescopeCloseCommand, TelescopeCommandsCommand,
            TelescopeConfirmCommand, TelescopeEnterInsertCommand, TelescopeEnterNormalCommand,
            TelescopeFindBuffersCommand, TelescopeFindFilesCommand, TelescopeGotoFirstCommand,
            TelescopeGotoLastCommand, TelescopeHelpTagsCommand, TelescopeKeymapsCommand,
            TelescopeLiveGrepCommand, TelescopePageDownCommand, TelescopePageUpCommand,
            TelescopeRecentFilesCommand, TelescopeSelectNextCommand, TelescopeSelectPrevCommand,
            TelescopeThemesCommand,
            // Text
            DeleteCharBackwardCommand, DeleteCharForwardCommand, DeleteLineCommand,
            InsertNewlineCommand, YankLineCommand, YankToEndCommand,
            // Visual
            VisualDeleteCommand, VisualExtendDownCommand, VisualExtendLeftCommand,
            VisualExtendRightCommand, VisualExtendUpCommand, VisualYankCommand,
            // Window
            WindowCloseCommand, WindowEqualizeCommand, WindowFocusDownCommand,
            WindowFocusLeftCommand, WindowFocusRightCommand, WindowFocusUpCommand,
            WindowMoveDownCommand, WindowMoveLeftCommand, WindowMoveRightCommand,
            WindowMoveUpCommand, WindowOnlyCommand, WindowSplitHorizontalCommand,
            WindowSplitVerticalCommand,
            // Tab
            TabCloseCommand, TabNewCommand, TabNextCommand, TabPrevCommand,
            // Settings Menu
            SettingsMenuCloseCommand, SettingsMenuCycleNextCommand, SettingsMenuCyclePrevCommand,
            SettingsMenuDecrementCommand, SettingsMenuExecuteCommand, SettingsMenuIncrementCommand,
            SettingsMenuNextCommand, SettingsMenuOpenCommand, SettingsMenuPrevCommand,
            SettingsMenuQuick1Command, SettingsMenuQuick2Command, SettingsMenuQuick3Command,
            SettingsMenuQuick4Command, SettingsMenuQuick5Command, SettingsMenuQuick6Command,
            SettingsMenuQuick7Command, SettingsMenuQuick8Command, SettingsMenuQuick9Command,
            SettingsMenuToggleCommand,
        };

        // Cursor movement commands
        let _ = self.register(CursorUpCommand);
        let _ = self.register(CursorDownCommand);
        let _ = self.register(CursorLeftCommand);
        let _ = self.register(CursorRightCommand);
        let _ = self.register(CursorLineStartCommand);
        let _ = self.register(CursorLineEndCommand);
        let _ = self.register(CursorWordForwardCommand);
        let _ = self.register(CursorWordBackwardCommand);
        let _ = self.register(GotoFirstLineCommand);
        let _ = self.register(GotoLastLineCommand);

        // Jump list commands
        let _ = self.register(JumpOlderCommand);
        let _ = self.register(JumpNewerCommand);

        // Leap motion commands
        let _ = self.register(LeapForwardCommand);
        let _ = self.register(LeapBackwardCommand);
        let _ = self.register(LeapCancelCommand);

        // History (Undo/Redo) commands
        let _ = self.register(UndoCommand);
        let _ = self.register(RedoCommand);

        // Mode switching commands
        let _ = self.register(EnterNormalModeCommand);
        let _ = self.register(EnterInsertModeCommand);
        let _ = self.register(EnterInsertModeAfterCommand);
        let _ = self.register(EnterInsertModeEolCommand);
        let _ = self.register(OpenLineBelowCommand);
        let _ = self.register(OpenLineAboveCommand);
        let _ = self.register(EnterVisualModeCommand);
        let _ = self.register(EnterVisualBlockModeCommand);
        let _ = self.register(EnterCommandModeCommand);

        // Operator commands
        let _ = self.register(EnterDeleteOperatorCommand);
        let _ = self.register(EnterYankOperatorCommand);
        let _ = self.register(EnterChangeOperatorCommand);

        // Text editing commands
        let _ = self.register(DeleteCharBackwardCommand);
        let _ = self.register(DeleteCharForwardCommand);
        let _ = self.register(DeleteLineCommand);
        let _ = self.register(InsertNewlineCommand);
        let _ = self.register(YankLineCommand);
        let _ = self.register(YankToEndCommand);

        // Visual mode commands
        let _ = self.register(VisualExtendUpCommand);
        let _ = self.register(VisualExtendDownCommand);
        let _ = self.register(VisualExtendLeftCommand);
        let _ = self.register(VisualExtendRightCommand);
        let _ = self.register(VisualDeleteCommand);
        let _ = self.register(VisualYankCommand);

        // Command line commands
        let _ = self.register(CommandLineBackspaceCommand);
        let _ = self.register(CommandLineExecuteCommand);
        let _ = self.register(CommandLineCancelCommand);

        // Clipboard commands
        let _ = self.register(PasteCommand);
        let _ = self.register(PasteBeforeCommand);

        // System commands
        let _ = self.register(QuitCommand);
        let _ = self.register(NoopCommand);

        // Completion commands
        let _ = self.register(CompletionTriggerCommand);
        let _ = self.register(CompletionNextCommand);
        let _ = self.register(CompletionPrevCommand);
        let _ = self.register(CompletionConfirmCommand);
        let _ = self.register(CompletionDismissCommand);

        // Explorer commands
        let _ = self.register(ExplorerCursorUpCommand);
        let _ = self.register(ExplorerCursorDownCommand);
        let _ = self.register(ExplorerPageUpCommand);
        let _ = self.register(ExplorerPageDownCommand);
        let _ = self.register(ExplorerGotoFirstCommand);
        let _ = self.register(ExplorerGotoLastCommand);
        let _ = self.register(ExplorerToggleNodeCommand);
        let _ = self.register(ExplorerOpenNodeCommand);
        let _ = self.register(ExplorerCloseParentCommand);
        let _ = self.register(ExplorerGoToParentCommand);
        let _ = self.register(ExplorerRefreshCommand);
        let _ = self.register(ExplorerToggleHiddenCommand);
        let _ = self.register(ExplorerCloseCommand);
        let _ = self.register(ExplorerFocusEditorCommand);
        let _ = self.register(ExplorerCreateFileCommand);
        let _ = self.register(ExplorerCreateDirCommand);
        let _ = self.register(ExplorerRenameCommand);
        let _ = self.register(ExplorerDeleteCommand);
        let _ = self.register(ExplorerFilterCommand);
        let _ = self.register(ExplorerClearFilterCommand);
        let _ = self.register(ExplorerConfirmInputCommand);
        let _ = self.register(ExplorerCancelInputCommand);
        let _ = self.register(ExplorerInputBackspaceCommand);

        // Telescope commands
        let _ = self.register(TelescopeFindFilesCommand);
        let _ = self.register(TelescopeFindBuffersCommand);
        let _ = self.register(TelescopeLiveGrepCommand);
        let _ = self.register(TelescopeRecentFilesCommand);
        let _ = self.register(TelescopeCommandsCommand);
        let _ = self.register(TelescopeHelpTagsCommand);
        let _ = self.register(TelescopeKeymapsCommand);
        let _ = self.register(TelescopeThemesCommand);
        let _ = self.register(TelescopeSelectNextCommand);
        let _ = self.register(TelescopeSelectPrevCommand);
        let _ = self.register(TelescopePageDownCommand);
        let _ = self.register(TelescopePageUpCommand);
        let _ = self.register(TelescopeGotoFirstCommand);
        let _ = self.register(TelescopeGotoLastCommand);
        let _ = self.register(TelescopeConfirmCommand);
        let _ = self.register(TelescopeCloseCommand);
        let _ = self.register(TelescopeBackspaceCommand);
        let _ = self.register(TelescopeEnterInsertCommand);
        let _ = self.register(TelescopeEnterNormalCommand);

        // Fold commands
        let _ = self.register(FoldToggleCommand);
        let _ = self.register(FoldOpenCommand);
        let _ = self.register(FoldCloseCommand);
        let _ = self.register(FoldOpenAllCommand);
        let _ = self.register(FoldCloseAllCommand);

        // Window commands
        let _ = self.register(WindowFocusLeftCommand);
        let _ = self.register(WindowFocusDownCommand);
        let _ = self.register(WindowFocusUpCommand);
        let _ = self.register(WindowFocusRightCommand);
        let _ = self.register(WindowMoveLeftCommand);
        let _ = self.register(WindowMoveDownCommand);
        let _ = self.register(WindowMoveUpCommand);
        let _ = self.register(WindowMoveRightCommand);
        let _ = self.register(WindowSplitHorizontalCommand);
        let _ = self.register(WindowSplitVerticalCommand);
        let _ = self.register(WindowCloseCommand);
        let _ = self.register(WindowOnlyCommand);
        let _ = self.register(WindowEqualizeCommand);

        // Tab commands
        let _ = self.register(TabNewCommand);
        let _ = self.register(TabCloseCommand);
        let _ = self.register(TabNextCommand);
        let _ = self.register(TabPrevCommand);

        // Settings menu commands
        let _ = self.register(SettingsMenuOpenCommand);
        let _ = self.register(SettingsMenuCloseCommand);
        let _ = self.register(SettingsMenuNextCommand);
        let _ = self.register(SettingsMenuPrevCommand);
        let _ = self.register(SettingsMenuToggleCommand);
        let _ = self.register(SettingsMenuCycleNextCommand);
        let _ = self.register(SettingsMenuCyclePrevCommand);
        let _ = self.register(SettingsMenuIncrementCommand);
        let _ = self.register(SettingsMenuDecrementCommand);
        let _ = self.register(SettingsMenuExecuteCommand);
        let _ = self.register(SettingsMenuQuick1Command);
        let _ = self.register(SettingsMenuQuick2Command);
        let _ = self.register(SettingsMenuQuick3Command);
        let _ = self.register(SettingsMenuQuick4Command);
        let _ = self.register(SettingsMenuQuick5Command);
        let _ = self.register(SettingsMenuQuick6Command);
        let _ = self.register(SettingsMenuQuick7Command);
        let _ = self.register(SettingsMenuQuick8Command);
        let _ = self.register(SettingsMenuQuick9Command);
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CommandRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.len();
        f.debug_struct("CommandRegistry")
            .field("command_count", &count)
            .finish()
    }
}

/// Errors that can occur during registry operations
#[derive(Debug)]
pub enum RegistryError {
    /// A command with this ID is already registered
    AlreadyRegistered(CommandId),
    /// The requested command was not found
    NotFound(CommandId),
    /// The internal lock was poisoned
    LockPoisoned,
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRegistered(id) => write!(f, "Command already registered: {id}"),
            Self::NotFound(id) => write!(f, "Command not found: {id}"),
            Self::LockPoisoned => write!(f, "Registry lock poisoned"),
        }
    }
}

impl std::error::Error for RegistryError {}
