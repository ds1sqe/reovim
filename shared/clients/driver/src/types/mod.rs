use std::{borrow::Cow, ops::Range};

pub use reovim_arch::Color;

// =============================================================================
// Lifecycle
// =============================================================================

/// Result of probing a module during initialization.
#[derive(Debug)]
pub enum ProbeResult {
    /// Module initialized successfully.
    Success,
    /// Module defers initialization (reason provided).
    Defer(String),
    /// Module failed to initialize.
    Failed(ClientModuleError),
}

/// Error from a client module operation.
#[derive(Debug)]
pub enum ClientModuleError {
    /// Module initialization failed.
    InitFailed {
        reason: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    /// Module exit/cleanup failed.
    ExitFailed { reason: String },
    /// Failed to parse a server notification.
    NotificationParse { kind: String, detail: String },
    /// Generic error (catch-all for backward compatibility).
    Other(String),
}

impl ClientModuleError {
    /// Create a generic error (replaces old `ClientModuleError { message }` pattern).
    #[must_use]
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// Create an init-failed error with an optional source.
    #[must_use]
    pub fn init_failed(
        reason: impl Into<String>,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self::InitFailed {
            reason: reason.into(),
            source,
        }
    }

    /// Create an exit-failed error.
    #[must_use]
    pub fn exit_failed(reason: impl Into<String>) -> Self {
        Self::ExitFailed {
            reason: reason.into(),
        }
    }

    /// Create a notification parse error.
    #[must_use]
    pub fn notification_parse(kind: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::NotificationParse {
            kind: kind.into(),
            detail: detail.into(),
        }
    }

    /// Get the human-readable error message.
    ///
    /// Provided for backward compatibility with code that accessed `.message`.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::InitFailed { reason, .. }
            | Self::ExitFailed { reason }
            | Self::NotificationParse { detail: reason, .. } => reason,
            Self::Other(msg) => msg,
        }
    }
}

impl std::fmt::Display for ClientModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InitFailed { reason, .. } => write!(f, "init failed: {reason}"),
            Self::ExitFailed { reason } => write!(f, "exit failed: {reason}"),
            Self::NotificationParse { kind, detail } => {
                write!(f, "notification parse error ({kind}): {detail}")
            }
            Self::Other(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for ClientModuleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InitFailed {
                source: Some(src), ..
            } => Some(src.as_ref()),
            _ => None,
        }
    }
}

/// Semantic version for a client module.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Client module API version for dynamic loading compatibility checks.
///
/// Loader checks this before calling any FFI symbols. Major version mismatch
/// = incompatible ABI; minor version mismatch = backward compatible.
///
/// ## Version History
///
/// - **0.1.0**: Initial API (lifecycle only: init, exit, `on_all_loaded`)
/// - **0.2.0**: SDK enrichment — `ClientModuleError` enum, `ScopedSurface`,
///   `ClientServiceRegistry`, `ClientModuleRegistry`, expanded `ServerHandle`
///   (`list_commands`, `get_option_metadata`), notification helpers (serde feature),
///   `ModuleContext` gains `services` and `module_registry` fields (Option).
pub const CLIENT_MODULE_API_VERSION: Version = Version::new(0, 2, 0);

/// Check if a required API version is compatible with the provided version.
///
/// Same semver rules as the kernel's `is_compatible()`:
/// - Major must match exactly
/// - Required minor must be <= provided minor
#[must_use]
pub const fn is_client_compatible(required: Version, provided: Version) -> bool {
    if required.major != provided.major {
        return false;
    }
    required.minor <= provided.minor
}

// =============================================================================
// ClientModuleProbe (FFI-safe metadata)
// =============================================================================

/// FFI-safe metadata for a client module (read before instantiation).
///
/// Mirrors the kernel's `ModuleProbe` adapted for `ClientModule`. All fields
/// are fixed-size `#[repr(C)]` so the loader can read them from a `.so`
/// without instantiating the module.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ClientModuleProbe {
    /// Module identifier (UTF-8, null-padded).
    pub id: [u8; 64],
    /// Human-readable name (UTF-8, null-padded).
    pub name: [u8; 128],
    /// Module version.
    pub version: Version,
    /// Required API version.
    pub api_version: Version,
    /// Number of required dependencies (0..=8).
    pub required_deps_count: u8,
    /// Required dependency IDs (UTF-8, null-padded).
    pub required_deps: [[u8; 64]; 8],
    /// Number of optional dependencies (0..=8).
    pub optional_deps_count: u8,
    /// Optional dependency IDs (UTF-8, null-padded).
    pub optional_deps: [[u8; 64]; 8],
}

impl ClientModuleProbe {
    /// Create a new probe with the given metadata.
    #[must_use]
    pub const fn new(id: &str, name: &str, version: Version, api_version: Version) -> Self {
        let mut probe = Self {
            id: [0; 64],
            name: [0; 128],
            version,
            api_version,
            required_deps_count: 0,
            required_deps: [[0; 64]; 8],
            optional_deps_count: 0,
            optional_deps: [[0; 64]; 8],
        };

        // Copy id
        let id_bytes = id.as_bytes();
        let id_len = if id_bytes.len() < 64 {
            id_bytes.len()
        } else {
            64
        };
        let mut i = 0;
        while i < id_len {
            probe.id[i] = id_bytes[i];
            i += 1;
        }

        // Copy name
        let name_bytes = name.as_bytes();
        let name_len = if name_bytes.len() < 128 {
            name_bytes.len()
        } else {
            128
        };
        i = 0;
        while i < name_len {
            probe.name[i] = name_bytes[i];
            i += 1;
        }

        probe
    }

    /// Get the module ID as a string slice.
    #[must_use]
    pub fn id_str(&self) -> &str {
        let len = self
            .id
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.id.len());
        std::str::from_utf8(&self.id[..len]).unwrap_or("")
    }

    /// Get the module name as a string slice.
    #[must_use]
    pub fn name_str(&self) -> &str {
        let len = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.name.len());
        std::str::from_utf8(&self.name[..len]).unwrap_or("")
    }

    /// Get required dependency IDs.
    #[must_use]
    pub fn required_deps(&self) -> Vec<&str> {
        (0..self.required_deps_count as usize)
            .filter_map(|i| {
                let len = self.required_deps[i]
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(64);
                std::str::from_utf8(&self.required_deps[i][..len]).ok()
            })
            .collect()
    }

    /// Get optional dependency IDs.
    #[must_use]
    pub fn optional_deps(&self) -> Vec<&str> {
        (0..self.optional_deps_count as usize)
            .filter_map(|i| {
                let len = self.optional_deps[i]
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(64);
                std::str::from_utf8(&self.optional_deps[i][..len]).ok()
            })
            .collect()
    }

    /// Add a required dependency at the given index (builder pattern).
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // index < 8, always fits in u8
    pub const fn with_required_dep(mut self, index: usize, dep: &str) -> Self {
        if index < 8 {
            let dep_bytes = dep.as_bytes();
            let dep_len = if dep_bytes.len() < 64 {
                dep_bytes.len()
            } else {
                64
            };
            let mut i = 0;
            while i < dep_len {
                self.required_deps[index][i] = dep_bytes[i];
                i += 1;
            }
            if index >= self.required_deps_count as usize {
                self.required_deps_count = (index + 1) as u8;
            }
        }
        self
    }

    /// Add an optional dependency at the given index (builder pattern).
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // index < 8, always fits in u8
    pub const fn with_optional_dep(mut self, index: usize, dep: &str) -> Self {
        if index < 8 {
            let dep_bytes = dep.as_bytes();
            let dep_len = if dep_bytes.len() < 64 {
                dep_bytes.len()
            } else {
                64
            };
            let mut i = 0;
            while i < dep_len {
                self.optional_deps[index][i] = dep_bytes[i];
                i += 1;
            }
            if index >= self.optional_deps_count as usize {
                self.optional_deps_count = (index + 1) as u8;
            }
        }
        self
    }
}

// =============================================================================
// Buffer ID (client-side, NOT kernel re-export)
// =============================================================================

/// Client-side buffer identifier.
///
/// Separate from kernel's `BufferId` to avoid coupling the client driver crate
/// to the kernel. Conversion happens at the bridge boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferId(pub usize);

// =============================================================================
// Platform
// =============================================================================

/// Color depth capability of the display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    Monochrome,
    Ansi16,
    Ansi256,
    TrueColor,
}

/// Rendering model supported by the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderingModel {
    /// Terminal-style cell grid (TUI).
    CellGrid,
    /// Canvas-based rendering (Web).
    Canvas,
    /// Native layout engine (iOS/Android).
    NativeLayout,
}

// =============================================================================
// Geometry
// =============================================================================

/// Axis-aligned rectangle in screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Compute the intersection of two rectangles.
    ///
    /// Returns `None` if they don't overlap.
    #[must_use]
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x.saturating_add(self.width)).min(other.x.saturating_add(other.width));
        let y2 = (self.y.saturating_add(self.height)).min(other.y.saturating_add(other.height));

        if x1 < x2 && y1 < y2 {
            Some(Self::new(x1, y1, x2 - x1, y2 - y1))
        } else {
            None
        }
    }

    /// Check if a point is inside this rectangle.
    #[must_use]
    pub const fn contains_point(&self, x: u16, y: u16) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x.saturating_add(self.width)
            && y < self.y.saturating_add(self.height)
    }
}

/// Edge insets (padding/margin from screen edges).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Insets {
    pub top: u16,
    pub bottom: u16,
    pub left: u16,
    pub right: u16,
}

impl Insets {
    /// Zero insets (no padding on any side).
    pub const ZERO: Self = Self {
        top: 0,
        bottom: 0,
        left: 0,
        right: 0,
    };

    #[must_use]
    pub const fn new(top: u16, bottom: u16, left: u16, right: u16) -> Self {
        Self {
            top,
            bottom,
            left,
            right,
        }
    }
}

// =============================================================================
// Rendering
// =============================================================================

/// Dynamic option value for module configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionValue {
    Bool(bool),
    Integer(i64),
    String(String),
}

/// Kind/type of an editor option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionKind {
    Bool,
    Integer,
    String,
}

/// Metadata about an editor option (type, description, default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionMetadata {
    /// Option name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Default value (if known).
    pub default_value: Option<OptionValue>,
    /// Value kind/type.
    pub kind: OptionKind,
}

/// How a token category should be rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderBehavior {
    /// Apply highlight group styling.
    Highlight,
    /// Replace token with a concealment character.
    Conceal { replacement: Cow<'static, str> },
    /// Override background color.
    Background(Color),
    /// Hide the token entirely.
    Hide,
    /// Render as a full-width line (e.g., horizontal rule).
    FullWidthLine { ch: char, style: Style },
}

/// A line transformed by a module before rendering.
///
/// Each segment is a `(text, optional_style)` pair. Segments are concatenated
/// to form the displayed line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformedLine {
    pub segments: Vec<(String, Option<Style>)>,
}

/// A virtual line injected by a module (not part of the buffer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualLine {
    /// Buffer line this virtual line is anchored to.
    pub buffer_line: usize,
    /// Whether to insert before or after the anchor line.
    pub position: VirtualLinePosition,
    /// Text content of the virtual line.
    pub content: String,
    /// Style for the virtual line.
    pub style: Style,
}

/// Position of a virtual line relative to its anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualLinePosition {
    Before,
    After,
}

/// An inline decoration applied to a range of columns on a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineDecoration {
    pub col_start: u16,
    pub col_end: u16,
    pub style: Style,
}

// =============================================================================
// Chrome
// =============================================================================

/// Position where chrome (UI furniture) is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromePosition {
    Top,
    Bottom,
    Left,
    Right,
    Overlay,
}

// =============================================================================
// Gutter (Annotations)
// =============================================================================

/// Context passed to annotation modules for gutter rendering.
#[derive(Debug, Clone)]
pub struct AnnotationContext {
    pub buffer_id: BufferId,
    pub total_lines: usize,
    pub visible_range: (usize, usize),
    pub cursor_line: usize,
    pub gutter_style: Style,
}

/// Width of an annotation column in the gutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnWidth {
    /// Fixed width in columns.
    Fixed(u16),
    /// Dynamic width with a minimum.
    Dynamic(u16),
}

/// A single cell in the gutter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GutterCell {
    pub text: String,
    pub style: Style,
}

// =============================================================================
// Buffer Events
// =============================================================================

/// Event describing a buffer content change.
#[derive(Debug, Clone)]
pub struct BufferUpdateEvent {
    pub buffer_id: BufferId,
    pub revision: u64,
    pub changed_range: Range<usize>,
    pub new_lines: Vec<String>,
    pub total_lines: usize,
}

// =============================================================================
// Layout
// =============================================================================

/// Window identifier (client-side).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub usize);

/// A window's position and size in the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowLayout {
    pub window_id: WindowId,
    pub bounds: Rect,
}

// =============================================================================
// Viewport Rendering
// =============================================================================

/// Line number display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineNumberMode {
    /// No line numbers.
    #[default]
    None,
    /// Absolute line numbers (1, 2, 3...).
    Absolute,
    /// Relative line numbers (distance from cursor).
    Relative,
    /// Hybrid: absolute for cursor line, relative for others.
    Hybrid,
}

/// Cursor position for viewport rendering.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CursorInfo {
    /// Line number (0-indexed).
    pub line: u64,
    /// Column number (0-indexed).
    pub column: u64,
}

/// Visual selection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// Character-wise selection.
    Char,
    /// Line-wise selection.
    Line,
    /// Block (column) selection.
    Block,
}

/// A visual selection range for viewport rendering.
#[derive(Debug, Clone)]
pub struct SelectionInfo {
    /// Start line (0-indexed).
    pub start_line: u64,
    /// Start column (0-indexed).
    pub start_col: u64,
    /// End line (0-indexed).
    pub end_line: u64,
    /// End column (0-indexed).
    pub end_col: u64,
    /// Selection mode.
    pub mode: SelectionMode,
    /// Background color for this selection.
    pub color: Color,
}

/// Presence information for a remote client in the viewport.
#[derive(Debug, Clone)]
pub struct RemoteClientInfo {
    /// Client's unique ID.
    pub client_id: u64,
    /// User-friendly display name.
    pub display_name: String,
    /// Cursor line (0-indexed).
    pub cursor_line: u64,
    /// Cursor column (0-indexed).
    pub cursor_col: u64,
    /// Current mode name.
    pub mode: String,
    /// Cursor color from the palette.
    pub cursor_color: Color,
    /// Selection range (if in visual mode).
    pub selection: Option<SelectionInfo>,
}

/// Read-only context for viewport rendering.
///
/// Carries all state the viewport renderer needs to produce a frame.
/// Constructed by the compositor (render engine) from TUI core state
/// and passed to `ViewportRenderer::render_viewport()`.
#[derive(Debug)]
pub struct ViewportContext<'a> {
    /// Buffer being rendered.
    pub buffer_id: Option<BufferId>,
    /// Buffer content lines.
    pub buffer_lines: Option<&'a [String]>,
    /// Local cursor position.
    pub cursor: Option<CursorInfo>,
    /// First visible buffer line (0-indexed).
    pub scroll_top: usize,
    /// Local selection (if in visual mode).
    pub local_selection: Option<SelectionInfo>,
    /// Remote client presence info.
    pub remote_clients: &'a [RemoteClientInfo],
    /// Folded line ranges: `(start_line, line_count)`.
    pub fold_ranges: &'a [(usize, usize)],
    /// Virtual lines injected by modules.
    pub virtual_lines: &'a [VirtualLine],
    /// Window opacity (`1.0` = fully opaque).
    pub opacity: f32,
    /// Line number display mode.
    pub line_number_mode: LineNumberMode,
    /// Gutter width (for line numbers + annotations).
    pub gutter_width: u16,
    /// Sidebar width (from left-chrome modules).
    pub sidebar_width: u16,
    /// Whether in insert mode (affects conceal bypass).
    pub is_insert_mode: bool,
    /// Whether to render cursor in buffer (headless mode).
    pub render_self_cursor: bool,
    /// Local client ID (for excluding from remote rendering).
    pub my_client_id: u64,
}

// =============================================================================
// Style
// =============================================================================

/// Bitflags for text attributes (bold, italic, etc.).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attributes(u8);

impl Attributes {
    pub const BOLD: Self = Self(0b0000_0001);
    pub const ITALIC: Self = Self(0b0000_0010);
    pub const UNDERLINE: Self = Self(0b0000_0100);
    pub const STRIKETHROUGH: Self = Self(0b0000_1000);
    pub const REVERSE: Self = Self(0b0001_0000);
    pub const DIM: Self = Self(0b0010_0000);

    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn set(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub const fn unset(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }
}

impl std::ops::BitOr for Attributes {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for Attributes {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

/// Platform-agnostic text style.
///
/// Defined locally in the client driver crate (not re-exported from display
/// driver) to keep this crate platform-independent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub attributes: Attributes,
}

impl Style {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: Attributes::new(),
        }
    }

    /// Set foreground color (builder pattern).
    #[must_use]
    pub const fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    /// Set background color (builder pattern).
    #[must_use]
    pub const fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    /// Enable bold attribute (builder pattern).
    #[must_use]
    pub const fn bold(mut self) -> Self {
        self.attributes.set(Attributes::BOLD);
        self
    }

    /// Enable italic attribute (builder pattern).
    #[must_use]
    pub const fn italic(mut self) -> Self {
        self.attributes.set(Attributes::ITALIC);
        self
    }

    /// Enable underline attribute (builder pattern).
    #[must_use]
    pub const fn underline(mut self) -> Self {
        self.attributes.set(Attributes::UNDERLINE);
        self
    }

    /// Enable dim attribute (builder pattern).
    #[must_use]
    pub const fn dim(mut self) -> Self {
        self.attributes.set(Attributes::DIM);
        self
    }

    /// Enable reverse attribute (builder pattern).
    #[must_use]
    pub const fn reverse(mut self) -> Self {
        self.attributes.set(Attributes::REVERSE);
        self
    }
}

// =============================================================================
// Input Events
// =============================================================================

/// Platform-agnostic input event.
///
/// Abstracts over terminal, web, and mobile input sources. Each platform
/// converts its native events into `InputEvent` at the boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    /// Keyboard event.
    Key(KeyEvent),
    /// Pointer (mouse/trackpad) event.
    Pointer(PointerEvent),
    /// Touch event (mobile/tablet).
    Touch(TouchEvent),
    /// Focus change event.
    Focus(FocusEvent),
    /// Paste event (bracketed paste or clipboard).
    Paste(String),
}

/// Keyboard event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    /// Platform-agnostic key code.
    pub code: KeyCode,
    /// Modifier keys held during the event.
    pub modifiers: Modifiers,
}

impl KeyEvent {
    /// Create a new key event.
    #[must_use]
    pub const fn new(code: KeyCode, modifiers: Modifiers) -> Self {
        Self { code, modifiers }
    }
}

/// Platform-agnostic key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// A unicode character.
    Char(char),
    /// Enter/Return.
    Enter,
    /// Escape.
    Esc,
    /// Tab.
    Tab,
    /// Backspace.
    Backspace,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Insert.
    Insert,
    /// Delete.
    Delete,
    /// Function key (F1-F12).
    F(u8),
    /// Null/unknown key.
    Null,
}

/// Modifier key bitflags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(0b0000_0001);
    pub const CTRL: Self = Self(0b0000_0010);
    pub const ALT: Self = Self(0b0000_0100);
    pub const SUPER: Self = Self(0b0000_1000);

    /// Create empty modifiers.
    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Check if this modifier set contains the given modifier.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Set a modifier flag.
    pub const fn set(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Unset a modifier flag.
    pub const fn unset(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    /// Check if no modifiers are set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Get raw bits.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for Modifiers {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

/// Pointer (mouse/trackpad) event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerEvent {
    /// What happened.
    pub kind: PointerKind,
    /// Column (0-indexed).
    pub x: u16,
    /// Row (0-indexed).
    pub y: u16,
    /// Modifier keys held.
    pub modifiers: Modifiers,
}

/// Kind of pointer event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    /// Button pressed.
    Down(PointerButton),
    /// Button released.
    Up(PointerButton),
    /// Drag with button held.
    Drag(PointerButton),
    /// Mouse moved (no button).
    Move,
    /// Scroll up.
    ScrollUp,
    /// Scroll down.
    ScrollDown,
}

/// Pointer button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

/// Touch event (mobile/tablet).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchEvent {
    /// What happened.
    pub kind: TouchKind,
    /// Touch identifier (for multi-touch tracking).
    pub id: u64,
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
}

/// Kind of touch event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchKind {
    /// Touch started.
    Start,
    /// Touch moved.
    Move,
    /// Touch ended.
    End,
    /// Touch cancelled.
    Cancel,
}

/// Focus change event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusEvent {
    /// Window/terminal gained focus.
    Gained,
    /// Window/terminal lost focus.
    Lost,
}

#[cfg(test)]
mod tests;
