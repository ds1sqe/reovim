# Feature Modules

Overview of major feature modules in reovim.

## Microscope (`plugins/features/microscope/`)

Fuzzy finder for files, buffers, grep, and commands. Uses nucleo for high-performance fuzzy matching.

### Components

- `MicroscopeState` - Current UI state (query, selected index)
- `MicroscopeMatcher` - nucleo-based fuzzy matching
- `Picker` trait - Extensible picker system
- Built-in pickers: files, buffers, live_grep, recent, commands, help, keymaps

**Keybindings:** `Space f` prefix (e.g., `Space ff` for files)

## Explorer (`plugins/features/explorer/`)

Tree-view file browser with file operations.

### Components

- `ExplorerState` - Tree structure and cursor position
- `ExplorerNode` - File/directory representation
- Commands for navigation, tree ops, file ops

**Keybinding:** `Space e` to toggle

## Completion Plugin (`plugins/features/completion/`)

Auto-completion with background processing, following the treesitter decoupling pattern.

### Plugin Structure

```
plugins/features/completion/src/
├── lib.rs          # CompletionPlugin only
├── state.rs        # SharedCompletionManager
├── window.rs       # CompletionPluginWindow
├── cache.rs        # CompletionCache (ArcSwap)
├── saturator.rs    # Background completion task
├── registry.rs     # SourceRegistry, SourceSupport
├── events.rs       # RegisterSource event
├── commands.rs     # Unified command-event types
└── source/
    └── buffer.rs   # BufferWordsSource
```

### Core Traits (in `lib/core/src/completion/`)

```rust
pub struct CompletionContext {
    pub buffer_id: usize,
    pub cursor_row: u32,
    pub cursor_col: u32,
    pub line: String,
    pub prefix: String,
    pub word_start_col: u32,
}

pub struct CompletionItem {
    pub label: String,
    pub source_id: String,
    pub kind: CompletionKind,
    pub insert_text: Option<String>,
    pub detail: Option<String>,
}
```

### Plugin Architecture

- `SharedCompletionManager` - Thread-safe wrapper holding registry, cache, saturator
- `CompletionCache` - ArcSwap-based lock-free cache for render access
- `CompletionSaturator` - Background tokio task for non-blocking completion
- `SourceRegistry` - Dynamic source registration with priority ordering
- `SourceSupport` trait - Interface for completion sources to implement

### Data Flow

```
┌─────────────────┐     ┌───────────────────────┐     ┌─────────────────┐
│  Trigger Event  │────▶│ CompletionSaturator   │────▶│ CompletionCache │
│   (Alt-Space)   │     │     (background)      │     │    (ArcSwap)    │
└─────────────────┘     └───────────────────────┘     └─────────────────┘
                                                              │
                                                              ▼
                                                  ┌────────────────────────┐
                                                  │ CompletionPluginWindow │
                                                  │    (lock-free read)    │
                                                  └────────────────────────┘
```

### Source Registration

External plugins register completion sources via `RegisterSource` event:

```rust
bus.emit(RegisterSource {
    source: Arc::new(LspCompletionSource::new(client)),
});
```

**Built-in Source:** `BufferWordsSource` - Extracts words from current buffer

**Keybindings:**
- `Alt-Space` (insert mode) - Trigger completion
- `Ctrl-n`/`Ctrl-p` - Navigate suggestions
- `Ctrl-y` - Confirm selection
- `Escape` - Dismiss popup

## Treesitter Plugin (`plugins/features/treesitter/`)

Syntax highlighting and semantic features powered by tree-sitter.

### Plugin Structure

- `TreesitterPlugin` - Main plugin, handles events and lifecycle
- `SharedTreesitterManager` - Shared state for all buffers
- `LanguageRegistry` - Dynamic language registration
- `BufferParser` - Per-buffer incremental parser
- `Highlighter` - Query execution and highlight generation
- `TextObjectResolver` - Semantic text object bounds

### Language Plugins (`plugins/languages/{lang}/`)

- Each language implements `LanguageSupport` trait
- Registers with treesitter via `RegisterLanguage` event
- Provides grammar, highlight queries, decoration queries

**Supported languages:** Rust, C, JavaScript, Python, JSON, TOML, Markdown

### Features

- Dynamic language registration via event bus
- Incremental parsing with debouncing
- Visible-range-only highlighting for performance
- Semantic text objects (function, class, etc.)
- Core accesses via `plugin_state.text_object_source()`

## Code Folding (`lib/core/src/folding.rs`)

Code folding with treesitter-computed ranges.

### Components

- `FoldManager` - Per-buffer fold state
- `FoldState` - Collapsed/expanded tracking
- `FoldRange` - Foldable region with preview

**Keybindings:**
- `za` - Toggle fold
- `zo`/`zc` - Open/close fold
- `zR`/`zM` - Open/close all folds

## Jump List (`lib/core/src/jump_list/`)

Navigation history for Ctrl-O/Ctrl-I.

**Behavior:** Records cursor position before jump commands, allows backtracking

## Registers (`lib/core/src/registers/`)

Multi-register copy/paste storage.

**Features:** Default register `"`, named registers `a-z`

## Mode State System

Editor mode is represented by a multi-dimensional `ModeState`:

```rust
pub struct ModeState {
    pub interactor_id: ComponentId,  // Editor, CommandLine, or plugin-defined
    pub edit_mode: EditMode,         // Normal, Insert, Visual
    pub sub_mode: SubMode,           // None, Command, OperatorPending, Interactor(id)
}

// ComponentId is extensible (plugins define their own IDs)
pub struct ComponentId(pub &'static str);

// Core defines only essential IDs
impl ComponentId {
    pub const EDITOR: Self = Self("editor");
    pub const COMMAND_LINE: Self = Self("command_line");
    pub const STATUS_LINE: Self = Self("status_line");
    pub const TAB_LINE: Self = Self("tab_line");
    pub const WINDOW: Self = Self("window");  // Window mode sub-mode
}
```

### Core Constructors

- `ModeState::normal()` - Editor + Normal mode
- `ModeState::insert()` - Editor + Insert mode
- `ModeState::visual()` - Editor + Visual mode
- `ModeState::command()` - Editor + Command sub-mode
- `ModeState::operator_pending(op, count)` - Operator-pending mode
- `ModeState::with_interactor(id)` - Generic interactor focus
- `ModeState::with_interactor_insert(id)` - Interactor + Insert mode

### State Checks

- `is_normal()`, `is_insert()`, `is_visual()` - Edit mode checks
- `is_command()`, `is_operator_pending()` - Sub-mode checks
- `is_interactor(id)` - Check if current interactor matches given ID
- `is_editor_focus()` - Check if focused on editor
- `accepts_char_input_with(registry)` - Check if mode accepts character input

### Hierarchical Display

The status line shows mode in hierarchical format: `Kind | Mode | SubMode`

```rust
// Examples:
"Editor | Normal"           // Basic editor
"Editor | Normal | Window"  // Window mode active
"Editor | Insert"           // Insert mode
"Explorer | Normal"         // Explorer focused
"Editor | Normal | Operator" // Operator pending (d, y, c)
```

## Interactor System (`lib/core/src/interactor/`)

Manages input-receiving components (UIComponents) that can receive focus and handle input.

### Components

- `ComponentId` - Unique identifier for interactors
- `UIComponent` trait - Interface for input-receiving components
- `UIComponentRegistry` - Manages registered UI components
- `InputResult` - Result enum: `NotHandled`, `Handled`, `SendEvent(RuntimeEvent)`
- `InteractorConfig` - Configuration for interactor input behavior
- `InteractorRegistry` - Registry for interactor configurations

### Registration Pattern

```rust
// Plugins register their UIComponents during build()
fn build(&self, ctx: &mut PluginContext) {
    ctx.register_ui_component(Explorer::new());
}

// Runtime dispatches generically via ComponentId lookup
fn handle_focus_input(&mut self, input: InputEvent) {
    if let Some(component) = self.ui_registry.get_mut(&self.mode_state.interactor_id) {
        component.handle_input(input);
    }
}
```

## Background Saturator Pattern

To eliminate scroll lag caused by synchronous syntax highlighting, reovim uses a **per-buffer saturator** pattern.

### Problem Solved

- Synchronous `syntax.highlight_range()` blocked render for ~46ms
- Caused visible stuttering during markdown scrolling

### Solution

- Background task owns syntax/decoration providers
- Computes highlights in parallel with render
- Lock-free `ArcSwap` cache for instant reads (~6µs)
- `RenderSignal` triggers re-render when cache updates

```
┌─────────────┐     ┌──────────────────┐     ┌─────────────┐
│   Render    │────▶│ ArcSwap Cache    │◀────│  Saturator  │
│  (instant)  │     │   (lock-free)    │     │ (background)│
└─────────────┘     └──────────────────┘     └─────────────┘
      │                                              │
      │ ~6µs read                                    │ ~46ms compute
      └──────────────────────────────────────────────┘
                    Total: ~0.5ms render
```

### Key Design

- `mpsc::channel(1)` with `try_send()` - only latest viewport matters
- Cache stores `(hash, data)` per line for validation
- `ArcSwap::store()` for atomic swap (no locks)

See [Saturator Documentation](../features/saturator.md) for full details.

## Related Documentation

- [Plugin System](./plugins.md) - Plugin architecture
- [Event System](../events/overview.md) - Event communication
- [Rendering](../rendering/overview.md) - Render pipeline
