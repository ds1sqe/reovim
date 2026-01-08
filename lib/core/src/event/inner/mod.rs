use std::path::PathBuf;

use {reovim_sys::event::KeyModifiers, tokio::sync::oneshot};

use crate::{
    bind::CommandRef,
    command::{CommandContext, traits::OperatorMotionAction},
    event_bus::{DynEvent, EventScope},
    highlight::{Highlight, HighlightGroup},
    modd::{ComponentId, ModeState},
    plugin::PluginId,
    rpc::RpcResponse,
    screen::{NavigateDirection, SplitDirection, window::SignColumnMode},
    syntax::SyntaxProvider,
    textobject::{SemanticTextObjectSpec, TextObject, WordTextObject},
};

// =============================================================================
// RuntimeEvent - Wrapper with scope tracking (like DynEvent)
// =============================================================================

/// Runtime event with scope tracking for deterministic synchronization.
///
/// This wrapper carries an optional `EventScope` that tracks the lifecycle
/// of related events, enabling callers to wait until all events in a scope
/// (and their children) complete.
pub struct RuntimeEvent {
    payload: RuntimeEventPayload,
    scope: Option<EventScope>,
}

impl RuntimeEvent {
    /// Create a new runtime event without scope tracking.
    #[must_use]
    pub const fn new(payload: RuntimeEventPayload) -> Self {
        Self {
            payload,
            scope: None,
        }
    }

    /// Attach a scope to this event for lifecycle tracking.
    #[must_use]
    pub fn with_scope(mut self, scope: EventScope) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Get a reference to the scope, if any.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // as_ref() is not const-stable
    pub fn scope(&self) -> Option<&EventScope> {
        self.scope.as_ref()
    }

    /// Take ownership of the scope, leaving None in its place.
    #[allow(clippy::missing_const_for_fn)] // Option::take() is not const-stable
    pub fn take_scope(&mut self) -> Option<EventScope> {
        self.scope.take()
    }

    /// Get a reference to the payload.
    #[must_use]
    pub const fn payload(&self) -> &RuntimeEventPayload {
        &self.payload
    }

    /// Consume self and return the payload.
    #[must_use]
    pub fn into_payload(self) -> RuntimeEventPayload {
        self.payload
    }
}

// Ergonomic conversion from any type that can become RuntimeEventPayload
impl<T: Into<RuntimeEventPayload>> From<T> for RuntimeEvent {
    fn from(payload: T) -> Self {
        Self::new(payload.into())
    }
}

// =============================================================================
// RuntimeEventPayload - The actual event data (renamed from InnerEvent)
// =============================================================================

/// The actual event data, grouped into logical categories.
pub enum RuntimeEventPayload {
    // Buffer operations
    Buffer(BufferEvent),

    // Window management
    Window(WindowEvent),

    // Command execution
    Command(CommandEvent),

    // Editing operations
    Editing(EditingEvent),

    // Mode changes
    Mode(ModeEvent),

    // Visual updates
    Render(RenderEvent),

    // Settings/Options
    Settings(SettingsEvent),

    // External input
    Input(InputEvent),

    // File operations
    File(FileEvent),

    // RPC request
    Rpc(RpcEvent),

    // Plugin event
    Plugin(PluginEventData),

    // Lifecycle - terminate the runtime
    Kill,
}

// =============================================================================
// Sub-enum: Editing operations
// =============================================================================

/// Editing-related events
pub enum EditingEvent {
    /// Text input event (insert char, backspace, etc.)
    TextInput(TextInputEvent),
    /// Operator + motion action (d+motion, y+motion, c+motion)
    OperatorMotion(OperatorMotionAction),
    /// Visual mode text object selection (viw, vi(, vif, etc.)
    VisualTextObject(VisualTextObjectAction),
    /// Move cursor to specific position (from plugins like range-finder, LSP)
    MoveCursor {
        buffer_id: usize,
        line: u32,
        column: u32,
    },
    /// Set a register's content
    SetRegister {
        /// Register name: None for unnamed, '+' for system clipboard, 'a'-'z' for named
        register: Option<char>,
        /// Text content to set
        text: String,
    },
}

// =============================================================================
// Sub-enum: Mode changes
// =============================================================================

/// Mode-related events
pub enum ModeEvent {
    /// Mode state change
    Change(ModeState),
    /// Pending keys display update
    PendingKeys(String),
}

// =============================================================================
// Sub-enum: Render updates
// =============================================================================

/// Render-related events
pub enum RenderEvent {
    /// Request a render cycle
    Signal,
    /// Highlight update
    Highlight(HighlightEvent),
    /// Syntax provider update
    Syntax(SyntaxEvent),
}

// =============================================================================
// Sub-enum: Settings
// =============================================================================

/// Settings/option events
pub enum SettingsEvent {
    /// Set line number visibility
    LineNumbers { enabled: bool },
    /// Set relative line number visibility
    RelativeLineNumbers { enabled: bool },
    /// Set color theme
    Theme { name: String },
    /// Set scrollbar visibility
    Scrollbar { enabled: bool },
    /// Set indent guide visibility
    IndentGuide { enabled: bool },
    /// Set sign column configuration
    SignColumn { mode: SignColumnMode },
    /// Apply completion text to command line
    ApplyCmdlineCompletion { text: String, replace_start: usize },
}

// =============================================================================
// Sub-enum: Input
// =============================================================================

/// External input events
pub enum InputEvent {
    /// Mouse event from terminal
    Mouse(MouseEvent),
    /// Terminal screen resize event
    ScreenResize { width: u16, height: u16 },
}

// =============================================================================
// Sub-enum: File operations
// =============================================================================

/// File operation events
pub enum FileEvent {
    /// Request to open a file (from explorer, commands, etc.)
    Open { path: PathBuf },
    /// Request to open a file at a specific position (for LSP navigation)
    OpenAt {
        path: PathBuf,
        /// Line number (0-indexed)
        line: usize,
        /// Column number (0-indexed)
        column: usize,
    },
}

// =============================================================================
// Sub-struct: RPC event
// =============================================================================

/// RPC request from server mode
pub struct RpcEvent {
    /// Request ID
    pub id: u64,
    /// Method name
    pub method: String,
    /// Method parameters
    pub params: serde_json::Value,
    /// Channel to send the response
    pub response_tx: oneshot::Sender<RpcResponse>,
}

// =============================================================================
// Sub-struct: Plugin event
// =============================================================================

/// Plugin-defined event (type-erased)
pub struct PluginEventData {
    /// Plugin that sent this event (for routing)
    pub plugin_id: PluginId,
    /// Type-erased event payload
    pub event: DynEvent,
}

// =============================================================================
// From implementations for sub-enums -> RuntimeEventPayload
// =============================================================================

impl From<BufferEvent> for RuntimeEventPayload {
    fn from(e: BufferEvent) -> Self {
        Self::Buffer(e)
    }
}

impl From<WindowEvent> for RuntimeEventPayload {
    fn from(e: WindowEvent) -> Self {
        Self::Window(e)
    }
}

impl From<CommandEvent> for RuntimeEventPayload {
    fn from(e: CommandEvent) -> Self {
        Self::Command(e)
    }
}

impl From<EditingEvent> for RuntimeEventPayload {
    fn from(e: EditingEvent) -> Self {
        Self::Editing(e)
    }
}

impl From<ModeEvent> for RuntimeEventPayload {
    fn from(e: ModeEvent) -> Self {
        Self::Mode(e)
    }
}

impl From<RenderEvent> for RuntimeEventPayload {
    fn from(e: RenderEvent) -> Self {
        Self::Render(e)
    }
}

impl From<SettingsEvent> for RuntimeEventPayload {
    fn from(e: SettingsEvent) -> Self {
        Self::Settings(e)
    }
}

impl From<InputEvent> for RuntimeEventPayload {
    fn from(e: InputEvent) -> Self {
        Self::Input(e)
    }
}

impl From<FileEvent> for RuntimeEventPayload {
    fn from(e: FileEvent) -> Self {
        Self::File(e)
    }
}

impl From<RpcEvent> for RuntimeEventPayload {
    fn from(e: RpcEvent) -> Self {
        Self::Rpc(e)
    }
}

impl From<PluginEventData> for RuntimeEventPayload {
    fn from(e: PluginEventData) -> Self {
        Self::Plugin(e)
    }
}

// =============================================================================
// Convenience constructors on RuntimeEvent
// =============================================================================

impl RuntimeEvent {
    /// Create a render signal event.
    #[must_use]
    pub fn render_signal() -> Self {
        RuntimeEventPayload::Render(RenderEvent::Signal).into()
    }

    /// Create a kill signal event.
    #[must_use]
    pub fn kill() -> Self {
        RuntimeEventPayload::Kill.into()
    }

    /// Create a mode change event.
    #[must_use]
    pub fn mode_change(mode: ModeState) -> Self {
        RuntimeEventPayload::Mode(ModeEvent::Change(mode)).into()
    }

    /// Create a pending keys event.
    #[must_use]
    pub fn pending_keys(keys: String) -> Self {
        RuntimeEventPayload::Mode(ModeEvent::PendingKeys(keys)).into()
    }

    /// Create a buffer event.
    #[must_use]
    pub fn buffer(event: BufferEvent) -> Self {
        RuntimeEventPayload::Buffer(event).into()
    }

    /// Create a window event.
    #[must_use]
    pub fn window(event: WindowEvent) -> Self {
        RuntimeEventPayload::Window(event).into()
    }

    /// Create a command event.
    #[must_use]
    pub fn command(event: CommandEvent) -> Self {
        RuntimeEventPayload::Command(event).into()
    }

    /// Create a highlight event.
    #[must_use]
    pub fn highlight(event: HighlightEvent) -> Self {
        RuntimeEventPayload::Render(RenderEvent::Highlight(event)).into()
    }

    /// Create a syntax event.
    #[must_use]
    pub fn syntax(event: SyntaxEvent) -> Self {
        RuntimeEventPayload::Render(RenderEvent::Syntax(event)).into()
    }

    /// Create a mouse event.
    #[must_use]
    pub fn mouse(event: MouseEvent) -> Self {
        RuntimeEventPayload::Input(InputEvent::Mouse(event)).into()
    }

    /// Create a screen resize event.
    #[must_use]
    pub fn screen_resize(width: u16, height: u16) -> Self {
        RuntimeEventPayload::Input(InputEvent::ScreenResize { width, height }).into()
    }

    /// Create an operator motion event.
    #[must_use]
    pub fn operator_motion(action: OperatorMotionAction) -> Self {
        RuntimeEventPayload::Editing(EditingEvent::OperatorMotion(action)).into()
    }

    /// Create a visual text object event.
    #[must_use]
    pub fn visual_text_object(action: VisualTextObjectAction) -> Self {
        RuntimeEventPayload::Editing(EditingEvent::VisualTextObject(action)).into()
    }

    /// Create a text input event.
    #[must_use]
    pub fn text_input(event: TextInputEvent) -> Self {
        RuntimeEventPayload::Editing(EditingEvent::TextInput(event)).into()
    }

    /// Create an open file request.
    #[must_use]
    pub fn open_file(path: PathBuf) -> Self {
        RuntimeEventPayload::File(FileEvent::Open { path }).into()
    }

    /// Create an open file at position request.
    #[must_use]
    pub fn open_file_at(path: PathBuf, line: usize, column: usize) -> Self {
        RuntimeEventPayload::File(FileEvent::OpenAt { path, line, column }).into()
    }

    /// Create a set register event.
    #[must_use]
    pub fn set_register(register: Option<char>, text: String) -> Self {
        RuntimeEventPayload::Editing(EditingEvent::SetRegister { register, text }).into()
    }

    /// Create a move cursor event.
    #[must_use]
    pub fn move_cursor(buffer_id: usize, line: u32, column: u32) -> Self {
        RuntimeEventPayload::Editing(EditingEvent::MoveCursor {
            buffer_id,
            line,
            column,
        })
        .into()
    }

    /// Create a plugin event.
    #[must_use]
    pub fn plugin(plugin_id: PluginId, event: DynEvent) -> Self {
        RuntimeEventPayload::Plugin(PluginEventData { plugin_id, event }).into()
    }

    /// Create an RPC request event.
    #[must_use]
    pub fn rpc(
        id: u64,
        method: String,
        params: serde_json::Value,
        response_tx: oneshot::Sender<RpcResponse>,
    ) -> Self {
        RuntimeEventPayload::Rpc(RpcEvent {
            id,
            method,
            params,
            response_tx,
        })
        .into()
    }

    // === Settings convenience constructors ===

    /// Create a set line numbers event.
    #[must_use]
    pub fn set_line_numbers(enabled: bool) -> Self {
        RuntimeEventPayload::Settings(SettingsEvent::LineNumbers { enabled }).into()
    }

    /// Create a set relative line numbers event.
    #[must_use]
    pub fn set_relative_line_numbers(enabled: bool) -> Self {
        RuntimeEventPayload::Settings(SettingsEvent::RelativeLineNumbers { enabled }).into()
    }

    /// Create a set theme event.
    #[must_use]
    pub fn set_theme(name: String) -> Self {
        RuntimeEventPayload::Settings(SettingsEvent::Theme { name }).into()
    }

    /// Create a set scrollbar event.
    #[must_use]
    pub fn set_scrollbar(enabled: bool) -> Self {
        RuntimeEventPayload::Settings(SettingsEvent::Scrollbar { enabled }).into()
    }

    /// Create a set indent guide event.
    #[must_use]
    pub fn set_indent_guide(enabled: bool) -> Self {
        RuntimeEventPayload::Settings(SettingsEvent::IndentGuide { enabled }).into()
    }

    /// Create a set sign column event.
    #[must_use]
    pub fn set_sign_column(mode: SignColumnMode) -> Self {
        RuntimeEventPayload::Settings(SettingsEvent::SignColumn { mode }).into()
    }

    /// Create an apply command line completion event.
    #[must_use]
    pub fn apply_cmdline_completion(text: String, replace_start: usize) -> Self {
        RuntimeEventPayload::Settings(SettingsEvent::ApplyCmdlineCompletion {
            text,
            replace_start,
        })
        .into()
    }
}

// =============================================================================
// Existing types (unchanged)
// =============================================================================

/// Input events routed to the active focus target
#[derive(Debug, Clone, Copy)]
pub enum TextInputEvent {
    /// Insert a character at the cursor position
    InsertChar(char),
    /// Delete the character before the cursor
    DeleteCharBackward,
}

/// Buffer-related events
pub enum BufferEvent {
    /// Set buffer content directly
    SetContent { buffer_id: usize, content: String },
    /// Load a file into a buffer
    LoadFile { buffer_id: usize, path: PathBuf },
    /// Create a new empty buffer
    Create { buffer_id: usize },
    /// Close a buffer
    Close { buffer_id: usize },
    /// Switch to a different buffer
    Switch { buffer_id: usize },
}

/// Window-related events
pub enum WindowEvent {
    FocusPlugin {
        id: ComponentId,
    },
    /// Return focus to the editor (unfocus any plugin)
    FocusEditor,
    // Splits
    SplitHorizontal {
        filename: Option<String>,
    },
    SplitVertical {
        filename: Option<String>,
    },
    Close {
        force: bool,
    },
    CloseOthers,

    // Navigation
    FocusDirection {
        direction: NavigateDirection,
    },
    MoveWindow {
        direction: NavigateDirection,
    },

    // Resize
    Resize {
        direction: SplitDirection,
        delta: i16,
    },
    Equalize,

    // Tabs
    TabNew {
        filename: Option<String>,
    },
    TabClose,
    TabNext,
    TabPrev,
    TabGoto {
        index: usize,
    },
}

/// Highlight update events
pub enum HighlightEvent {
    /// Add highlights to a buffer
    Add {
        buffer_id: usize,
        highlights: Vec<Highlight>,
    },
    /// Clear a highlight group from a buffer
    ClearGroup {
        buffer_id: usize,
        group: HighlightGroup,
    },
    /// Clear all highlights from a buffer
    ClearAll { buffer_id: usize },
}

/// Syntax provider events
pub enum SyntaxEvent {
    /// Attach a syntax provider to a buffer
    Attach {
        buffer_id: usize,
        syntax: Box<dyn SyntaxProvider>,
    },
    /// Detach syntax provider from a buffer
    Detach { buffer_id: usize },
    /// Request a reparse (after buffer modification)
    Reparse { buffer_id: usize },
}

/// Command event to be processed by runtime
pub struct CommandEvent {
    pub command: CommandRef,
    pub context: CommandContext,
}

/// Visual mode text object selection actions (viw, vi(, vif, etc.)
#[derive(Debug)]
pub enum VisualTextObjectAction {
    /// Select delimiter-based text object (vi(, va{, etc.)
    SelectDelimiter { text_object: TextObject },
    /// Select word text object (viw, vaw, viW, vaW)
    SelectWord { text_object: WordTextObject },
    /// Select semantic text object (vif, vac, etc.) - uses treesitter
    SelectSemantic { text_object: SemanticTextObjectSpec },
}

// =============================================================================
// Mouse Events
// =============================================================================

/// Mouse event from terminal input
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    /// Type of mouse action
    pub kind: MouseEventKind,
    /// Column (x) position in terminal coordinates
    pub column: u16,
    /// Row (y) position in terminal coordinates
    pub row: u16,
    /// Modifier keys held during the event
    pub modifiers: KeyModifiers,
}

/// Type of mouse action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEventKind {
    /// Mouse button pressed down
    Down(MouseButton),
    /// Mouse button released
    Up(MouseButton),
    /// Mouse dragged while button held
    Drag(MouseButton),
    /// Scroll wheel up
    ScrollUp,
    /// Scroll wheel down
    ScrollDown,
    /// Scroll wheel left (horizontal)
    ScrollLeft,
    /// Scroll wheel right (horizontal)
    ScrollRight,
    /// Mouse moved (without button pressed)
    Moved,
}

/// Mouse button identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Left mouse button
    Left,
    /// Right mouse button
    Right,
    /// Middle mouse button (scroll wheel click)
    Middle,
}

impl From<reovim_sys::event::MouseEvent> for MouseEvent {
    fn from(event: reovim_sys::event::MouseEvent) -> Self {
        Self {
            kind: event.kind.into(),
            column: event.column,
            row: event.row,
            modifiers: event.modifiers,
        }
    }
}

impl From<reovim_sys::event::MouseEventKind> for MouseEventKind {
    fn from(kind: reovim_sys::event::MouseEventKind) -> Self {
        use reovim_sys::event::MouseEventKind as CT;
        match kind {
            CT::Down(btn) => Self::Down(btn.into()),
            CT::Up(btn) => Self::Up(btn.into()),
            CT::Drag(btn) => Self::Drag(btn.into()),
            CT::Moved => Self::Moved,
            CT::ScrollUp => Self::ScrollUp,
            CT::ScrollDown => Self::ScrollDown,
            CT::ScrollLeft => Self::ScrollLeft,
            CT::ScrollRight => Self::ScrollRight,
        }
    }
}

impl From<reovim_sys::event::MouseButton> for MouseButton {
    fn from(button: reovim_sys::event::MouseButton) -> Self {
        use reovim_sys::event::MouseButton as CT;
        match button {
            CT::Left => Self::Left,
            CT::Right => Self::Right,
            CT::Middle => Self::Middle,
        }
    }
}
