# Type Definitions

All types referenced by `ClientModule` methods. Defined in CLIENT DRIVER.

`Style` and `Color` are re-exported from `reovim-arch` (the existing platform
abstraction crate). `Style` has `fg: Option<Color>`, `bg: Option<Color>`,
`attributes: Attributes`. These types already exist in the codebase and are
shared between server and client.

## Lifecycle Types

```rust
/// Module initialization result.
/// Mirrors server-side ProbeResult from reovim-kernel.
enum ProbeResult {
    /// Module initialized successfully.
    Success,
    /// Module cannot initialize yet — retry later.
    /// Used when a dependency is not ready.
    Defer(String),
    /// Module failed to initialize permanently.
    Failed(ClientModuleError),
}

/// Client-side module error type.
/// Named `ClientModuleError` to avoid collision with kernel's `ModuleError`.
struct ClientModuleError {
    pub message: String,
}

impl ClientModuleError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
}

/// Semantic version. Same as server-side Version from reovim-kernel.
struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
}
```

## Platform Types

```rust
/// Color depth capability of the rendering surface.
enum ColorDepth {
    /// Monochrome — no color support.
    Monochrome,
    /// ANSI 16 standard colors.
    Ansi16,
    /// Extended 256-color palette.
    Ansi256,
    /// 24-bit true color (RGB).
    TrueColor,
}

/// Buffer metadata from the server. Returned by ServerHandle::get_buffer_metadata().
struct BufferMetadata {
    pub filename: Option<String>,
    pub filetype: Option<String>,
    pub modified: bool,
    pub readonly: bool,
    pub total_lines: usize,
}
```

## Buffer Types

```rust
/// Incremental buffer update event.
/// Carries ONLY what changed, not the full buffer.
struct BufferUpdateEvent {
    pub buffer_id: BufferId,
    /// Monotonically increasing revision number.
    /// Modules can use this to detect out-of-order or missed updates.
    pub revision: u64,
    /// Range of lines that changed (start..end, 0-indexed).
    pub changed_range: Range<usize>,
    /// New content for the changed range.
    pub new_lines: Vec<String>,
    /// Total line count after the update.
    pub total_lines: usize,
}

/// Buffer content snapshot for viewport rendering.
struct BufferContent {
    pub buffer_id: BufferId,
    pub lines: Vec<String>,       // logical lines, UTF-8, no trailing newline
    pub cursor: (usize, usize),   // (line, col)
    pub selection: Option<Selection>,
    pub scroll_offset: usize,
}

struct Selection {
    pub start: (usize, usize),  // (line, col)
    pub end: (usize, usize),
    pub mode: SelectionMode,
}

enum SelectionMode {
    Character,
    Line,
    Block,
}
```

## Rendering Types

```rust
/// Option value from server.
enum OptionValue {
    Bool(bool),
    Integer(i64),
    String(String),
}

/// How a token should be rendered after classification.
enum RenderBehavior {
    /// Apply the theme's highlight style for this category.
    Highlight,
    /// Conceal: replace the token visually.
    Conceal { replacement: Cow<'static, str> },
    /// Override background only.
    Background(Color),
    /// Hide the token entirely (zero width).
    Hide,
    /// Full-width line decoration.
    FullWidthLine { ch: char, style: Style },
}

/// A transformed line — replaces the original visual representation.
/// Segment-based: each segment is a (text, optional_style) pair.
struct TransformedLine {
    pub segments: Vec<(String, Option<Style>)>,
}

/// A virtual line inserted between real buffer lines.
struct VirtualLine {
    pub buffer_line: usize,
    pub position: VirtualLinePosition,
    pub content: String,
    pub style: Style,
}

enum VirtualLinePosition {
    Before,
    After,
}

/// Inline decoration applied to a span within a line.
struct InlineDecoration {
    pub col_start: u16,
    pub col_end: u16,
    pub style: Style,
}
```

## Gutter Types

```rust
/// Context for gutter annotation rendering.
struct AnnotationContext {
    pub buffer_id: BufferId,
    pub total_lines: usize,
    pub visible_range: (usize, usize),  // first..last visible line
    pub cursor_line: usize,
    pub gutter_style: Style,  // gutter default style from ThemeProvider
}

/// Width specification for a gutter column.
enum ColumnWidth {
    /// Fixed number of characters.
    Fixed(u16),
    /// Dynamic — the current computed width (e.g., log10(total_lines) + 1).
    /// The u16 is the current width value, recomputed by the module
    /// when buffer content changes.
    Dynamic(u16),
}

/// A single gutter cell for one line in one column.
/// `text` is `String` because dynamic content (line numbers, diagnostics)
/// must be formatted per-line. This is a Phase 2 allocation — acceptable
/// per the "guideline, not hard constraint" policy in 08-extensibility.md.
/// For static content, modules can cache formatted strings in Phase 1.
struct GutterCell {
    pub text: String,
    pub style: Style,
}
```

## Cross-Module Communication Types

```rust
/// Key for looking up a component provider in the ServiceRegistry.
/// Uses &'static str to ensure keys are string literals (compile-time known).
struct ComponentProviderKey(pub &'static str);

impl ComponentProviderKey {
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }
}

/// A component that can render a short text snippet.
/// Used for cross-module data sharing (e.g., git-signs provides branch
/// name, statusline consumes it). Defined in CLIENT DRIVER.
trait ComponentProvider: Send + Sync {
    fn render(&self) -> Option<Cow<'static, str>>;
}

/// ServiceRegistry API for cross-module communication.
/// Re-exported from reovim-kernel. Uses TypeId-based primary lookup
/// with MultiServiceRegistry for keyed sub-registries.
///
/// Type alias (MultiServiceRegistry internally stores Arc<T>):
///   type ComponentProviderRegistry =
///       MultiServiceRegistry<ComponentProviderKey, dyn ComponentProvider>;
///
/// The pattern for component providers:
///   1. CORE registers ComponentProviderRegistry in ServiceRegistry at startup.
///   2. Producer modules call get_or_create() during init() or on_all_loaded()
///      to register their providers.
///   3. Consumer modules call get() during on_all_loaded() or Phase 1 events.
///
/// Producer usage (get_or_create returns Arc<R>, never None):
///   let providers = ctx.services.get_or_create::<ComponentProviderRegistry>();
///   providers.register(ComponentProviderKey::new("git-branch"), arc_provider);
///
/// Consumer usage (get returns Option<Arc<R>>):
///   let providers = ctx.services.get::<ComponentProviderRegistry>();
///   if let Some(p) = providers {
///       let branch = p.get(&ComponentProviderKey::new("git-branch"));
///   }
```

## Registry Types

```rust
/// Module registry. CORE owns this. Platform binary populates it
/// via register_modules() before CORE starts the event loop.
struct ModuleRegistry {
    modules: Vec<Box<dyn ClientModule>>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self { modules: Vec::new() }
    }

    /// Register a module. Order matters only as a tiebreaker for
    /// equal priority — first registered wins.
    pub fn register(&mut self, module: Box<dyn ClientModule>) {
        self.modules.push(module);
    }

    /// Consume the registry and return all modules for CORE to own.
    pub fn into_modules(self) -> Vec<Box<dyn ClientModule>> {
        self.modules
    }
}
```

## Geometry Types

```rust
struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

struct Insets {
    pub top: u16,
    pub bottom: u16,
    pub left: u16,
    pub right: u16,
}
```

## Input Types

```rust
struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: Modifiers,
}

enum KeyCode {
    Char(char),
    Enter, Esc, Backspace, Tab, Delete,
    Up, Down, Left, Right,
    Home, End, PageUp, PageDown,
    F(u8),
    // ... extensible
}

bitflags! {
    struct Modifiers: u8 {
        const SHIFT   = 0b0001;
        const CTRL    = 0b0010;
        const ALT     = 0b0100;
        const META    = 0b1000;
    }
}

struct PointerEvent {
    pub kind: PointerKind,
    pub x: u16,
    pub y: u16,
    pub button: Option<MouseButton>,
    pub scroll_delta: Option<(i16, i16)>,
}

enum PointerKind { Press, Release, Move, Scroll }
enum MouseButton { Left, Right, Middle }

struct TouchEvent {
    pub kind: TouchKind,
    pub id: u64,
    pub x: u16,
    pub y: u16,
}

enum TouchKind { Start, Move, End, Cancel }

enum FocusEvent { Gained, Lost }
```
