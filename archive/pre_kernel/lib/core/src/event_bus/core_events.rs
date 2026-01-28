//! Core events that are available to all plugins
//!
//! These events represent fundamental editor operations that plugins commonly
//! need to react to. They use low priority numbers (0-50) to ensure core
//! handlers run before plugin handlers.

use {super::Event, crate::screen::window::SignColumnMode};

/// Priority constants for core events
pub mod priority {
    /// Highest priority - for critical core handlers
    pub const CRITICAL: u32 = 0;
    /// High priority - for core system handlers
    pub const CORE: u32 = 10;
    /// Normal priority - for standard core handlers
    pub const NORMAL: u32 = 50;
    /// Default priority for plugins
    pub const PLUGIN: u32 = 100;
    /// Low priority - for cleanup/finalization handlers
    pub const LOW: u32 = 200;
}

// =============================================================================
// Buffer Events
// =============================================================================

/// A buffer was created
#[derive(Debug, Clone)]
pub struct BufferCreated {
    /// ID of the created buffer
    pub buffer_id: usize,
}

impl Event for BufferCreated {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// A buffer was closed
#[derive(Debug, Clone)]
pub struct BufferClosed {
    /// ID of the closed buffer
    pub buffer_id: usize,
}

impl Event for BufferClosed {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// A buffer's content was modified
#[derive(Debug, Clone)]
pub struct BufferModified {
    /// ID of the modified buffer
    pub buffer_id: usize,
    /// Type of modification
    pub modification: BufferModification,
}

impl Event for BufferModified {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

/// Type of buffer modification
#[derive(Debug, Clone)]
pub enum BufferModification {
    /// Text was inserted
    Insert {
        /// Start position (line, column)
        start: (usize, usize),
        /// Inserted text
        text: String,
    },
    /// Text was deleted
    Delete {
        /// Start position (line, column)
        start: (usize, usize),
        /// End position (line, column)
        end: (usize, usize),
        /// Deleted text
        text: String,
    },
    /// Text was replaced
    Replace {
        /// Start position (line, column)
        start: (usize, usize),
        /// End position (line, column)
        end: (usize, usize),
        /// Old text that was replaced
        old_text: String,
        /// New text
        new_text: String,
    },
    /// Buffer content was fully replaced (e.g., file load)
    FullReplace,
}

/// Active buffer changed
#[derive(Debug, Clone)]
pub struct BufferSwitched {
    /// Previous buffer ID (if any)
    pub from: Option<usize>,
    /// New active buffer ID
    pub to: usize,
}

impl Event for BufferSwitched {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Buffer was saved to disk
#[derive(Debug, Clone)]
pub struct BufferSaved {
    /// ID of the saved buffer
    pub buffer_id: usize,
    /// Path the buffer was saved to
    pub path: String,
}

impl Event for BufferSaved {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

// =============================================================================
// Mode Events
// =============================================================================

/// Editor mode changed
#[derive(Debug, Clone)]
pub struct ModeChanged {
    /// Previous mode description
    pub from: String,
    /// New mode description
    pub to: String,
}

impl Event for ModeChanged {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

// =============================================================================
// Cursor Events
// =============================================================================

/// Cursor position changed
#[derive(Debug, Clone)]
pub struct CursorMoved {
    /// Buffer ID where cursor moved
    pub buffer_id: usize,
    /// Previous position (line, column)
    pub from: (usize, usize),
    /// New position (line, column)
    pub to: (usize, usize),
}

impl Event for CursorMoved {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

// =============================================================================
// Window Events
// =============================================================================

/// Window was created
#[derive(Debug, Clone)]
pub struct WindowCreated {
    /// ID of the created window
    pub window_id: usize,
}

impl Event for WindowCreated {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Window was closed
#[derive(Debug, Clone)]
pub struct WindowClosed {
    /// ID of the closed window
    pub window_id: usize,
}

impl Event for WindowClosed {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Active window changed
#[derive(Debug, Clone)]
pub struct WindowFocused {
    /// Previous window ID (if any)
    pub from: Option<usize>,
    /// New active window ID
    pub to: usize,
}

impl Event for WindowFocused {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Screen was resized
#[derive(Debug, Clone, Copy)]
pub struct ScreenResized {
    /// New width in columns
    pub width: u16,
    /// New height in rows
    pub height: u16,
}

impl Event for ScreenResized {
    fn priority(&self) -> u32 {
        priority::CRITICAL
    }
}

/// Viewport scrolled (window anchor changed)
///
/// Emitted when a window's visible viewport changes due to scrolling.
/// Plugins can subscribe to this event to update viewport-dependent UI
/// (e.g., sticky context headers).
#[derive(Debug, Clone, Copy)]
pub struct ViewportScrolled {
    /// ID of the window that scrolled
    pub window_id: usize,
    /// ID of the buffer being viewed
    pub buffer_id: usize,
    /// First visible line (0-indexed)
    pub top_line: u32,
    /// Last visible line (0-indexed)
    pub bottom_line: u32,
}

impl Event for ViewportScrolled {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

/// Request to change the focused component
///
/// Plugins emit this event to request focus change. The runtime subscribes
/// to this event and updates `ModeState.interactor_id` to route keybindings
/// to the newly focused component.
#[derive(Debug, Clone)]
pub struct RequestFocusChange {
    /// The component ID to focus
    pub target: crate::modd::ComponentId,
}

impl Event for RequestFocusChange {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

impl super::TargetedEvent for RequestFocusChange {
    fn target(&self) -> crate::modd::ComponentId {
        self.target
    }
}

/// Request to change the editor mode
///
/// Plugins emit this event to request a mode change (edit mode, sub-mode, etc.).
/// The runtime subscribes to this event and updates `ModeState` to change
/// keybinding routing and input handling.
#[derive(Debug, Clone)]
pub struct RequestModeChange {
    /// The new mode state to set
    pub mode: crate::modd::ModeState,
}

impl Event for RequestModeChange {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Request to open a file in the editor
///
/// Plugins emit this event to request opening a file. The runtime subscribes
/// to this event and creates a new buffer with the file content.
#[derive(Debug, Clone)]
pub struct RequestOpenFile {
    /// Path to the file to open
    pub path: std::path::PathBuf,
}

impl Event for RequestOpenFile {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Request to open a file at a specific position
///
/// Plugins emit this event to open a file and jump to a specific line/column.
/// Used by LSP goto definition, goto references, etc.
#[derive(Debug, Clone)]
pub struct RequestOpenFileAtPosition {
    /// Path to the file to open
    pub path: std::path::PathBuf,
    /// Line number (0-indexed)
    pub line: usize,
    /// Column number (0-indexed)
    pub column: usize,
}

impl Event for RequestOpenFileAtPosition {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Request to set a register's content
///
/// Plugins emit this event to set text into a register. The runtime subscribes
/// to this event and updates the register (including system clipboard for + register).
#[derive(Debug, Clone)]
pub struct RequestSetRegister {
    /// Register name: None for unnamed, '+' for system clipboard, 'a'-'z' for named
    pub register: Option<char>,
    /// Text content to set
    pub text: String,
}

impl Event for RequestSetRegister {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Request to insert text at the current cursor position
///
/// Plugins emit this event to programmatically insert text into the active buffer.
/// The runtime subscribes to this event and inserts the text as if the user typed it.
/// This is used for features like auto-pair insertion and completion.
#[derive(Debug, Clone)]
pub struct RequestInsertText {
    /// Text to insert
    pub text: String,
    /// If true, move cursor left after insertion (for auto-pair: insert `)` then move left)
    pub move_cursor_left: bool,
    /// Number of characters to delete backwards before inserting (for completion: replace prefix)
    pub delete_prefix_len: usize,
}

impl Event for RequestInsertText {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Request to move cursor to a specific position
///
/// Plugins emit this event to programmatically move the cursor in the active buffer.
/// The runtime subscribes to this event and updates the cursor position.
/// This is used for features like jump navigation, goto definition, etc.
///
/// # Optional Motion Context
///
/// When `motion_context` is provided, the cursor movement is treated as a motion
/// that can complete pending operators (d, y, c, etc.). The motion creates a range
/// from the current cursor position to the target position.
#[derive(Debug, Clone)]
pub struct RequestCursorMove {
    /// Target buffer ID
    pub buffer_id: usize,
    /// Target line (0-indexed)
    pub line: u32,
    /// Target column (0-indexed)
    pub column: u32,
    /// Optional motion context for operator completion
    ///
    /// When present, this cursor movement acts as a motion that completes
    /// any pending operator (d, y, c). The range is calculated from the
    /// current cursor position to the target position.
    pub motion_context: Option<MotionContext>,
}

/// Context for motion-based cursor movement
///
/// This allows cursor movements to integrate with vim-style operators.
/// When a cursor movement is performed with motion context, it can complete
/// pending operators like delete (d), yank (y), or change (c).
#[derive(Debug, Clone)]
pub struct MotionContext {
    /// Whether this is a linewise motion (affects whole lines)
    pub linewise: bool,
    /// Whether to include the character at the target position
    pub inclusive: bool,
    /// Optional operator to apply (Delete, Yank, Change)
    /// When set, overrides any operator from current mode state
    pub operator: Option<crate::modd::OperatorType>,
    /// Optional repeat count for the operator
    pub count: Option<usize>,
}

impl Event for RequestCursorMove {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

// =============================================================================
// Settings/Option Request Events
// =============================================================================

/// Request to set line number visibility
///
/// Emitted by plugins (e.g., settings menu) when the line number option changes.
/// The runtime subscribes to apply the change to the screen.
#[derive(Debug, Clone, Copy)]
pub struct RequestSetLineNumbers {
    /// Whether line numbers should be visible
    pub enabled: bool,
}

impl Event for RequestSetLineNumbers {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Request to set relative line number visibility
///
/// Emitted by plugins when the relative line number option changes.
#[derive(Debug, Clone, Copy)]
pub struct RequestSetRelativeLineNumbers {
    /// Whether relative line numbers should be visible
    pub enabled: bool,
}

impl Event for RequestSetRelativeLineNumbers {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Request to change the color theme
///
/// Emitted by plugins when the theme option changes.
#[derive(Debug, Clone)]
pub struct RequestSetTheme {
    /// Name of the theme to apply
    pub name: String,
}

impl Event for RequestSetTheme {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Request to set scrollbar visibility
///
/// Emitted by plugins when the scrollbar option changes.
#[derive(Debug, Clone, Copy)]
pub struct RequestSetScrollbar {
    /// Whether scrollbar should be visible
    pub enabled: bool,
}

impl Event for RequestSetScrollbar {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Request to set indent guide visibility
///
/// Emitted by plugins when the indent guide option changes.
#[derive(Debug, Clone, Copy)]
pub struct RequestSetIndentGuide {
    /// Whether indent guides should be visible
    pub enabled: bool,
}

impl Event for RequestSetIndentGuide {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Request to set sign column configuration
///
/// Emitted by plugins when the signcolumn option changes.
#[derive(Debug, Clone, Copy)]
pub struct RequestSetSignColumn {
    /// Sign column display mode
    pub mode: SignColumnMode,
}

impl Event for RequestSetSignColumn {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

// =============================================================================
// Text Input Events
// =============================================================================

/// Text input received by a plugin component
///
/// Emitted when a character is typed while a plugin component has focus.
/// Plugins subscribe to this event to handle text input (e.g., filter input,
/// file rename input, etc.).
#[derive(Debug, Clone)]
pub struct PluginTextInput {
    /// The component ID that received the input
    pub target: crate::modd::ComponentId,
    /// The character that was typed
    pub c: char,
}

impl Event for PluginTextInput {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

impl super::TargetedEvent for PluginTextInput {
    fn target(&self) -> crate::modd::ComponentId {
        self.target
    }
}

/// Backspace received by a plugin component
///
/// Emitted when backspace is pressed while a plugin component has focus.
/// Plugins subscribe to this event to handle text deletion.
#[derive(Debug, Clone)]
pub struct PluginBackspace {
    /// The component ID that received the input
    pub target: crate::modd::ComponentId,
}

impl Event for PluginBackspace {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

impl super::TargetedEvent for PluginBackspace {
    fn target(&self) -> crate::modd::ComponentId {
        self.target
    }
}

// =============================================================================
// Lifecycle Events
// =============================================================================

/// Request a render of the UI
#[derive(Debug, Clone, Copy)]
pub struct RenderRequest;

impl Event for RenderRequest {
    fn priority(&self) -> u32 {
        priority::LOW
    }
}

/// Request the editor to quit
#[derive(Debug, Clone)]
pub struct QuitRequest {
    /// Whether to force quit (ignore unsaved changes)
    pub force: bool,
}

impl Event for QuitRequest {
    fn priority(&self) -> u32 {
        priority::CRITICAL
    }
}

/// Editor is about to shut down
///
/// Plugins can use this to perform cleanup.
#[derive(Debug, Clone, Copy)]
pub struct ShutdownEvent;

impl Event for ShutdownEvent {
    fn priority(&self) -> u32 {
        priority::CRITICAL
    }
}

// =============================================================================
// File Events
// =============================================================================

/// A file was opened
#[derive(Debug, Clone)]
pub struct FileOpened {
    /// Buffer ID the file was loaded into
    pub buffer_id: usize,
    /// Path of the opened file
    pub path: String,
}

impl Event for FileOpened {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

/// File type was detected/changed
#[derive(Debug, Clone)]
pub struct FileTypeChanged {
    /// Buffer ID
    pub buffer_id: usize,
    /// Detected file type (e.g., "rust", "python", "markdown")
    pub file_type: String,
}

impl Event for FileTypeChanged {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

// =============================================================================
// Fold Events
// =============================================================================

use crate::folding::FoldRange;

/// Fold ranges were computed for a buffer (e.g., by treesitter)
#[derive(Debug, Clone)]
pub struct FoldRangesComputed {
    /// Buffer ID where fold ranges were computed
    pub buffer_id: usize,
    /// The computed fold ranges
    pub ranges: Vec<FoldRange>,
}

impl Event for FoldRangesComputed {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

// =============================================================================
// Ex-Command Events
// =============================================================================

/// Event emitted by plugins to register ex-command handlers
///
/// Plugins emit this event during the `subscribe()` phase to register
/// their custom ex-commands (e.g., `:settings`, `:profile`).
///
/// # Example
///
/// ```ignore
/// use reovim_core::{
///     command_line::ExCommandHandler,
///     event_bus::core_events::RegisterExCommand,
/// };
///
/// bus.emit(RegisterExCommand::new(
///     "settings",
///     ExCommandHandler::ZeroArg {
///         event_constructor: || DynEvent::new(SettingsMenuOpen),
///         description: "Open the settings menu",
///     },
/// ));
/// ```
#[derive(Clone)]
pub struct RegisterExCommand {
    /// Command name (e.g., "settings", "profile")
    pub name: std::borrow::Cow<'static, str>,

    /// Handler for the command
    pub handler: crate::command_line::ExCommandHandler,
}

impl RegisterExCommand {
    /// Create a new ex-command registration
    #[must_use]
    pub fn new(
        name: impl Into<std::borrow::Cow<'static, str>>,
        handler: crate::command_line::ExCommandHandler,
    ) -> Self {
        Self {
            name: name.into(),
            handler,
        }
    }
}

impl Event for RegisterExCommand {
    fn priority(&self) -> u32 {
        // High priority - run during plugin initialization
        priority::NORMAL
    }
}

impl std::fmt::Debug for RegisterExCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisterExCommand")
            .field("name", &self.name)
            .field("handler", &self.handler)
            .finish()
    }
}

// =============================================================================
// Profile Events
// =============================================================================

/// Event emitted to list available profiles
///
/// Plugins emit this event to open a profile picker/list UI.
/// The profiles plugin subscribes to this event and opens the picker.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProfileListEvent;

impl Event for ProfileListEvent {
    fn priority(&self) -> u32 {
        priority::PLUGIN
    }
}

/// Event emitted to load a profile by name
///
/// Plugins emit this event to trigger profile loading.
/// The runtime subscribes to this event and coordinates the actual load
/// across all registered Configurable components.
#[derive(Debug, Clone)]
pub struct ProfileLoadEvent {
    /// Name of the profile to load
    pub name: String,
}

impl Event for ProfileLoadEvent {
    fn priority(&self) -> u32 {
        priority::PLUGIN
    }
}

/// Event emitted to save current settings as a profile
///
/// Plugins emit this event to trigger profile saving.
/// The runtime subscribes to this event and coordinates the actual save
/// across all registered Configurable components.
#[derive(Debug, Clone)]
pub struct ProfileSaveEvent {
    /// Name of the profile to save
    pub name: String,
}

impl Event for ProfileSaveEvent {
    fn priority(&self) -> u32 {
        priority::PLUGIN
    }
}

// === Configurable Component Registration ===

/// Event emitted by plugins to register configurable components
///
/// Plugins emit this event during their `subscribe()` phase to register
/// components that participate in profile save/load. The runtime subscribes
/// to this event and registers the component with the `ProfileRegistry`.
///
/// # Example
///
/// ```ignore
/// use reovim_core::{
///     config::Configurable,
///     event_bus::core_events::RegisterConfigurable,
/// };
/// use std::sync::{Arc, RwLock};
///
/// struct MyConfig {
///     enabled: bool,
/// }
///
/// impl Configurable for MyConfig {
///     fn config_section(&self) -> &'static str { "my_plugin" }
///     fn to_config(&self) -> std::collections::HashMap<String, toml::Value> {
///         // ...
///         std::collections::HashMap::new()
///     }
///     fn from_config(&mut self, _data: &std::collections::HashMap<String, toml::Value>) {
///         // ...
///     }
/// }
///
/// // In plugin subscribe():
/// let config = Arc::new(RwLock::new(MyConfig { enabled: true }));
/// bus.emit(RegisterConfigurable::new(config));
/// ```
pub struct RegisterConfigurable {
    /// The configurable component to register
    pub component: std::sync::Arc<std::sync::RwLock<dyn crate::config::Configurable + Send + Sync>>,
}

impl RegisterConfigurable {
    /// Create a new registration event for a configurable component
    #[must_use]
    pub fn new(
        component: std::sync::Arc<std::sync::RwLock<dyn crate::config::Configurable + Send + Sync>>,
    ) -> Self {
        Self { component }
    }
}

impl std::fmt::Debug for RegisterConfigurable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let section = self
            .component
            .read()
            .map(|c| c.config_section())
            .unwrap_or("<locked>");

        f.debug_struct("RegisterConfigurable")
            .field("section", &section)
            .finish()
    }
}

impl Event for RegisterConfigurable {
    fn priority(&self) -> u32 {
        priority::CORE // High priority, register early
    }
}

// =============================================================================
// Command Line Completion Events
// =============================================================================

/// Emitted by runtime when command-line completion is triggered (Tab key)
///
/// The plugin subscribes to this event to generate completions based on
/// the current command line state.
#[derive(Debug, Clone)]
pub struct CmdlineCompletionTriggered {
    /// The completion context (command or argument)
    pub context: crate::command_line::CmdlineCompletionContext,
    /// Available registry commands (name, description) for command completion
    pub registry_commands: Vec<(String, String)>,
}

impl Event for CmdlineCompletionTriggered {
    fn priority(&self) -> u32 {
        priority::PLUGIN
    }
}

/// Emitted by runtime when previous completion is requested (Shift-Tab)
#[derive(Debug, Clone, Copy)]
pub struct CmdlineCompletionPrevRequested;

impl Event for CmdlineCompletionPrevRequested {
    fn priority(&self) -> u32 {
        priority::PLUGIN
    }
}

/// Emitted by plugin to request applying a completion to the command line
#[derive(Debug, Clone)]
pub struct RequestApplyCmdlineCompletion {
    /// Text to insert
    pub text: String,
    /// Position where to start replacing
    pub replace_start: usize,
}

impl Event for RequestApplyCmdlineCompletion {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}
