# Server/Client Architecture

This document describes the architectural split between server and client components, enabling multi-platform client support through the gRPC v2 protocol.

## Overview

Reovim follows a **data/presentation separation** model:

- **Server**: Provides raw data (buffer content, syntax tokens, diagnostics)
- **Client**: Handles all presentation (colors, layout, rendering)

This enables a single server to support multiple client platforms:

```
                    ┌─────────────────────┐
                    │   reovim-server     │
                    │   (gRPC v2)         │
                    │                     │
                    │ • Buffer management │
                    │ • Syntax parsing    │
                    │ • LSP integration   │
                    │ • Mode/state        │
                    └─────────┬───────────┘
                              │
              ┌───────────────┼───────────────┐
              │               │               │
              ▼               ▼               ▼
      ┌───────────┐   ┌───────────┐   ┌───────────┐
      │    TUI    │   │  Android  │   │    Web    │
      │  (Rust)   │   │  (Kotlin) │   │   (TS)    │
      └───────────┘   └───────────┘   └───────────┘
```

## Protocol Evolution: v1 vs v2

### v1: Cell-Grid Model (JSON-RPC)

```
┌─────────────────────────────────────────────────────────────┐
│  Server (does everything)                                   │
│  1. Parse syntax (treesitter)                               │
│  2. Apply theme colors                                      │
│  3. Render cell grid with positions + colors                │
│  4. Send pre-rendered cells to client                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Client (dumb terminal)                                     │
│  Just displays the cell grid it receives                    │
└─────────────────────────────────────────────────────────────┘
```

**Problems with v1:**
- Server must know client's terminal size
- Theme is server-side (can't have platform-native themes)
- Tight coupling prevents alternative clients
- Every client redraws entire grid on every change

### v2: Raw-Data Model (gRPC)

```
┌─────────────────────────────────────────────────────────────┐
│  Server (data provider)                                     │
│  • Raw buffer lines                                         │
│  • Syntax tokens (spans + types, NOT colors)                │
│  • Cursor position, mode, registers                         │
│  • Diagnostics (positions + messages)                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼  gRPC streaming
┌─────────────────────────────────────────────────────────────┐
│  Client (smart renderer)                                    │
│  • Applies its own theme (tokens → colors)                  │
│  • Handles its own layout (terminal size is local)          │
│  • Platform-native UI (TUI, Android Views, Web DOM)         │
└─────────────────────────────────────────────────────────────┘
```

**Benefits of v2:**
- Server is platform-agnostic
- Clients can use native themes and UI patterns
- Efficient updates via streaming (only changed data)
- Clean separation enables testing and alternative clients

## Server Responsibilities

The server handles **computation and state**:

| Subsystem | Server Provides | Notes |
|-----------|-----------------|-------|
| **Buffer** | Raw lines, line count | No formatting |
| **Syntax** | Token spans + types | `keyword`, `string`, not colors |
| **LSP** | Diagnostics, completions, hover | Positions + text |
| **Mode** | Current mode name | `Normal`, `Insert`, etc. |
| **Cursor** | Position (line, column) | Per-window |
| **Registers** | Register contents | Yank/paste data |
| **Marks** | Mark positions | Named positions |
| **Search** | Match positions | Highlight ranges |
| **Folding** | Foldable ranges | Start/end lines |
| **Git** | Changed line ranges | Diff hunks |

**Server does NOT handle:**
- Terminal dimensions (client's concern)
- Colors or themes (client applies)
- Layout/splits (client renders)
- Gutter rendering (client draws)
- Statusline content (client formats)

## Client Responsibilities

The client handles **presentation and interaction**:

| Subsystem | Client Handles | Notes |
|-----------|----------------|-------|
| **Theme** | Token type → color mapping | Platform-native themes |
| **Layout** | Window splits, tabs | Client's terminal/screen size |
| **Gutter** | Line numbers, signs, folds | Client draws all decorations |
| **Statusline** | Mode display, file info | Client formats and renders |
| **Input** | Key capture, translation | Sends vim notation to server |
| **Rendering** | Final pixel/cell output | Platform-specific |

## gRPC v2 Services

### Implemented Services

| Service | Methods | Purpose |
|---------|---------|---------|
| `BufferService` | `GetRawContent`, `List`, `GetLineCount` | Raw buffer data |
| `InputService` | `SendKeys` | Key input handling |
| `StateService` | `GetMode`, `GetCursor` | Editor state queries |
| `ServerService` | `Ping`, `Info`, `Quit` | Server management |
| `NotificationService` | `Subscribe` (streaming) | Real-time updates |

### Planned Services

| Service | Methods | Purpose |
|---------|---------|---------|
| `ViewportService` | `Create`, `Close`, `Focus`, `Get` | Multi-viewport management |
| `SyntaxService` | `GetTokens`, `StreamTokens` | Syntax token data |
| `OverlayService` | `Show`, `Hide`, `Interact`, `Stream` | Shared overlay state |
| `LayoutService` | `Get`, `Set`, `Stream`, `Save`, `Restore` | Logical layout sharing |
| `PresenceService` | `Join`, `Leave`, `Stream`, `Update` | Multi-client awareness |
| `DiagnosticService` | `GetDiagnostics`, `StreamDiagnostics` | LSP diagnostics |
| `CompletionService` | `GetCompletions` | Completion items |
| `FoldingService` | `GetFoldRanges` | Fold markers |

### Removed from Protocol

| Service/Method | Reason |
|----------------|--------|
| `EditorService.Resize` | Client handles its own layout |
| Cell-grid rendering | Server provides data, not presentation |

## ViewportService Design

A **Viewport** is the server-side concept of a view into a buffer. Each viewport tracks:
- Which buffer it displays
- Cursor position within that buffer
- Scroll offset (top visible line)

Clients create viewports when splitting windows. Multiple viewports can view the same buffer with independent cursors.

### Protocol

```protobuf
service ViewportService {
  // Create a new viewport for a buffer
  rpc Create(CreateViewportRequest) returns (CreateViewportResponse);

  // Close a viewport
  rpc Close(CloseViewportRequest) returns (CloseViewportResponse);

  // Set the focused viewport (input goes here)
  rpc Focus(FocusViewportRequest) returns (FocusViewportResponse);

  // Get viewport state
  rpc Get(GetViewportRequest) returns (GetViewportResponse);

  // Stream viewport updates (cursor moves, scroll changes)
  rpc Stream(StreamViewportRequest) returns (stream ViewportUpdate);
}

message CreateViewportRequest {
  uint64 buffer_id = 1;
  optional Position initial_cursor = 2;
}

message CreateViewportResponse {
  uint64 viewport_id = 1;
}

message CloseViewportRequest {
  uint64 viewport_id = 1;
}

message FocusViewportRequest {
  uint64 viewport_id = 1;
}

message GetViewportRequest {
  uint64 viewport_id = 1;
}

message GetViewportResponse {
  uint64 viewport_id = 1;
  uint64 buffer_id = 2;
  Position cursor = 3;
  uint32 scroll_top = 4;  // Top visible line
}

message ViewportUpdate {
  uint64 viewport_id = 1;
  oneof update {
    Position cursor_moved = 2;
    uint32 scroll_changed = 3;
    uint64 buffer_changed = 4;  // Viewport now shows different buffer
  }
}
```

### Viewport Lifecycle

```
1. Client starts with default viewport (id=0)
   │
   ▼
2. User: <C-w>v (vertical split)
   │
   ├─► Client: ViewportService.Create(buffer_id=current)
   │   └─► Server: Returns viewport_id=1
   │
   ├─► Client: Creates new window panel for viewport 1
   │
   └─► Client: ViewportService.Focus(viewport_id=1)
       └─► Server: Routes future InputService.SendKeys to viewport 1
   │
   ▼
3. User edits in new viewport
   │
   ├─► Server: Moves cursor in viewport 1
   │
   └─► Server: Broadcasts ViewportUpdate { cursor_moved } to all clients
       └─► Other clients see the cursor move
   │
   ▼
4. User: <C-w>c (close window)
   │
   ├─► Client: ViewportService.Close(viewport_id=1)
   │
   └─► Client: Removes window panel, focuses remaining viewport
```

### Key Concepts

| Concept | Server (Viewport) | Client (Window) |
|---------|-------------------|-----------------|
| **Identity** | `viewport_id` | `window_id` (local) |
| **Buffer** | Which buffer to show | — |
| **Cursor** | Position in buffer | Rendered cursor glyph |
| **Scroll** | Top visible line | Scroll offset in pixels/rows |
| **Size** | — | Width × Height |
| **Position** | — | Screen coordinates |

The server tracks **what** to show (viewport), the client decides **how** to show it (window).

## SyntaxService Design

The SyntaxService is the key to platform-independent syntax highlighting:

```protobuf
service SyntaxService {
  // Get syntax tokens for a buffer region
  rpc GetTokens(GetTokensRequest) returns (GetTokensResponse);

  // Stream token updates when buffer changes
  rpc StreamTokens(StreamTokensRequest) returns (stream TokenUpdate);
}

message GetTokensRequest {
  optional uint64 buffer_id = 1;
  optional uint32 start_line = 2;
  optional uint32 end_line = 3;
}

message GetTokensResponse {
  repeated SyntaxToken tokens = 1;
}

message SyntaxToken {
  uint32 start_line = 1;
  uint32 start_col = 2;
  uint32 end_line = 3;
  uint32 end_col = 4;
  string token_type = 5;  // "keyword", "string", "comment", etc.
  optional string scope = 6;  // For nested scopes: "string.quoted.double"
}

message TokenUpdate {
  uint64 buffer_id = 1;
  uint32 start_line = 2;
  uint32 end_line = 3;
  repeated SyntaxToken tokens = 4;
}
```

### Token Types (Semantic)

Standard token types that all clients understand:

| Token Type | Examples | Typical Color |
|------------|----------|---------------|
| `keyword` | `fn`, `if`, `return` | Purple/Blue |
| `keyword.control` | `if`, `else`, `match` | Purple |
| `keyword.function` | `fn`, `async` | Blue |
| `type` | `String`, `Vec` | Yellow |
| `type.builtin` | `i32`, `bool` | Yellow |
| `function` | `main`, `println` | Blue |
| `function.builtin` | `println!` | Cyan |
| `variable` | `foo`, `bar` | White |
| `variable.parameter` | Function parameters | Orange |
| `constant` | `PI`, `MAX_SIZE` | Orange |
| `string` | `"hello"` | Green |
| `string.escape` | `\n`, `\t` | Cyan |
| `number` | `42`, `3.14` | Orange |
| `comment` | `// comment` | Gray |
| `comment.doc` | `/// doc comment` | Gray italic |
| `operator` | `+`, `-`, `=` | White |
| `punctuation` | `{`, `}`, `;` | White |
| `attribute` | `#[derive]` | Yellow |
| `namespace` | `std::io` | White |

## Client Theme Engine

Each client implements a theme engine that maps token types to platform-native styles:

### TUI Theme (256 colors)

```rust
pub struct TuiTheme {
    mappings: HashMap<&'static str, Style>,
}

impl TuiTheme {
    pub fn style_for_token(&self, token_type: &str) -> Style {
        // Try exact match first
        if let Some(style) = self.mappings.get(token_type) {
            return *style;
        }
        // Fall back to parent scope: "keyword.control" → "keyword"
        if let Some(dot) = token_type.rfind('.') {
            return self.style_for_token(&token_type[..dot]);
        }
        // Default
        Style::default()
    }
}
```

### Android Theme (Material)

```kotlin
class AndroidTheme(private val context: Context) {
    private val mappings: Map<String, TextStyle>

    fun styleForToken(tokenType: String): TextStyle {
        return mappings[tokenType]
            ?: mappings[tokenType.substringBeforeLast('.')]
            ?: TextStyle.Default
    }
}
```

### Web Theme (CSS classes)

```typescript
interface WebTheme {
  tokenStyles: Record<string, string>;  // token type → CSS class
}

function getTokenClass(tokenType: string, theme: WebTheme): string {
  return theme.tokenStyles[tokenType]
    ?? theme.tokenStyles[tokenType.split('.').slice(0, -1).join('.')]
    ?? 'token-default';
}
```

## Data Flow Example

### Syntax Highlighting Flow

```
1. User types character
   │
   ▼
2. TUI sends: InputService.SendKeys("a")
   │
   ▼
3. Server:
   - Inserts character into buffer
   - Re-parses affected region (treesitter)
   - Emits notification with updated tokens
   │
   ▼
4. NotificationService streams: TokenUpdate {
     buffer_id: 1,
     start_line: 5,
     end_line: 5,
     tokens: [
       { start: (5,0), end: (5,2), type: "keyword" },
       { start: (5,3), end: (5,7), type: "function" },
     ]
   }
   │
   ▼
5. TUI client:
   - Receives token update
   - Maps "keyword" → Purple (via theme)
   - Maps "function" → Blue (via theme)
   - Re-renders line 5 with new colors
```

### Mode Change Flow

```
1. User presses Escape
   │
   ▼
2. TUI sends: InputService.SendKeys("<Esc>")
   │
   ▼
3. Server:
   - Changes mode to Normal
   - Emits mode_changed notification
   │
   ▼
4. NotificationService streams: Notification {
     event_type: "mode_changed",
     mode_changed: { mode: "Normal", display: "NORMAL" }
   }
   │
   ▼
5. TUI client:
   - Updates statusline with "NORMAL"
   - Changes cursor style (block vs line)
```

## Common Client Model

All clients (TUI, Android, Web) share a common abstraction layer that handles:
- **Panel**: Content regions (editor windows)
- **Overlay**: Floating elements (popups, menus, completions)
- **Layout**: Spatial arrangement (splits, tabs)
- **Focus**: Input routing and z-order
- **Sync**: Multi-client presence and following

### Architecture

```
┌────────────────────────────────────────────────────────────┐
│  Platform Layer                                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │     TUI     │  │   Android   │  │     Web     │         │
│  │  (crossterm)│  │  (Compose)  │  │    (DOM)    │         │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘         │
│         │ impl           │ impl           │ impl           │
└─────────┼────────────────┼────────────────┼────────────────┘
          │                │                │
          ▼                ▼                ▼
┌────────────────────────────────────────────────────────────┐
│  Common Client Model (lib/clients/model/)                  │
│                                                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │   Panel     │  │   Overlay   │  │   Layout    │         │
│  │  (content)  │  │  (popups)   │  │  (splits)   │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
│                                                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │   Focus     │  │  Viewport   │  │  Presence   │         │
│  │  (z-order)  │  │  (server)   │  │   (sync)    │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
└────────────────────────────────────────────────────────────┘
                              │
                              ▼ gRPC
┌────────────────────────────────────────────────────────────┐
│  Server                                                    │
└────────────────────────────────────────────────────────────┘
```

### Core Abstractions

The Common Client Model separates **wire format** (from server) from **rendered state** (client-side).

```
┌─────────────────────────────────────────────────────────────┐
│  Wire Format (from server via gRPC)                         │
│  • LogicalOverlay { kind, data: Value, state }              │
│  • LogicalLayout { splits, tabs, ratios }                   │
│  • ViewportUpdate { cursor, scroll }                        │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼ Client interprets
┌─────────────────────────────────────────────────────────────┐
│  Rendered State (client-side)                               │
│  • RenderedOverlay { position, size, content }              │
│  • WindowTree { windows with screen positions }             │
│  • Panel { viewport_id, visible_range }                     │
└─────────────────────────────────────────────────────────────┘
```

#### Panel Trait

```rust
/// A rectangular content region displaying a buffer
pub trait Panel {
    /// Buffer being displayed
    fn buffer_id(&self) -> BufferId;

    /// Server-side viewport for cursor tracking
    fn viewport_id(&self) -> ViewportId;

    /// Currently visible line range
    fn visible_range(&self) -> LineRange;

    /// Scroll to show a line
    fn scroll_to(&mut self, line: u32);
}
```

#### Layout Trait

```rust
/// Container that arranges panels spatially
pub trait Layout {
    /// Create a new split (requests viewport from server)
    fn split(&mut self, direction: Direction) -> ViewportId;

    /// Close a panel
    fn close(&mut self, viewport_id: ViewportId);

    /// Move focus to a panel
    fn focus(&mut self, viewport_id: ViewportId);

    /// Navigate focus in direction
    fn focus_direction(&mut self, direction: Direction);

    /// Resize a panel
    fn resize(&mut self, viewport_id: ViewportId, delta: i32);

    /// Get logical layout (for sharing with server)
    fn to_logical(&self) -> LogicalLayout;

    /// Apply logical layout (from server)
    fn from_logical(&mut self, layout: &LogicalLayout);
}
```

#### Overlay Rendering (Trait-based)

Overlays use a **renderer registry** pattern, not a concrete content type:

```rust
/// Wire format from server (generic)
pub struct LogicalOverlay {
    pub id: String,
    pub anchor: Anchor,
    pub kind: String,       // "completion", "hover", etc.
    pub data: Value,        // Generic payload
    pub state: OverlayState,
}

/// Client-side rendered overlay (has screen position)
pub struct RenderedOverlay {
    pub id: String,
    pub position: ScreenPosition,  // Calculated from anchor
    pub size: Size,
    pub z_index: u32,
    pub is_modal: bool,
}

/// Trait for rendering overlays - clients register per-kind renderers
pub trait OverlayRenderer: Send + Sync {
    fn handles(&self, kind: &str) -> bool;
    fn render(&self, overlay: &LogicalOverlay, ctx: &mut RenderContext) -> RenderedOverlay;
    fn interact(&self, overlay: &LogicalOverlay, interaction: Interaction) -> Option<StateUpdate>;
}

/// Manages overlay stack and renderer dispatch
pub trait OverlayManager {
    /// Register a renderer for overlay kinds
    fn register_renderer(&mut self, renderer: Box<dyn OverlayRenderer>);

    /// Show overlay (from server or local)
    fn show(&mut self, overlay: LogicalOverlay);

    /// Hide overlay by id
    fn hide(&mut self, id: &str);

    /// Get active overlays (for rendering)
    fn active(&self) -> &[RenderedOverlay];

    /// Check if any overlay captures input
    fn has_modal(&self) -> bool;

    /// Route interaction to appropriate renderer
    fn interact(&mut self, interaction: Interaction) -> Option<StateUpdate>;
}
```

#### Focus Management

```rust
/// Focus can be on a panel or an overlay
pub enum Focus {
    Panel(ViewportId),
    Overlay(String),  // overlay id
}

/// Focus management for panels and overlays
pub trait FocusManager {
    /// Get current focus
    fn current(&self) -> Focus;

    /// Focus a panel
    fn focus_panel(&mut self, viewport_id: ViewportId);

    /// Focus an overlay (when shown)
    fn focus_overlay(&mut self, overlay_id: &str);

    /// Return focus to panel (when overlay dismissed)
    fn return_to_panel(&mut self);
}
```

### Layout Key Handling

Layout keys (`<C-w>` commands) are handled client-side:

```
User: <C-w>v (vertical split)
  │
  ├─► Client intercepts (recognized as layout key)
  │
  ├─► Client: ViewportService.CreateViewport(buffer_id)
  │   └─► Server: Returns viewport_id=42
  │
  ├─► Client: layout.split(Direction::Vertical)
  │   └─► New panel created with viewport_id=42
  │
  └─► Client: ViewportService.FocusViewport(42)
      └─► Server: Routes future keys to viewport 42
```

| Key | Client Handles | Server Notified |
|-----|----------------|-----------------|
| `<C-w>s` | Split horizontal | CreateViewport |
| `<C-w>v` | Split vertical | CreateViewport |
| `<C-w>c` | Close window | CloseViewport |
| `<C-w>h/j/k/l` | Focus movement | FocusViewport |
| `<C-w>=` | Equalize sizes | — |
| `<C-w>+/-` | Resize | — |

### Viewport vs Window

- **Viewport** (server-side): Logical view into a buffer with cursor position
- **Window** (client-side): Visual region on screen with position and size

```
Server:                          Client (TUI):
┌──────────────────┐             ┌──────────────────┐
│ Viewport 1       │             │ Window 1 (50x20) │
│ buffer: 1        │ ◄────────── │ viewport: 1      │
│ cursor: (10, 5)  │             │ position: (0, 0) │
└──────────────────┘             └──────────────────┘
┌──────────────────┐             ┌──────────────────┐
│ Viewport 2       │             │ Window 2 (50x20) │
│ buffer: 1        │ ◄────────── │ viewport: 2      │
│ cursor: (50, 0)  │             │ position: (51, 0)│
└──────────────────┘             └──────────────────┘

Same buffer, two viewports with different cursors.
Client arranges windows; server tracks cursors.
```

## Multi-Client Presence

Multiple clients can connect to the same session and see each other.

### PresenceService Protocol

```protobuf
service PresenceService {
  // Announce this client to the session
  rpc Join(JoinRequest) returns (JoinResponse);

  // Leave the session
  rpc Leave(LeaveRequest) returns (LeaveResponse);

  // Stream other clients' presence updates
  rpc StreamPresence(StreamPresenceRequest) returns (stream PresenceUpdate);

  // Update my cursor/viewport (broadcast to others)
  rpc UpdatePresence(UpdatePresenceRequest) returns (UpdatePresenceResponse);

  // Set sync mode (independent, follow, present)
  rpc SetSyncMode(SetSyncModeRequest) returns (SetSyncModeResponse);
}

message JoinRequest {
  string client_type = 1;   // "tui", "android", "web"
  string display_name = 2;  // "laptop", "phone", "tablet"
}

message ClientPresence {
  string client_id = 1;
  string client_type = 2;
  string display_name = 3;

  // Current state
  uint64 buffer_id = 4;
  Position cursor = 5;
  LineRange visible_lines = 6;
  string mode = 7;

  // Sync mode
  SyncMode sync_mode = 8;
}

message PresenceUpdate {
  oneof update {
    ClientPresence joined = 1;
    ClientPresence updated = 2;
    string left = 3;  // client_id that left
  }
}

enum SyncMode {
  INDEPENDENT = 0;  // Own cursor, own scroll
  FOLLOW = 1;       // Follow another client
  PRESENT = 2;      // I'm presenting, others can follow me
}
```

### Sync Levels

| Level | What Syncs | Use Case |
|-------|------------|----------|
| **Buffer only** | Content changes | Independent editing |
| **+ Mode** | Content + INSERT/NORMAL | See other's mode |
| **+ Cursor** | Content + cursor position | Pair programming |
| **+ Scroll** | Content + viewport | Presentation mode |

### Visual: Multi-Client Cursors

**TUI sees Android's cursor:**
```
  1 │ fn main() {
  2 │ █   println!("Hello");    ← TUI cursor (block)
  3 │ ┆   let x = 42;           ← Android cursor (dotted, blue)
  4 │ }
      ─────────────────────────
      📱 phone connected (line 3)
```

**Android sees TUI's cursor:**
```
┌─────────────────────────────┐
│ fn main() {                 │
│     println!("Hello"); █    │ ← TUI cursor (badge)
│     let x = 42;│            │ ← My cursor
│ }                           │
├─────────────────────────────┤
│ 💻 laptop (line 2)          │
└─────────────────────────────┘
```

### Follow Mode Flow

```
1. Android: SetSyncMode(FOLLOW, target="tui-laptop")
   │
   ▼
2. Server: Marks Android as following TUI
   │
   ▼
3. TUI moves cursor to line 100
   │
   ▼
4. Server: Broadcasts PresenceUpdate to Android
   │
   ▼
5. Android client: Scrolls to line 100, shows TUI cursor
```

## Layout Sharing

Clients can share **logical layout** (intent) while having **physical layout** (pixels) be platform-specific.

### Logical vs Physical Layout

```
TUI (150x40 terminal):           Android (360x800 phone):
┌─────────┬─────────┐            ┌──────────────┐
│ file.rs │ test.rs │            │   [tabs]     │
│         │         │            │ file | test  │
│         │         │            ├──────────────┤
└─────────┴─────────┘            │              │
                                 │   file.rs    │
Same logical layout:             │              │
"2 files side-by-side"           └──────────────┘
Different physical rendering
```

### Logical Layout Structure

```rust
/// Platform-agnostic layout description
pub enum LogicalLayout {
    /// Single panel showing a buffer
    Single {
        buffer_id: BufferId,
        viewport_id: ViewportId,
    },

    /// Split container
    Split {
        direction: Direction,      // Horizontal or Vertical
        children: Vec<LogicalLayout>,
        ratios: Vec<f32>,          // Relative sizes [0.5, 0.5]
    },

    /// Tab container
    Tabs {
        tabs: Vec<LogicalLayout>,
        active: usize,
    },
}
```

### LayoutService Protocol

```protobuf
service LayoutService {
  // Get current logical layout for session
  rpc GetLayout(GetLayoutRequest) returns (GetLayoutResponse);

  // Update layout (broadcasts to other clients)
  rpc SetLayout(SetLayoutRequest) returns (SetLayoutResponse);

  // Stream layout changes from other clients
  rpc StreamLayout(StreamLayoutRequest) returns (stream LayoutUpdate);

  // Save layout to session (persisted across reconnects)
  rpc SaveLayout(SaveLayoutRequest) returns (SaveLayoutResponse);

  // Restore saved layout
  rpc RestoreLayout(RestoreLayoutRequest) returns (RestoreLayoutResponse);
}

message LogicalLayout {
  oneof layout {
    SinglePanel single = 1;
    SplitLayout split = 2;
    TabLayout tabs = 3;
  }
}

message SinglePanel {
  uint64 buffer_id = 1;
  uint64 viewport_id = 2;
}

message SplitLayout {
  Direction direction = 1;
  repeated LogicalLayout children = 2;
  repeated float ratios = 3;
}

message TabLayout {
  repeated LogicalLayout tabs = 1;
  uint32 active_tab = 2;
}

message LayoutUpdate {
  string source_client_id = 1;
  LogicalLayout layout = 2;
  LayoutChangeType change_type = 3;
}

enum LayoutChangeType {
  FULL_REPLACE = 0;
  SPLIT_ADDED = 1;
  SPLIT_REMOVED = 2;
  TAB_ADDED = 3;
  TAB_REMOVED = 4;
  FOCUS_CHANGED = 5;
}
```

### Layout Sync Modes

```rust
pub enum LayoutSyncMode {
    /// My layout is independent (default)
    Independent,

    /// Share my layout changes with session
    Broadcast,

    /// Follow another client's layout
    Follow { target_client_id: String },

    /// Accept layout changes from session (but don't broadcast mine)
    Accept,
}
```

### Platform Interpretation

Each client interprets logical layout for its platform:

```rust
pub trait LayoutInterpreter {
    /// Convert logical layout to platform-specific layout
    fn interpret(&self, logical: &LogicalLayout, screen: ScreenSize) -> PlatformLayout;

    /// Convert platform action back to logical change
    fn to_logical(&self, action: PlatformAction) -> LogicalLayoutChange;
}
```

**TUI Interpreter** (direct mapping):
```rust
impl LayoutInterpreter for TuiInterpreter {
    fn interpret(&self, logical: &LogicalLayout, screen: ScreenSize) -> PlatformLayout {
        match logical {
            LogicalLayout::Split { direction, children, ratios } => {
                // Direct terminal splits
                PlatformLayout::Split {
                    direction: *direction,
                    children: children.iter().map(|c| self.interpret(c, screen)).collect(),
                    sizes: calculate_sizes(ratios, screen),
                }
            }
            LogicalLayout::Tabs { tabs, active } => {
                // TUI can show tabs as statusline indicators
                PlatformLayout::Tabs { ... }
            }
            LogicalLayout::Single { buffer_id, viewport_id } => {
                PlatformLayout::Single { buffer_id, viewport_id }
            }
        }
    }
}
```

**Android Interpreter** (adaptive):
```rust
impl LayoutInterpreter for AndroidInterpreter {
    fn interpret(&self, logical: &LogicalLayout, screen: ScreenSize) -> PlatformLayout {
        match logical {
            LogicalLayout::Split { children, .. } if screen.width < 600 => {
                // Small screen: convert splits to tabs
                PlatformLayout::Tabs {
                    tabs: children.iter().map(|c| self.interpret(c, screen)).collect(),
                    active: 0,
                }
            }
            LogicalLayout::Split { direction, children, ratios } if screen.is_landscape() => {
                // Tablet landscape: actual splits
                PlatformLayout::Split { ... }
            }
            _ => { ... }
        }
    }
}
```

### Layout Sync Flow

```
1. TUI: SetLayoutSyncMode(Broadcast)
   Android: SetLayoutSyncMode(Follow { target: "tui" })
   │
   ▼
2. TUI: User presses <C-w>v (vertical split)
   │
   ▼
3. TUI: LayoutService.SetLayout(
     Split {
       direction: Vertical,
       children: [Single(buf_1), Single(buf_2)],
       ratios: [0.5, 0.5]
     }
   )
   │
   ▼
4. Server: Stores layout, broadcasts LayoutUpdate to followers
   │
   ▼
5. Android receives: LayoutUpdate { layout: Split {...} }
   │
   ▼
6. Android: AndroidInterpreter.interpret(layout, screen)
   │
   ├─► Phone (portrait): Shows as tabs [buf_1 | buf_2]
   └─► Tablet (landscape): Shows as vertical split
```

### Session Layout Persistence

Layouts are saved per-session for restore on reconnect:

```
User opens project "reovim":
  │
  ▼
Client: LayoutService.RestoreLayout(session_id="reovim")
  │
  ▼
Server: Returns last saved LogicalLayout
  │
  ▼
Client: Interprets and displays
  │
  ▼
User's window arrangement is restored!
```

### Summary: What's Shared

| Aspect | Where | Shared? |
|--------|-------|---------|
| **Logical layout** (structure) | Server | ✅ Optional |
| **Physical layout** (pixels) | Client | ❌ No |
| **Layout interpretation** | Client | ❌ No |
| **Saved layouts** | Server | ✅ Per-session |
| **Sync mode** | Client choice | ✅ Configurable |

## Logical Overlay Sharing

Overlays (popups, completions, menus) can be shared across clients using a **generic data model** at the protocol level and **traits** at the client level.

### Design Principle

| Layer | What | How |
|-------|------|-----|
| **Protocol** | Generic data | `kind: String` + `data: Value` |
| **Convention** | Documented shapes | "completion" kind expects `{ items: [...] }` |
| **Client** | Traits/interfaces | Renderers handle specific kinds |

This is like HTTP `Content-Type` + body - protocol doesn't interpret, clients do.

### Protocol Level (Wire Format)

```rust
/// Wire format - server doesn't interpret content
pub struct LogicalOverlay {
    pub id: String,
    pub anchor: Anchor,
    pub kind: String,       // "completion", "hover", "plugin.custom"
    pub data: Value,        // Generic JSON / protobuf Any
    pub state: OverlayState,
}

pub enum Anchor {
    /// Anchored to buffer position
    Buffer { buffer_id: u64, line: u32, col: u32 },
    /// Follows cursor
    Cursor,
    /// Screen-relative
    Screen { x: f32, y: f32 },
}

pub struct OverlayState {
    pub selected: Option<u32>,
    pub filter: Option<String>,
    pub scroll: u32,
    pub custom: HashMap<String, Value>,
}
```

```protobuf
// Protocol buffer definition
message LogicalOverlay {
  string id = 1;
  Anchor anchor = 2;
  string kind = 3;
  google.protobuf.Any data = 4;  // Generic payload
  OverlayState state = 5;
}

service OverlayService {
  rpc Show(ShowOverlayRequest) returns (ShowOverlayResponse);
  rpc Hide(HideOverlayRequest) returns (HideOverlayResponse);
  rpc Interact(InteractRequest) returns (InteractResponse);
  rpc Stream(StreamRequest) returns (stream OverlayUpdate);
}
```

### Convention Level (Documented Shapes)

Conventions are **documented**, not enforced by protocol:

```
kind: "completion"
data: {
  "items": [
    { "id": "1", "label": "from_utf8", "detail": "fn(...)", "icon": "function" }
  ]
}

kind: "hover"
data: {
  "content": "# String::from_utf8\n\nConverts bytes to string..."
}

kind: "signature"
data: {
  "signatures": [...],
  "active": 0,
  "active_param": 1
}

kind: "my-plugin.custom"
data: { ... }  // Only my-plugin understands this
```

### Client Level (Traits)

```rust
/// Trait for overlay renderers - clients implement per-kind
pub trait OverlayRenderer: Send + Sync {
    /// Which overlay kinds can this renderer handle?
    fn handles(&self, kind: &str) -> bool;

    /// Render overlay to platform output
    fn render(&self, overlay: &LogicalOverlay, ctx: &mut RenderContext);

    /// Handle interaction, return state update if needed
    fn interact(
        &self,
        overlay: &LogicalOverlay,
        interaction: Interaction,
    ) -> Option<StateUpdate>;
}

/// Trait for parsing list-like content from overlay data
pub trait Listable {
    fn from_data(data: &Value) -> Option<Self> where Self: Sized;
    fn items(&self) -> &[ListItem];
    fn selected(&self) -> Option<usize>;
}

/// Trait for parsing document-like content from overlay data
pub trait Documentable {
    fn from_data(data: &Value) -> Option<Self> where Self: Sized;
    fn as_markdown(&self) -> Option<&str>;
    fn as_plain(&self) -> &str;
}

/// Platform-agnostic interaction (client translates input to this)
pub enum Interaction {
    SelectNext,
    SelectPrev,
    SelectIndex(u32),
    Filter(String),
    Confirm,
    Cancel,
    Custom(String),
}
```

### Client Registration

```rust
impl TuiClient {
    fn setup_renderers(&mut self) {
        // Built-in renderers for common kinds
        self.register_renderer(Box::new(CompletionRenderer::new()));
        self.register_renderer(Box::new(HoverRenderer::new()));
        self.register_renderer(Box::new(SignatureRenderer::new()));

        // Fallback for unknown kinds (shows raw data)
        self.register_renderer(Box::new(FallbackRenderer::new()));
    }
}

impl OverlayRenderer for CompletionRenderer {
    fn handles(&self, kind: &str) -> bool {
        kind == "completion"
    }

    fn render(&self, overlay: &LogicalOverlay, ctx: &mut RenderContext) {
        // Parse data according to completion convention
        if let Some(items) = overlay.data.get("items").and_then(|v| v.as_array()) {
            // Render terminal popup with items...
        }
    }
}
```

### Platform Input Mapping

Each platform maps native input to abstract `Interaction`:

| Platform | Input | Interaction |
|----------|-------|-------------|
| TUI | `<C-n>`, `↓` | `SelectNext` |
| TUI | `<CR>` | `Confirm` |
| Android | Tap item | `SelectIndex(n)` |
| Android | Swipe down | `Cancel` |
| Web | ArrowDown | `SelectNext` |
| Web | Click item | `SelectIndex(n)` |

### Sync Flow

```
TUI                         Server                      Android
 │                             │                            │
 │ Show("completion", data)    │                            │
 │ ────────────────────────►   │                            │
 │                             │  OverlayUpdate::Shown      │
 │ ◄────────────────────────   │  ──────────────────────►   │
 │ [CompletionRenderer]        │  [CompletionRenderer]      │
 │                             │                            │
 │ Interact(SelectNext)        │                            │
 │ ────────────────────────►   │                            │
 │                             │  OverlayUpdate::Changed    │
 │ ◄────────────────────────   │  ──────────────────────►   │
 │ [update selection]          │  [update selection]        │
 │                             │                            │
 │                             │  Interact(SelectIndex(2))  │
 │                             │  ◄──────────────────────   │
 │  OverlayUpdate::Changed     │                            │
 │ ◄────────────────────────   │  ──────────────────────►   │
 │ [jump to item 2]            │  [highlight item 2]        │
```

### Why This Design

| Benefit | How |
|---------|-----|
| **Extensible** | New overlay kinds without protocol changes |
| **Platform-native** | Each client renders with its own UI conventions |
| **Plugin-friendly** | Plugins define custom kinds, custom renderers |
| **Decoupled** | Server is a dumb relay, clients have all rendering logic |
| **Testable** | Renderers can be unit tested with mock data |

## Input Routing

Input routing depends on focus state (panel vs overlay) and whether the overlay is modal.

### Routing Flow

```
User Input (key press)
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  Client: FocusManager.current()                             │
└─────────────────────────────────────────────────────────────┘
    │
    ├─► Focus::Panel(viewport_id)
    │   │
    │   └─► InputService.SendKeys(keys, viewport_id)
    │       └─► Server handles, broadcasts state changes
    │
    └─► Focus::Overlay(overlay_id)
        │
        └─► Client: OverlayManager.interact(interaction)
            │
            ├─► Renderer handles locally (navigation)
            │   └─► OverlayService.Interact(overlay_id, state_change)
            │       └─► Server broadcasts to other clients
            │
            └─► Renderer returns Confirm/Cancel
                │
                ├─► Confirm: Client sends result to server
                │   └─► InputService.SendKeys (completion text)
                │   └─► OverlayService.Hide(overlay_id)
                │
                └─► Cancel: OverlayService.Hide(overlay_id)
                    └─► FocusManager.return_to_panel()
```

### Modal vs Non-Modal

| Type | Input Behavior |
|------|----------------|
| **Modal** | All input goes to overlay until dismissed |
| **Non-modal** | Overlay keys intercepted, others pass through to panel |

```rust
impl Client {
    fn handle_input(&mut self, key: Key) {
        match self.focus.current() {
            Focus::Panel(viewport_id) => {
                // Check if key triggers an overlay (e.g., <C-Space>)
                if let Some(overlay) = self.check_overlay_trigger(&key) {
                    self.overlay_manager.show(overlay);
                    self.focus.focus_overlay(&overlay.id);
                } else {
                    // Send to server
                    self.grpc.send_keys(&key.to_vim_notation(), viewport_id);
                }
            }
            Focus::Overlay(ref overlay_id) => {
                let interaction = self.key_to_interaction(&key);

                if let Some(result) = self.overlay_manager.interact(interaction) {
                    match result {
                        InteractionResult::StateUpdate(update) => {
                            // Broadcast state change
                            self.grpc.overlay_interact(overlay_id, update);
                        }
                        InteractionResult::Confirm(value) => {
                            // Apply result (e.g., insert completion)
                            self.apply_overlay_result(value);
                            self.overlay_manager.hide(overlay_id);
                            self.focus.return_to_panel();
                        }
                        InteractionResult::Cancel => {
                            self.overlay_manager.hide(overlay_id);
                            self.grpc.overlay_hide(overlay_id);
                            self.focus.return_to_panel();
                        }
                        InteractionResult::PassThrough => {
                            // Non-modal: key goes to panel
                            if let Focus::Panel(vp) = self.focus.current() {
                                self.grpc.send_keys(&key.to_vim_notation(), vp);
                            }
                        }
                    }
                }
            }
        }
    }
}
```

### Overlay Trigger Sources

Overlays can be triggered from multiple sources:

| Source | Example | Who Shows |
|--------|---------|-----------|
| **Client key** | `<C-Space>` → completion | Client requests, server provides data |
| **Server event** | LSP completion auto-trigger | Server pushes, client shows |
| **Plugin** | Custom popup | Plugin requests via server |

```
Client-triggered:
  TUI: <C-Space>
    │
    └─► Client: CompletionService.GetCompletions(position)
        │
        └─► Server: Returns items
            │
            └─► Client: OverlayService.Show("completion", items)
                │
                └─► All clients show completion popup

Server-triggered:
  Server: LSP sends completion (auto-triggered by typing)
    │
    └─► Server: OverlayService broadcasts Show("completion", items)
        │
        └─► All clients show completion popup
```

## Crate Structure

```
lib/
├── server/                    # Server crate
│   ├── src/
│   │   ├── grpc/              # gRPC service implementations
│   │   │   ├── buffer.rs
│   │   │   ├── input.rs
│   │   │   ├── state.rs
│   │   │   ├── server_service.rs
│   │   │   ├── notification.rs
│   │   │   ├── viewport.rs    # Future: ViewportService
│   │   │   ├── presence.rs    # Future: PresenceService
│   │   │   └── syntax.rs      # Future: SyntaxService
│   │   └── session/           # Session management
│   │       ├── session.rs
│   │       ├── registry.rs
│   │       └── presence.rs    # Future: Client presence tracking
│   └── Cargo.toml
│
├── clients/
│   ├── model/                 # Common Client Model (shared abstractions)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   │
│   │   │   ├── wire/          # Wire format types (from server)
│   │   │   │   ├── mod.rs
│   │   │   │   ├── overlay.rs # LogicalOverlay, Anchor, OverlayState
│   │   │   │   ├── layout.rs  # LogicalLayout enum
│   │   │   │   └── viewport.rs# ViewportUpdate
│   │   │   │
│   │   │   ├── rendered/      # Client-side rendered state
│   │   │   │   ├── mod.rs
│   │   │   │   ├── overlay.rs # RenderedOverlay, OverlayManager trait
│   │   │   │   ├── window.rs  # Window, WindowTree
│   │   │   │   └── panel.rs   # Panel trait
│   │   │   │
│   │   │   ├── traits/        # Core trait definitions
│   │   │   │   ├── mod.rs
│   │   │   │   ├── layout.rs  # Layout trait
│   │   │   │   ├── renderer.rs# OverlayRenderer trait
│   │   │   │   ├── focus.rs   # FocusManager trait
│   │   │   │   └── interpreter.rs  # LayoutInterpreter trait
│   │   │   │
│   │   │   ├── sync/          # Multi-client sync
│   │   │   │   ├── mod.rs
│   │   │   │   ├── layout.rs  # LayoutSyncMode
│   │   │   │   ├── overlay.rs # OverlaySyncMode
│   │   │   │   └── presence.rs# Presence tracking
│   │   │   │
│   │   │   └── interaction.rs # Interaction enum
│   │   └── Cargo.toml
│   │
│   ├── tui/                   # TUI client crate
│   │   ├── src/
│   │   │   ├── grpc_client.rs # gRPC client wrapper
│   │   │   ├── platform.rs    # Platform trait impl (crossterm)
│   │   │   ├── theme/         # Theme engine (tokens → colors)
│   │   │   │   ├── mod.rs
│   │   │   │   ├── default.rs
│   │   │   │   └── loader.rs
│   │   │   ├── render/        # Platform rendering
│   │   │   │   ├── mod.rs
│   │   │   │   ├── panel.rs   # Panel rendering
│   │   │   │   ├── overlay.rs # Overlay rendering
│   │   │   │   └── gutter.rs  # Gutter rendering
│   │   │   └── input.rs       # Key translation
│   │   └── Cargo.toml         # depends on clients/model
│   │
│   └── cli/                   # CLI client crate
│       └── src/
│           └── client.rs      # GrpcClient
│
└── protocol/                  # Shared protocol types
    ├── proto/reovim/v2/       # Protobuf definitions
    │   ├── buffer.proto
    │   ├── input.proto
    │   ├── state.proto
    │   ├── notification.proto
    │   ├── viewport.proto     # Future: Viewport management
    │   ├── layout.proto       # Future: Logical layout sharing
    │   ├── presence.proto     # Future: Multi-client presence
    │   └── syntax.proto       # Future: Syntax tokens
    └── src/v2/                # Generated code
```

### Client Model as Shared Library

The `lib/clients/model/` crate can be:

1. **Rust library** - Used directly by TUI (Rust)
2. **FFI bindings** - Exposed via C ABI for Android (JNI) and iOS (Swift)
3. **WASM module** - Compiled to WebAssembly for Web client

```
┌─────────────────────────────────────────────────────────────┐
│  lib/clients/model/ (Rust)                                  │
│  • Pure logic, no I/O                                       │
│  • Platform-agnostic abstractions                           │
└───────────┬─────────────────┬─────────────────┬─────────────┘
            │                 │                 │
    ┌───────▼───────┐ ┌───────▼───────┐ ┌───────▼───────┐
    │  Direct use   │ │   JNI/FFI     │ │     WASM      │
    │  (TUI, CLI)   │ │  (Android)    │ │    (Web)      │
    └───────────────┘ └───────────────┘ └───────────────┘
```

## Migration Path

### Phase 8A ✅ (Complete)
- NotificationService with streaming
- TuiGrpcClient module ready

### Phase 8B (Deferred)
- Migrate TUI from JSON-RPC v1 to gRPC v2
- Remove `lib/clients/core/` dependency

### Phase 9: Web Client PoC (Next)

**Goal**: Validate multi-platform architecture by building a minimal web client.

#### Why Web First
- Maximum platform difference from TUI (TypeScript/DOM vs Rust/terminal)
- Fast iteration (hot reload, browser devtools)
- Easy to demo (just share URL)
- Forces clean data/presentation separation

#### Transport: WebSocket + Protobuf
Browsers can't do native gRPC. Add WebSocket transport to server:

```
┌─────────────┐     WebSocket      ┌─────────────┐
│  Web Client │ ◄──────────────►   │   Server    │
│  (browser)  │   protobuf msgs    │  (Rust)     │
└─────────────┘                    └─────────────┘
```

- Server: Add WebSocket endpoint alongside gRPC (`/ws`)
- Messages: Same protobuf types, just different transport
- No proxy needed (unlike gRPC-Web)

#### Server Changes
```
lib/server/src/
├── transport/
│   ├── mod.rs
│   ├── grpc.rs      # Existing gRPC transport
│   └── websocket.rs # NEW: WebSocket transport
```

- Accept WebSocket upgrade on `/ws`
- Wrap service calls in WebSocket message framing
- Reuse existing service implementations

#### Web Client Structure
```
clients/web/
├── package.json
├── vite.config.ts
├── src/
│   ├── main.ts           # Entry point
│   ├── transport/
│   │   └── websocket.ts  # WebSocket + protobuf client
│   ├── services/
│   │   ├── buffer.ts     # BufferService client
│   │   ├── input.ts      # InputService client
│   │   ├── state.ts      # StateService client
│   │   └── notification.ts # NotificationService client
│   ├── editor/
│   │   ├── Editor.ts     # Main editor component
│   │   ├── Buffer.ts     # Buffer display
│   │   ├── Cursor.ts     # Cursor rendering
│   │   └── Statusline.ts # Mode/file info
│   └── theme/
│       └── default.ts    # Token → CSS class mapping
└── index.html
```

#### MVP Features

| Feature | Description | Service Used |
|---------|-------------|--------------|
| Connect | WebSocket handshake | — |
| Display buffer | Show raw lines | `BufferService.GetRawContent` |
| Send keys | Keyboard → server | `InputService.SendKeys` |
| Mode display | Show NORMAL/INSERT | `NotificationService.Subscribe` |
| Cursor | Blinking caret at position | `StateService.GetCursor` |

#### Tech Stack
- **Build**: Vite (fast, modern)
- **Language**: TypeScript
- **Protobuf**: `protobuf.js` or `@bufbuild/protobuf`
- **Styling**: Plain CSS or Tailwind
- **No framework**: Vanilla TS for simplicity

#### Acceptance Criteria
- [ ] Server accepts WebSocket connections on `/ws`
- [ ] Web client connects and displays buffer content
- [ ] Keyboard input sends keys to server
- [ ] Mode changes reflected in statusline
- [ ] Cursor position updates in real-time
- [ ] Works alongside TUI (same server, multiple clients)

### Phase 10: Common Client Model (Future)
- Create `lib/clients/model/` crate
- Define Panel, Overlay, Layout, Focus traits
- Implement LogicalLayout structure (splits, tabs)
- Client-side `<C-w>` key handling
- LayoutInterpreter trait for platform adaptation

### Phase 11: ViewportService (Future)
- Server tracks viewports (buffer + cursor per viewport)
- `CreateViewport`, `CloseViewport`, `FocusViewport`
- Enables multiple cursors in same buffer

### Phase 12: LayoutService (Future)
- Logical layout storage on server
- `GetLayout`, `SetLayout`, `StreamLayout` RPCs
- Layout sync modes (Independent, Broadcast, Follow, Accept)
- Session layout persistence (save/restore)

### Phase 13: SyntaxService (Future)
- Server provides tokens via treesitter
- `GetTokens`, `StreamTokens` RPCs
- Token types: keyword, string, comment, etc.
- Remove colors from server-side rendering

### Phase 14: TUI Theme Engine (Future)
- Token type → color mapping
- Theme file format (TOML/JSON)
- Client-side gutter rendering
- Full data/presentation separation

### Phase 15: PresenceService (Future)
- Multi-client awareness
- Cursor sharing between clients
- Follow mode for presentations
- Enables Android/Web clients to feel "synced"

## Related Documents

- [Overview](./overview.md) - General architecture
- [Session Model](./session-model.md) - Multi-client sessions
- [Type Layers](./type-layers.md) - Type organization

## Related Issues

- [#465](https://github.com/user/reovim/issues/465) - Epic: Server/Client split
- [#464](https://github.com/user/reovim/issues/464) - Move gutter rendering to client
