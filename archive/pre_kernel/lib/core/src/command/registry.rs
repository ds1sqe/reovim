//! Command registry for runtime command management

use {
    super::{id::CommandId, traits::CommandTrait},
    std::{
        collections::HashMap,
        fmt,
        sync::{Arc, RwLock},
    },
};

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
            // Buffer
            BufferDeleteCommand,
            BufferNextCommand,
            BufferPrevCommand,
            // Change
            ChangeLineCommand,
            // Command line
            CommandLineBackspaceCommand,
            CommandLineCancelCommand,
            CommandLineExecuteCommand,
            // Completion commands are now in reovim-plugin-completion crate
            // Cursor
            CursorDownCommand,
            CursorLeftCommand,
            CursorLineEndCommand,
            CursorLineStartCommand,
            CursorRightCommand,
            CursorUpCommand,
            CursorWordBackwardCommand,
            CursorWordEndCommand,
            CursorWordForwardCommand,
            // Text
            DeleteCharBackwardCommand,
            DeleteCharForwardCommand,
            DeleteLineCommand,
            // Operators
            EnterChangeOperatorCommand,
            // Mode
            EnterCommandModeCommand,
            EnterDeleteOperatorCommand,
            EnterInsertModeAfterCommand,
            EnterInsertModeCommand,
            EnterInsertModeEolCommand,
            EnterNormalModeCommand,
            EnterVisualBlockModeCommand,
            EnterVisualLineModeCommand,
            EnterVisualModeCommand,
            // Window Mode (Ctrl-W)
            EnterWindowModeCommand,
            EnterYankOperatorCommand,
            // Fold commands are now in reovim-plugin-fold crate
            GotoFirstLineCommand,
            GotoLastLineCommand,
            InsertNewlineCommand,
            // Jump
            JumpNewerCommand,
            JumpOlderCommand,
            // System
            NoopCommand,
            OpenLineAboveCommand,
            OpenLineBelowCommand,
            // Clipboard
            PasteBeforeCommand,
            PasteCommand,
            QuitCommand,
            // History (Undo/Redo)
            RedoCommand,
            // Settings Menu commands are registered by the settings-menu plugin
            // Tab
            TabCloseCommand,
            TabNewCommand,
            TabNextCommand,
            TabPrevCommand,
            UndoCommand,
            // Visual
            VisualDeleteCommand,
            VisualExtendDownCommand,
            VisualExtendLeftCommand,
            VisualExtendRightCommand,
            VisualExtendUpCommand,
            VisualYankCommand,
            // Window
            WindowCloseCommand,
            WindowEqualizeCommand,
            WindowFocusDownCommand,
            WindowFocusLeftCommand,
            WindowFocusRightCommand,
            WindowFocusUpCommand,
            WindowModeCloseCommand,
            WindowModeEqualizeCommand,
            WindowModeFocusDownCommand,
            WindowModeFocusLeftCommand,
            WindowModeFocusRightCommand,
            WindowModeFocusUpCommand,
            WindowModeMoveDownCommand,
            WindowModeMoveLeftCommand,
            WindowModeMoveRightCommand,
            WindowModeMoveUpCommand,
            WindowModeOnlyCommand,
            WindowModeSplitHCommand,
            WindowModeSplitVCommand,
            WindowModeSwapDownCommand,
            WindowModeSwapLeftCommand,
            WindowModeSwapRightCommand,
            WindowModeSwapUpCommand,
            WindowMoveDownCommand,
            WindowMoveLeftCommand,
            WindowMoveRightCommand,
            WindowMoveUpCommand,
            WindowOnlyCommand,
            WindowSplitHorizontalCommand,
            WindowSplitVerticalCommand,
            YankLineCommand,
            YankToEndCommand,
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
        let _ = self.register(CursorWordEndCommand);
        let _ = self.register(GotoFirstLineCommand);
        let _ = self.register(GotoLastLineCommand);

        // Jump list commands
        let _ = self.register(JumpOlderCommand);
        let _ = self.register(JumpNewerCommand);

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
        let _ = self.register(EnterVisualLineModeCommand);
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
        let _ = self.register(ChangeLineCommand);

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

        // Completion commands are registered by the completion plugin (reovim-plugin-completion)

        // Explorer commands are registered by the explorer plugin (reovim-plugin-explorer)
        // Telescope commands are registered by the telescope plugin (reovim-plugin-telescope)

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

        // Window mode commands (Ctrl-W)
        let _ = self.register(EnterWindowModeCommand);
        let _ = self.register(WindowModeFocusLeftCommand);
        let _ = self.register(WindowModeFocusDownCommand);
        let _ = self.register(WindowModeFocusUpCommand);
        let _ = self.register(WindowModeFocusRightCommand);
        let _ = self.register(WindowModeMoveLeftCommand);
        let _ = self.register(WindowModeMoveDownCommand);
        let _ = self.register(WindowModeMoveUpCommand);
        let _ = self.register(WindowModeMoveRightCommand);
        let _ = self.register(WindowModeSwapLeftCommand);
        let _ = self.register(WindowModeSwapDownCommand);
        let _ = self.register(WindowModeSwapUpCommand);
        let _ = self.register(WindowModeSwapRightCommand);
        let _ = self.register(WindowModeSplitHCommand);
        let _ = self.register(WindowModeSplitVCommand);
        let _ = self.register(WindowModeCloseCommand);
        let _ = self.register(WindowModeOnlyCommand);
        let _ = self.register(WindowModeEqualizeCommand);

        // Tab commands
        let _ = self.register(TabNewCommand);
        let _ = self.register(TabCloseCommand);
        let _ = self.register(TabNextCommand);
        let _ = self.register(TabPrevCommand);

        // Buffer commands
        let _ = self.register(BufferPrevCommand);
        let _ = self.register(BufferNextCommand);
        let _ = self.register(BufferDeleteCommand);

        // Settings menu commands are registered by the settings-menu plugin
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
            Self::AlreadyRegistered(id) => {
                write!(f, "Command already registered: {id}")
            }
            Self::NotFound(id) => write!(f, "Command not found: {id}"),
            Self::LockPoisoned => write!(f, "Registry lock poisoned"),
        }
    }
}

impl std::error::Error for RegistryError {}
