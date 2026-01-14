# Driver Layer

Drivers (`lib/drivers/*`) implement traits defined by the kernel. Each driver is a separate crate.

## Driver Overview

| Driver | Crate | Purpose |
|--------|-------|---------|
| `command/` | `reovim-driver-command` | Command traits and execution |
| `syntax/` | `reovim-driver-syntax` | Syntax highlighting abstraction |
| `input/` | `reovim-driver-input` | Keyboard, mouse, clipboard |
| `display/` | `reovim-driver-display` | Frame buffer, compositor |
| `lsp/` | `reovim-driver-lsp` | LSP client infrastructure |
| `net/` | `reovim-driver-net` | RPC server, transports |
| `vfs/` | `reovim-driver-vfs` | Virtual filesystem |
| `log/` | `reovim-driver-log` | Logger implementation (tracing) |

---

## command/ - Command Driver

Command traits and execution infrastructure.

```rust
// lib/drivers/command/src/

/// Command metadata (identity, description, arguments)
pub trait Command {
    fn id(&self) -> CommandId;
    fn description(&self) -> &'static str;
    fn args(&self) -> Vec<ArgSpec>;
}

/// Command execution (takes KernelContext)
pub trait CommandHandler: Command + Send + Sync {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult;
}

/// Argument specification
pub struct ArgSpec {
    pub name: &'static str,
    pub kind: ArgKind,
    pub description: &'static str,
    pub required: bool,
}

pub enum ArgKind {
    Count,     // Numeric count (e.g., 3j)
    Register,  // Register name (e.g., "a)
    Motion,    // Motion argument
    String,    // String argument
}

/// Command execution context (parsed arguments)
pub struct CommandContext {
    count: Option<usize>,
    register: Option<char>,
    // ...
}

/// Command execution result
pub enum CommandResult {
    Success,
    Error(String),
    Quit,
    ForceQuit,
}
```

---

## syntax/ - Syntax Driver

Abstraction for syntax highlighting. **No tree-sitter in this crate.**

```rust
// lib/drivers/syntax/src/

pub trait SyntaxDriver: Send + Sync {
    fn name(&self) -> &str;
    fn extensions(&self) -> &[&str];

    fn parse(&mut self, source: &str);
    fn highlight(&self, range: Range) -> Vec<HighlightSpan>;
    fn fold_regions(&self) -> Vec<FoldRegion>;
}

pub enum HighlightGroup {
    Keyword,
    Function,
    String,
    Comment,
    Type,
    Variable,
    Operator,
    // ... 120+ groups
}

pub struct HighlightSpan {
    pub range: Range,
    pub group: HighlightGroup,
}
```

### Implementation Location

Tree-sitter implementations live in plugins, not drivers:

```
plugins/features/treesitter/    # TreeSitterDriver impl
plugins/languages/rust/         # Rust queries
plugins/languages/python/       # Python queries
```

---

## input/ - Input Driver

Keyboard, mouse, and clipboard abstractions.

```rust
// lib/drivers/input/src/

pub trait KeyboardDriver: Send + Sync {
    fn read_key(&mut self) -> Option<KeyEvent>;
    fn pending(&self) -> bool;
}

pub trait MouseDriver: Send + Sync {
    fn read_mouse(&mut self) -> Option<MouseEvent>;
    fn enable(&mut self);
    fn disable(&mut self);
}

pub trait ClipboardDriver: Send + Sync {
    fn get(&self) -> Result<String>;
    fn set(&mut self, content: &str) -> Result<()>;
}

pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: Modifiers,
    pub kind: KeyEventKind,  // Press, Release, Repeat
}

impl KeyEvent {
    pub fn new(code: KeyCode) -> Self;
    pub fn with_modifiers(code: KeyCode, modifiers: Modifiers) -> Self;
    pub fn is_press(&self) -> bool;  // Check if this is a key press
}

pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub position: (u16, u16),
    pub modifiers: Modifiers,
}

// Key sequence for multi-key bindings (e.g., "gg", "<C-w>h")
pub struct KeySequence {
    keys: Vec<KeyEvent>,
}

impl KeySequence {
    pub fn parse(s: &str) -> Option<Self>;  // Parse "gg", "<C-w>h", etc.
    pub fn push(&mut self, key: KeyEvent);
    pub fn starts_with(&self, other: &Self) -> bool;
}

// Mode behavior trait
pub trait ModeInput {
    fn accepts_char_input(&self) -> bool;  // Does this mode accept character input?
}
```

---

## display/ - Display Driver

Frame buffer and window compositor.

```rust
// lib/drivers/display/src/

pub trait DisplayDriver: Send + Sync {
    fn size(&self) -> (u16, u16);
    fn resize(&mut self, width: u16, height: u16);

    fn draw(&mut self, x: u16, y: u16, cell: Cell);
    fn flush(&mut self) -> Result<()>;

    fn set_cursor(&mut self, x: u16, y: u16);
    fn set_cursor_style(&mut self, style: CursorStyle);
    fn show_cursor(&mut self, visible: bool);
}

pub struct Cell {
    pub char: char,
    pub style: Style,
}

pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub modifiers: StyleModifiers,
}

pub enum CursorStyle {
    Block,      // Normal mode (vim)
    Bar,        // Insert mode (vim) - thin vertical line
    Underline,  // Replace mode
    Hidden,
}

// Mode display behavior trait
pub trait ModeDisplay {
    fn cursor_style(&self) -> CursorStyle;  // Cursor shape for this mode
    fn status_text(&self) -> &'static str;   // Status line text (e.g., "NORMAL", "INSERT")
}
```

### Compositor

Layer-based rendering:

```rust
pub trait Compositor {
    fn push_layer(&mut self, layer: Box<dyn Layer>);
    fn pop_layer(&mut self);
    fn render(&mut self, display: &mut dyn DisplayDriver);
}

pub trait Layer {
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn handle_event(&mut self, event: &Event) -> EventResult;
}
```

---

## lsp/ - LSP Driver

Language Server Protocol client infrastructure.

```rust
// lib/drivers/lsp/src/

pub trait LspClient: Send + Sync {
    fn initialize(&mut self, root: &Path) -> Result<InitializeResult>;
    fn shutdown(&mut self) -> Result<()>;

    fn did_open(&mut self, doc: TextDocumentItem) -> Result<()>;
    fn did_change(&mut self, uri: &Url, changes: Vec<TextEdit>) -> Result<()>;
    fn did_save(&mut self, uri: &Url) -> Result<()>;
    fn did_close(&mut self, uri: &Url) -> Result<()>;

    fn completion(&mut self, params: CompletionParams) -> Result<CompletionList>;
    fn hover(&mut self, params: HoverParams) -> Result<Option<Hover>>;
    fn definition(&mut self, params: GotoParams) -> Result<Vec<Location>>;
    fn references(&mut self, params: ReferenceParams) -> Result<Vec<Location>>;
}

pub struct LspMessage {
    pub jsonrpc: String,
    pub id: Option<RequestId>,
    pub method: Option<String>,
    pub params: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<ResponseError>,
}
```

---

## net/ - Network Driver

RPC server and transport abstractions.

```rust
// lib/drivers/net/src/

pub trait RpcServer: Send + Sync {
    fn start(&mut self, config: TransportConfig) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn handle_request(&mut self, request: RpcRequest) -> RpcResponse;
}

pub enum TransportConfig {
    Stdio,
    Tcp { host: String, port: u16 },
    UnixSocket { path: PathBuf },
}

pub struct RpcRequest {
    pub id: RequestId,
    pub method: String,
    pub params: Value,
}

pub struct RpcResponse {
    pub id: RequestId,
    pub result: Option<Value>,
    pub error: Option<RpcError>,
}
```

### Transport Layer

```rust
pub trait TransportReader: Send {
    fn read(&mut self) -> Result<RpcMessage>;
}

pub trait TransportWriter: Send {
    fn write(&mut self, message: &RpcMessage) -> Result<()>;
}

pub trait TransportListener: Send {
    fn accept(&mut self) -> Result<(Box<dyn TransportReader>, Box<dyn TransportWriter>)>;
}
```

---

## vfs/ - Virtual Filesystem

File operations abstraction.

```rust
// lib/drivers/vfs/src/

pub trait VfsDriver: Send + Sync {
    fn read(&self, path: &Path) -> Result<Vec<u8>>;
    fn write(&mut self, path: &Path, content: &[u8]) -> Result<()>;
    fn exists(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;
    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>>;
    fn create_dir(&mut self, path: &Path) -> Result<()>;
    fn remove(&mut self, path: &Path) -> Result<()>;
    fn metadata(&self, path: &Path) -> Result<Metadata>;
}

pub struct DirEntry {
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

pub struct Metadata {
    pub size: u64,
    pub modified: SystemTime,
    pub is_readonly: bool,
}
```

---

## log/ - Log Driver

Logger implementation using `tracing`.

```rust
// lib/drivers/log/src/

pub struct TracingLogger {
    // Uses tracing subscriber
}

impl Logger for TracingLogger {
    fn log(&self, level: Level, message: &str) {
        match level {
            Level::Error => tracing::error!("{}", message),
            Level::Warn => tracing::warn!("{}", message),
            Level::Info => tracing::info!("{}", message),
            Level::Debug => tracing::debug!("{}", message),
            Level::Trace => tracing::trace!("{}", message),
        }
    }
}
```

This is the ONLY place where `tracing` dependency exists.

---

## Driver Registration

Drivers are registered at startup in the runner:

```rust
// runner/src/main.rs

let display = CrosstermDisplay::new();
let input = CrosstermInput::new();
let vfs = StandardVfs::new();
let logger = TracingLogger::new();

let ctx = KernelContext::builder()
    .display(display)
    .input(input)
    .vfs(vfs)
    .logger(logger)
    .build();
```

---

## Related Documents

- [Overview](./overview.md) - Architecture overview
- [Kernel Subsystems](./kernel.md) - Kernel internals
- [Module System](./modules.md) - Dynamic modules
