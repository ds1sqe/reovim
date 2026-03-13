# Plugin and Module Architecture Design (Superseded)

> **Superseded by**: [Client Layer Model](../architecture/client/overview.md) (v6.3).
> This document is preserved as historical reference. The `ClientExtension` trait described
> here has been replaced by `ClientModule` in the Client Layer Model.

Reference: #468 (which-key), #469 (cmdline UI)

## 1. Taxonomy

This section establishes precise definitions for the four component types
in reovim. Getting the naming right prevents confusion in all future work.

### 1.1 Module (Server-Side Policy)

A **module** is a server-side component that implements the `Module` trait.
Modules define policy -- HOW the editor behaves. They are loaded as `.so`
shared libraries via `declare_module!` and run inside the server process.

- Lives in: `server/modules/`
- Trait: `reovim_kernel::api::v1::Module`
- FFI: `declare_module!` macro generates entry points
- Loading: Dynamic `.so` loading via `libloading`
- Communication: Direct kernel API calls (`ModuleContext`)
- Examples: vim, editor, motions, textobjects, keymap, commands

Modules have NO knowledge of rendering. They produce data and state
changes. They never emit terminal escape sequences, DOM nodes, or
pixels.

### 1.2 Client Extension (Client-Side UI)

A **client extension** is a client-side component that renders UI
and handles client-local interactions. Each platform (TUI, web)
implements its own extensions using platform-native facilities.

- Lives in: `clients/{tui,web}/extensions/` (each extension is a separate crate)
- Trait (TUI): `ClientExtension` with `composable()` bridge to `Composable`
- Trait (Web): TypeScript `ClientExtension` with `getRenderer()` bridge to `DomRenderer`
- Loading: Compiled into the client binary (TUI) or bundled (web)
- Communication: Subscribes to event topics, receives only subscribed events
- Examples: which-key popup, cmdline renderer, completion popup

**Game-mod separation**: The client engine (TUI main loop, web app shell)
has ZERO knowledge of specific extensions. It only knows the
`ClientExtension` trait. Concrete extensions are provided by a
`defaults` meta-crate that mirrors the server's `DefaultsModule` pattern:

```
clients/tui/extensions/
  defaults/src/lib.rs     -- pub fn create_extensions() -> Vec<Box<dyn ClientExtension>>
  which-key/src/lib.rs    -- WhichKeyExtension (separate crate)
  cmdline/src/lib.rs      -- CmdlineExtension (separate crate)
```

The engine calls `create_extensions()` at startup. It never imports,
names, or constructs individual extensions. This prevents architectural
coupling where the engine "knows" about which-key, cmdline, etc.

Client extensions are platform-specific. The TUI extension for which-key
uses `FrameBuffer` and terminal cells. The web extension uses DOM elements.
They share no code at the rendering level.

### 1.3 Feature (Module + Extension Pair)

A **feature** is the user-visible capability produced by combining a
server module with zero or more client extensions. Features are what
users think about: "completion", "which-key", "cmdline".

Not all features require both halves:

| Feature | Server Module | Client Extension |
|---------|--------------|-----------------|
| vim mode | Yes (vim) | No (no special UI) |
| which-key | No (reads existing keymap data) | Yes (popup renderer) |
| cmdline | Yes (CmdlineState) | Yes (input renderer) |
| completion | Yes (completion provider) | Yes (popup renderer) |
| diagnostics | Yes (LSP driver) | Yes (gutter + inline) |
| file explorer | Yes (VFS + tree state) | Yes (panel renderer) |

### 1.4 Session Extension (Per-Session State)

A **session extension** is a typed storage slot in an `ExtensionMap`
for server-side mutable state. Session extensions are the mechanism
by which modules store state that varies across sessions.

- Trait: `reovim_driver_session::SessionExtension`
- Storage: `ExtensionMap` (TypeId-keyed HashMap)
- Lifetime: Per-session (created on first access, destroyed with session)

**Shared vs Per-Client state** (see ongoing work in resolver API split):

There are TWO `ExtensionMap` instances per session:

| Map | Scope | Examples |
|-----|-------|---------|
| `shared_extensions` | Shared across all clients in a session | Keymap registrations, completion sources |
| `client_extensions` | Per-client, isolated | `VimSessionState`, `CmdlineState`, `OperatorPendingState` |

This distinction is critical: `CmdlineState` is per-client (each client
has its own `:` prompt and cursor position). `VimSessionState` (pending
motion, count, register) is also per-client. Mixing these up causes bugs
where one client's operator state leaks into another's.

**Impact on ExtensionStateBridge**: Bridges must declare which map they
read from (see Section 4.4). `CmdlineBridge` reads `client_extensions`.

Session extensions are NOT the same as modules or client extensions.
They are the server-side state layer that modules and the runner share.


## 2. Layer Model

This section shows where each component type fits in the existing
Linux-kernel-inspired architecture.

```
CLIENT LAYER (Application)
===========================================================================
clients/tui/                         clients/web/
  src/
    extension.rs  (ClientExtension trait + ClientEvent + DeserializerRegistry)
    router.rs     (ExtensionRouter -- topic-based dispatch)
  extensions/                          extensions/
    defaults/     (meta-crate)           defaults/   (meta-module)
      create_extensions()                  createExtensions()
    which-key/    (separate crate)       which-key/  (separate module)
    cmdline/      (separate crate)       cmdline/    (separate module)
    completion/   (separate crate)       completion/ (separate module)
  lib/drivers/display/               src/render/
    compositor/                          overlay.ts
      composable.rs                      layout.ts
      layer.rs                           buffer.ts

  Engine imports ONLY: create_extensions() -> Vec<Box<dyn ClientExtension>>
  Engine NEVER imports individual extension crates.
  Extensions declare subscriptions; router dispatches by topic.
---------------------------------------------------------------------------
                      gRPC v2 Protocol
                  (shared/protocol/proto/)
---------------------------------------------------------------------------
SERVER LAYER (Policy + Mechanism)
===========================================================================

SHARED CLIENT MODEL (platform-agnostic logic)
  shared/clients/model/
    wire/       -- LogicalOverlay, Anchor, OverlayState
    rendered/   -- RenderedOverlay, WindowTree
    traits/     -- OverlayRenderer, OverlayManager
    interaction/ -- Interaction, InteractionResult
    sync/       -- LayoutSyncMode, OverlaySyncMode

MODULE LAYER (Policy)
  server/modules/
    vim/            -- Vim keybindings, operators, modes
    editor/         -- Core editing operations
    commands/       -- Ex-commands (:w, :q, :e)
    keymap/         -- Keymap definitions
    motions/        -- Movement commands
    textobjects/    -- Text object definitions
    ...             -- 19 modules total

DRIVER LAYER (Mechanism)
  server/lib/drivers/
    session/        -- Session state, ExtensionMap, CmdlineState
    input/          -- Key events, mode resolution
    command/        -- Command dispatch
    buffer/         -- Buffer operations
    syntax/         -- Syntax highlighting
    ...             -- 14 drivers total

KERNEL LAYER (Mechanism)
  server/lib/kernel/
    api/            -- Module trait, ServiceRegistry, ModuleContext
    mm/             -- Buffer memory management
    ipc/            -- EventBus, pub/sub
    core/           -- MotionEngine, TextObjectEngine, RegisterBank
    block/          -- File I/O
    sched/          -- Scheduling primitives
```


## 3. Lifecycle Diagrams

### 3.1 Server Module Lifecycle

This is the EXISTING lifecycle. No changes needed -- it is mature and
well-tested with 19 modules.

```
                    load .so
                       |
                       v
   +-----------+   +-----------+   +--------+   +-----------+
   | Discovered|-->| Probed    |-->| Init   |-->| Running   |
   +-----------+   +-----------+   +--------+   +-----------+
        |               |              |              |
        |               |              |              | on_all_loaded()
        |               |              |              | on_buffer_focus()
        |               |              |              |
        |               |        ProbeResult::Defer   | exit() + on_unload()
        |               |              |              |
        |               v              v              v
        |          +---------+   +---------+   +-----------+
        |          | Rejected|   | Deferred|   | Unloaded  |
        |          +---------+   +---------+   +-----------+
        |                             |
        |                             | retry
        |                             v
        |                        +--------+
        +----------------------->| Init   |
                                 +--------+
```

Key states:
- **Discovered**: `.so` file found on disk
- **Probed**: `reovim_module_probe()` called, metadata read
- **Init**: `Module::init(&mut self, ctx: &ModuleContext)` called
- **Running**: Module is active, processing events
- **Deferred**: Init returned `ProbeResult::Defer`, will retry
- **Unloaded**: `exit()` called, `.so` can be unloaded

### 3.2 Client Extension Lifecycle

This is the NEW lifecycle for client-side extensions.

Extensions self-manage their active/suspended states based on events
they receive through subscriptions. The engine never calls activate()
or deactivate() -- it only queries `is_active()` and `wants_input()`
as capability checks.

```
   +-----------+   +-----------+   +-----------+   +-----------+
   | Created   |-->| Routed    |-->| Active    |-->| Inactive  |
   +-----------+   +-----------+   +-----------+   +-----------+
        |               |               |               |
        |  Router builds|               |               | (self-managed)
        |  routing table|               |               | on_event() triggers
        |  from subs()  |               |               | is_active() -> true
        |               |               |               v
        |               |               +<---------+-----------+
        |               |               |          | Active    |
        |               |               |          +-----------+
        |               |               |
        |               |               | destroy()
        |               |               v
        |               |          +-----------+
        +-------------->+--------->| Destroyed |
                                   +-----------+
```

Key states:
- **Created**: Extension instantiated by `create_extensions()`, not yet routed
- **Routed**: Engine has indexed this extension's subscriptions into routing table
- **Active**: Extension is visible (`is_active() == true`), receiving subscribed events
- **Inactive**: Extension is hidden but still receiving subscribed events (self-manages transition)
- **Destroyed**: Extension cleaned up (client disconnect)

Note: Active/Inactive transitions are INTERNAL to the extension. The
engine only observes them via `is_active()` for rendering decisions and
`wants_input()` for keyboard routing. The engine never commands an
extension to activate or deactivate.

### 3.3 Feature Lifecycle (Coordinated)

A feature that spans server and client coordinates both lifecycles.

Example: **cmdline** feature lifecycle:

```
  Server (CmdlineState)              Client (CmdlineExtension)
  =======================            =========================

  1. Module init()                   1. Extension registered
     - CmdlineState registered          at client startup
       as SessionExtension

  2. User presses ':'                2. Client receives
     - VimModule enters cmdline mode    ModeChanged notification
     - CmdlineState.enter(Command)      (mode = "command_line")
     - ModeChanged event emitted

                                     3. CmdlineExtension.activate()
                                        - Queries CmdlineState
                                          via new gRPC endpoint
                                        - Renders prompt ":" at bottom
                                        - Captures keyboard focus

  3. User types "wq"                 4. Each keystroke:
     - Input routed to CmdlineState     - Notification: cmdline_updated
     - CmdlineState.insert_char('w')    - Re-render with "wq" + cursor
     - CmdlineState.insert_char('q')

  4. User presses Enter              5. CmdlineExtension.deactivate()
     - ExitCommandLineMode runs         - Hides prompt
     - CmdlineState.exit()              - Returns keyboard focus
     - ModeChanged event (normal)
     - Commands module executes ":wq"
```


## 4. Interaction Protocol

### 4.1 State Ownership Principle

**The server owns ALL editing state. Clients own ALL rendering state.**

This means:
- Buffer content, cursor positions, mode, selections --> server
- Frame buffers, compositors, DOM elements, scroll state --> client
- Overlay data (what to show) --> server
- Overlay rendering (how to show it) --> client

### 4.2 Communication Patterns

Three patterns cover all server-client communication:

#### Pattern A: Push Notification (Real-time Updates)

Server pushes state changes to clients via gRPC streaming.

```
Server                              Client
  |                                   |
  |--- Notification (stream) -------->|
  |    event_type: "mode_changed"     |
  |    payload: {name: "cmdline"}     |
  |                                   |
```

Used for: mode changes, cursor moves, buffer modifications, layout
changes, viewport updates, presence updates.

This is the EXISTING pattern. No changes needed for the core protocol.

#### Pattern B: Pull Query (On-Demand State)

Client queries server state via unary gRPC RPCs.

```
Server                              Client
  |                                   |
  |<-- GetMode(client_id) ------------|
  |--- ModeResponse {name, display} ->|
  |                                   |
```

Used for: initial state sync, frame capture, register queries.

This is the EXISTING pattern. Extensions need new RPCs (see 4.3).

#### Pattern C: Extension State Streaming (NEW)

A new gRPC service for querying extension-specific state. This is the
key addition that enables client extensions to read server module state.

```
Server                              Client
  |                                   |
  |<-- GetExtensionState(kind) -------|
  |--- ExtensionState {json_data} --->|
  |                                   |
  |--- Notification ----------------->|
  |    event_type: "extension_updated"|
  |    payload: {kind, data}          |
  |                                   |
```

This is the NEW pattern that bridges modules and client extensions.


### 4.3 New Protocol Additions

#### 4.3.1 Extension State Service

A new gRPC service for extension state queries:

```protobuf
// shared/protocol/proto/reovim/v2/extension.proto

service ExtensionService {
  // Query extension state by kind.
  rpc GetState(GetExtensionStateRequest) returns (GetExtensionStateResponse);

  // List available extension kinds and their capabilities.
  rpc ListExtensions(ListExtensionsRequest) returns (ListExtensionsResponse);
}

message GetExtensionStateRequest {
  // Extension kind (e.g., "cmdline", "which-key", "completion")
  string kind = 1;
  // Client ID for per-client state
  uint64 client_id = 2;
}

message GetExtensionStateResponse {
  bool active = 1;
  // JSON-encoded state (schema depends on kind)
  string data = 2;
}

message ListExtensionsRequest {}

message ListExtensionsResponse {
  repeated ExtensionInfo extensions = 1;
}

message ExtensionInfo {
  string kind = 1;           // Extension identifier
  string description = 2;    // Human-readable description
  bool push_supported = 3;   // Whether push notifications are available
  bool query_supported = 4;  // Whether pull queries are available
}
```

#### 4.3.2 Extension Update Notification

Add to existing notification proto:

```protobuf
// In notification.proto, add to Notification.payload oneof:

ExtensionUpdatedPayload extension_updated = 26;

message ExtensionUpdatedPayload {
  string kind = 1;     // Which extension changed
  string data = 2;     // JSON-encoded state
  uint64 client_id = 3; // Which client this applies to
}
```

**Who emits these notifications and where:**

Extension update notifications are emitted by the same pipeline that emits
all other notifications (mode changes, cursor moves, buffer edits):

1. Key resolved in `server/lib/server/src/grpc/input.rs`
2. State changes accumulated into `StateChanges` struct
3. `emit_notifications(&session, &accumulated_changes, client_id)` called
4. `build_notifications()` in `notification_builder.rs` converts `StateChanges`
   to gRPC `Notification` messages (including `ExtensionUpdatedPayload`)
5. `session.emit_notification()` broadcasts to all connected clients

The new `ExtensionUpdatedPayload` variant is added to `build_notifications()`.
When a bridge's `is_active()` state changes (detected by comparing pre/post
snapshots), the builder emits an `extension_updated` notification with the
bridge's `kind()` and `snapshot()` data. No new emission site is needed --
the existing notification pipeline handles it.

#### 4.3.3 Keymap Query Service

For which-key specifically, the server needs to expose keymap state:

```protobuf
// In a new keymap.proto:

service KeymapService {
  // Get keybindings matching a prefix in the current mode.
  rpc GetBindings(GetBindingsRequest) returns (GetBindingsResponse);
}

message GetBindingsRequest {
  string prefix = 1;     // Key prefix (e.g., "<Space>", "g")
  string mode = 2;       // Mode name (e.g., "normal")
  uint64 client_id = 3;  // For per-client mode
}

message GetBindingsResponse {
  repeated KeyBinding bindings = 1;
}

message KeyBinding {
  string keys = 1;          // Full key sequence
  string description = 2;   // Human-readable description
  string category = 3;      // Category for grouping
  string command_id = 4;    // Command that will be executed
}
```

### 4.4 Extension State Bridge

The server needs a mechanism to expose session extension state to clients
without violating the separation of concerns.

The solution: an **ExtensionStateBridge** in the server that adapts
typed `SessionExtension` state into serializable wire format.

```rust
// server/lib/drivers/session/src/bridges/mod.rs
//
// Lives in the DRIVER layer, not the runner/server layer.
// Bridges adapt driver-layer state (ExtensionMap) to wire format.
// The runner imports bridges from the driver crate and registers them
// with the gRPC handler.

/// [PROPOSED] Which ExtensionMap a bridge reads from.
///
/// NOTE: This enum does not exist yet. It is proposed as part of this
/// architecture document. The dual ExtensionMap (shared vs client) is
/// being introduced by the resolver API split (see active plan in
/// ~/.claude/plans/ for issue #474). ExtensionScope will be added to
/// the driver layer once that work lands.
///
/// Sessions have TWO extension maps (see Section 1.4):
/// - Shared: state visible to all clients (keymap registrations, etc.)
/// - Client: per-client isolated state (CmdlineState, VimSessionState, etc.)
///
/// Bridges must declare which map they need. The runner passes the
/// correct map when calling snapshot().
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionScope {
    /// Read from shared_extensions (same for all clients).
    Shared,
    /// Read from client_extensions (different per client).
    /// Requires client_id to identify which client's map.
    Client,
}

/// Trait for adapting session extension state to wire format.
///
/// Each extension that wants to expose state to clients implements
/// this trait. The server calls snapshot() when a client queries
/// state or when the extension notifies of changes.
pub trait ExtensionStateBridge: Send + Sync + 'static {
    /// Extension kind identifier (matches proto GetExtensionStateRequest.kind).
    fn kind(&self) -> &'static str;

    /// Which ExtensionMap this bridge reads from.
    ///
    /// The runner uses this to pass the correct map to snapshot().
    /// Client-scoped bridges receive the requesting client's map.
    fn scope(&self) -> ExtensionScope;

    /// Snapshot current state as JSON.
    ///
    /// The `extensions` parameter is the map indicated by scope():
    /// - Shared -> shared_extensions
    /// - Client -> client_extensions for the requesting client
    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value>;

    /// Whether this extension is currently active.
    fn is_active(&self, extensions: &ExtensionMap) -> bool;
}
```

Concrete bridge implementations live in the **driver layer** alongside
the state types they adapt. This respects the layer model: bridges read
driver state (`ExtensionMap`, `CmdlineState`), so they belong with drivers.

```rust
// server/lib/drivers/session/src/bridges/cmdline.rs

pub struct CmdlineBridge;

impl ExtensionStateBridge for CmdlineBridge {
    fn kind(&self) -> &'static str { "cmdline" }

    // CmdlineState is per-client: each client has its own : prompt.
    fn scope(&self) -> ExtensionScope { ExtensionScope::Client }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        // `extensions` is the CLIENT's ExtensionMap (per scope()).
        let state = extensions.get::<CmdlineState>()?;
        Some(serde_json::json!({
            "active": state.is_active(),
            "prompt": state.prompt().char().to_string(),
            "input": state.input(),
            "cursor": state.cursor(),
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions.get::<CmdlineState>()
            .map_or(false, |s| s.is_active())
    }
}
```

**Runner dispatch logic:**
```rust
// In gRPC handler for GetExtensionState:
fn get_state(&self, kind: &str, client_id: ClientId) -> Option<Value> {
    let bridge = self.bridges.get(kind)?;
    let extensions = match bridge.scope() {
        ExtensionScope::Shared => &self.app.extensions,
        ExtensionScope::Client => &self.clients.get(client_id)?.state.extensions,
    };
    bridge.snapshot(extensions)
}
```

**Layer dependency direction:**
```
server/lib/server (runner)
  ↓ imports ExtensionStateBridge trait + CmdlineBridge from
server/lib/drivers/session (driver)
  ↓ reads from
ExtensionMap (shared OR client, based on scope())
```

The runner registers bridges and wires them to gRPC handlers. The
driver defines bridges alongside the state they serialize. No circular
dependencies.

**Dependency note**: `ExtensionStateBridge::snapshot()` returns
`serde_json::Value`. The session driver crate (`reovim-driver-session`)
does not currently depend on `serde_json`. Adding this dependency is
required when implementing bridges. This is acceptable: `serde_json` is
already a transitive dependency via the server crate, and bridges need
JSON serialization by design.


## 5. Client Extension Traits

### 5.1 TUI Client Extension Trait (Rust)

The design uses a **single trait with capability bridge methods** rather
than a sub-trait hierarchy. This avoids Rust's trait-object downcasting
limitation: you cannot cast `dyn ClientExtension` to `dyn Composable`
at runtime without additional infrastructure.

Instead, extensions that render UI implement `Composable` on their
concrete type AND override `composable()` to return `Some(self)`.
The router calls `composable()` to discover renderable extensions
without knowing their concrete type.

```rust
// clients/tui/src/extension.rs

use std::any::Any;
use reovim_display::compositor::{Composable, FrameBuffer, Style, Bounds, ZOrder};

// ---------------------------------------------------------------------------
// ClientEvent: typed event delivered to extensions
// ---------------------------------------------------------------------------

/// Type-erased event delivered to extensions.
///
/// Wraps a topic string and a typed payload. Extensions use
/// downcast_ref::<T>() to recover the concrete payload type.
///
/// The router owns a DeserializerRegistry that converts raw gRPC
/// bytes into typed payloads. Extensions never touch raw bytes.
pub struct ClientEvent {
    topic: String,
    payload: Box<dyn Any + Send + Sync>,
}

impl ClientEvent {
    pub fn new<T: Any + Send + Sync>(topic: String, payload: T) -> Self {
        Self { topic, payload: Box::new(payload) }
    }

    pub fn topic(&self) -> &str { &self.topic }

    /// Downcast the payload to a concrete type.
    ///
    /// Returns None if the type doesn't match. Extensions should
    /// log a warning and return early on type mismatch.
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.payload.downcast_ref()
    }
}

// ---------------------------------------------------------------------------
// DeserializerRegistry: topic -> typed payload conversion
// ---------------------------------------------------------------------------

/// Registry mapping topic strings to deserialization functions.
///
/// The router uses this to convert raw gRPC bytes into typed
/// ClientEvent payloads. Each topic has exactly one deserializer.
///
/// Registered by the defaults meta-crate alongside extensions:
///   registry.register(topics::MODE_CHANGED, |bytes| {
///       serde_json::from_slice::<ModeChangedPayload>(bytes)
///           .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
///   });
pub struct DeserializerRegistry {
    deserializers: HashMap<String, DeserializerFn>,
}

/// A function that deserializes raw bytes into a type-erased payload.
///
/// Uses Box<dyn Error> rather than serde_json::Error because the wire
/// format is protobuf (gRPC). Concrete deserializers may use serde_json
/// internally (for JSON-encoded extension state), protobuf decoding,
/// or any other format. The router only cares about success/failure.
type DeserializerFn = Box<dyn Fn(&[u8]) -> Result<Box<dyn Any + Send + Sync>, Box<dyn std::error::Error + Send + Sync>>
    + Send + Sync>;

impl DeserializerRegistry {
    pub fn new() -> Self {
        Self { deserializers: HashMap::new() }
    }

    /// Register a deserializer for a topic.
    pub fn register<T, F>(&mut self, topic: &str, f: F)
    where
        T: Any + Send + Sync + 'static,
        F: Fn(&[u8]) -> Result<T, Box<dyn std::error::Error + Send + Sync>> + Send + Sync + 'static,
    {
        self.deserializers.insert(
            topic.to_owned(),
            Box::new(move |bytes| f(bytes).map(|v| Box::new(v) as Box<dyn Any + Send + Sync>)),
        );
    }

    /// Deserialize raw bytes for a given topic into a ClientEvent.
    ///
    /// Returns None if no deserializer is registered for this topic.
    pub fn deserialize(&self, topic: &str, bytes: &[u8]) -> Option<ClientEvent> {
        let f = self.deserializers.get(topic)?;
        let payload = f(bytes).ok()?;
        Some(ClientEvent { topic: topic.to_owned(), payload })
    }
}

// ---------------------------------------------------------------------------
// Topic constants: prevent typos in subscription strings
// ---------------------------------------------------------------------------

/// Well-known topic strings used by extensions and the defaults meta-crate.
///
/// Extensions subscribe to these topics. The server emits notifications
/// tagged with these strings. Using constants prevents hard-to-debug
/// typos ("mode.change" vs "mode.changed").
///
/// New topics are added here as new notification types are introduced.
/// The engine never interprets these strings -- they are opaque keys.
pub mod topics {
    pub const MODE_CHANGED: &str = "mode.changed";
    pub const CMDLINE_UPDATED: &str = "cmdline.updated";
    pub const KEYMAP_TIMEOUT: &str = "keymap.timeout";
    pub const BUFFER_CHANGED: &str = "buffer.changed";
    pub const CURSOR_MOVED: &str = "cursor.moved";
    pub const COMPLETION_UPDATED: &str = "completion.updated";
    pub const DIAGNOSTIC_UPDATED: &str = "diagnostic.updated";
}

// ---------------------------------------------------------------------------
// ClientExtension: the single trait for all extensions
// ---------------------------------------------------------------------------

/// Trait for ALL client extensions.
///
/// One trait with capability bridge methods, not a sub-trait hierarchy.
/// This design avoids Rust's trait-object downcasting limitation.
///
/// Extensions that render UI: implement Composable on their concrete
/// type AND override composable()/composable_mut() to return Some(self).
///
/// Extensions that capture input: override wants_input()/handle_key().
///
/// DESIGN INVARIANT: The engine (TUI main loop) NEVER routes by
/// extension identity. It uses only capability queries and the
/// subscription-based routing table.
pub trait ClientExtension: Send + Sync {
    /// Extension identifier, used ONLY for logging and debugging.
    ///
    /// The engine MUST NOT use this for routing, dispatch, or any
    /// behavioral decision.
    fn id(&self) -> &'static str;

    /// Event topics this extension subscribes to.
    ///
    /// Called at registration time to build the routing table.
    /// Returns Vec<String> to allow config-dependent subscriptions.
    ///
    /// Topics are opaque strings. Convention: "domain.event_name"
    /// Use constants from `topics` module to prevent typos:
    ///   vec![topics::MODE_CHANGED.into(), topics::CMDLINE_UPDATED.into()]
    fn subscriptions(&self) -> Vec<String>;

    /// Called when a subscribed event arrives.
    ///
    /// The router guarantees: event.topic() is one of self.subscriptions().
    /// Use event.downcast_ref::<T>() to recover the typed payload.
    fn on_event(&mut self, event: &ClientEvent);

    /// Whether this extension is currently active.
    ///
    /// For renderable extensions: active means visible (composable() renders).
    /// For non-renderable extensions: active means processing events.
    /// Extensions self-manage this state via on_event().
    fn is_active(&self) -> bool;

    // -- Rendering capability (bridge to Composable) --

    /// Return a Composable view of this extension for rendering.
    ///
    /// Override to return Some(self) if this extension renders UI.
    /// The router uses this to discover renderable extensions without
    /// knowing their concrete type.
    ///
    /// Default: None (non-rendering extension).
    fn composable(&self) -> Option<&dyn Composable> { None }

    /// Mutable Composable view (for set_z_order, etc).
    fn composable_mut(&mut self) -> Option<&mut dyn Composable> { None }

    // -- Input capability (optional) --

    /// Whether this extension should receive keyboard input.
    ///
    /// Override to return true for modal extensions (cmdline, etc).
    /// Default: false.
    fn wants_input(&self) -> bool { false }

    /// Handle a key event. Returns true if consumed.
    ///
    /// Only called when wants_input() returns true.
    /// Default: not consumed.
    fn handle_key(&mut self, key: &KeyEvent) -> bool { let _ = key; false }
}
```

**How the bridge method works:**

```rust
// Example: WhichKeyExtension renders UI AND captures input

struct WhichKeyExtension { /* state */ }

// Implement Composable on the concrete type (6 required + 2 default methods)
impl Composable for WhichKeyExtension {
    fn id(&self) -> ComposableId { /* ... */ }
    fn z_order(&self) -> ZOrder { /* ... */ }
    fn set_z_order(&mut self, z: ZOrder) { /* ... */ }
    fn is_visible(&self) -> bool { self.active }
    fn bounds(&self, w: u16, h: u16) -> Bounds { /* popup bounds */ }
    fn render(&self, buf: &mut FrameBuffer, style: &Style) { /* draw popup */ }
}

// Implement ClientExtension, bridging to Composable via self
impl ClientExtension for WhichKeyExtension {
    fn id(&self) -> &'static str { "which-key" }
    fn subscriptions(&self) -> Vec<String> {
        vec![topics::KEYMAP_TIMEOUT.into(), topics::MODE_CHANGED.into()]
    }
    fn on_event(&mut self, event: &ClientEvent) {
        if let Some(data) = event.downcast_ref::<KeymapTimeoutPayload>() {
            self.show_popup(data);
        }
    }
    fn is_active(&self) -> bool { self.active }

    // Bridge: return self as &dyn Composable
    fn composable(&self) -> Option<&dyn Composable> { Some(self) }
    fn composable_mut(&mut self) -> Option<&mut dyn Composable> { Some(self) }

    // This extension captures input
    fn wants_input(&self) -> bool { self.active }
    fn handle_key(&mut self, key: &KeyEvent) -> bool { /* ... */ }
}

// Example: BackgroundSyncExtension does NOT render or capture input

struct BackgroundSyncExtension { /* state */ }

impl ClientExtension for BackgroundSyncExtension {
    fn id(&self) -> &'static str { "bg-sync" }
    fn subscriptions(&self) -> Vec<String> { vec![topics::BUFFER_CHANGED.into()] }
    fn on_event(&mut self, event: &ClientEvent) { /* update cache */ }
    fn is_active(&self) -> bool { true }
    // composable() defaults to None -- no rendering
    // wants_input() defaults to false -- no input capture
}
```

**Capability discovery at a glance:**

| Extension | `composable()` | `wants_input()` | Rendering | Input |
|-----------|:-:|:-:|:-:|:-:|
| which-key popup | `Some(self)` | `true` | Yes | Yes |
| cmdline | `Some(self)` | `true` | Yes | Yes |
| completion popup | `Some(self)` | `true` | Yes | Yes |
| background sync | `None` | `false` | No | No |
| macro recorder | `None` | `true` | No | Yes |
| status indicator | `Some(self)` | `false` | Yes | No |

**Anti-coupling guarantee**: The engine never calls `ext.id()` for routing.
It never matches on topic strings. It builds the routing table mechanically
from `subscriptions()` and dispatches by table lookup. This means:

1. Adding a new extension = add crate + register in `defaults` meta-crate
2. The engine binary does NOT need to change
3. No string matching like `if ext.id() == "which_key"` anywhere in engine code

### 5.2 Web Client Extension Interface (TypeScript)

Same bridge method pattern, adapted for TypeScript. Web uses
`unknown` payloads (deserialized from JSON by the router) with
type guards in each extension.

```typescript
// clients/web/src/extensions/types.ts

/**
 * Type-erased event delivered to extensions.
 * Mirrors Rust ClientEvent.
 */
export interface ClientEvent {
  readonly topic: string;
  readonly payload: unknown;  // deserialized JSON, use type guards
}

/**
 * Single interface for ALL web client extensions.
 *
 * Uses capability methods (getRenderer, wantsInput) instead of
 * sub-interfaces, matching the Rust bridge method pattern.
 *
 * DESIGN INVARIANT: The engine never routes by extension identity.
 */
export interface ClientExtension {
  /** Extension identifier, used ONLY for logging/debugging. */
  readonly id: string;

  /** Event topics this extension subscribes to. */
  subscriptions(): string[];

  /** Called when a subscribed event arrives. */
  onEvent(event: ClientEvent): void;

  /** Whether the extension is currently active. */
  isActive(): boolean;

  /** Cleanup when client disconnects. */
  destroy(): void;

  // -- Rendering capability (bridge to DOM) --

  /** Return a DOM renderer if this extension renders UI. */
  getRenderer(): DomRenderer | null;

  // -- Input capability (optional) --

  /** Whether this extension should capture keyboard input. */
  wantsInput(): boolean;

  /** Handle a keyboard event. Returns true if consumed. */
  handleKey(event: KeyboardEvent): boolean;
}

/** Renderer interface for extensions that produce DOM output. */
export interface DomRenderer {
  mount(container: HTMLElement): void;
  render(): void;
  unmount(): void;
}
```

### 5.3 Extension Router (Subscription-Based Dispatch)

Both TUI and web clients use a router that dispatches events by topic
subscription. The router has ZERO knowledge of specific extensions.

**Conceptual parallel to kernel's EventBus**: Both are pub/sub systems
where publishers don't know subscribers. However, they differ in
implementation:

| | Kernel EventBus | Client Router |
|--|-----------------|---------------|
| Key type | `TypeId` (compile-time) | `String` (runtime) |
| Dispatch | Lock-free `ArcSwap` | `HashMap` lookup |
| Priorities | Yes (ordered handlers) | No (insertion order) |
| Consumption | Yes (`EventResult::Consumed`) | No (all subscribers get event) |
| Subscriptions | RAII (dropped with `Subscription`) | Explicit (`rebuild_routes()`) |
| Boundary | In-process | Cross-process (gRPC) |

The parallel is conceptual (pub/sub decoupling), not implementation-level.

**Deserialization pipeline**: The router owns a `DeserializerRegistry`
that converts raw gRPC bytes into typed `ClientEvent` payloads. Extensions
receive typed events via `downcast_ref::<T>()` and never touch raw bytes.

```rust
// clients/tui/src/router.rs

use std::collections::HashMap;

/// Routes events to extensions by topic subscription.
///
/// The router treats topic strings as opaque keys. It never interprets
/// them, never matches on them, never makes behavioral decisions based
/// on their content. It only builds a lookup table and dispatches.
///
/// The router also owns the DeserializerRegistry: it knows how to
/// convert raw gRPC bytes into typed ClientEvent payloads.
pub struct ExtensionRouter {
    extensions: Vec<Box<dyn ClientExtension>>,
    /// Topic -> list of extension indices that subscribed to this topic.
    routes: HashMap<String, Vec<usize>>,
    /// Topic -> deserializer function (converts gRPC bytes to typed payload).
    deserializers: DeserializerRegistry,
}

impl ExtensionRouter {
    /// Create a router from extensions and a deserializer registry.
    ///
    /// Called once at startup. The defaults meta-crate provides both:
    ///   let (extensions, deserializers) = defaults::create_extensions();
    ///   let router = ExtensionRouter::new(extensions, deserializers);
    pub fn new(
        extensions: Vec<Box<dyn ClientExtension>>,
        deserializers: DeserializerRegistry,
    ) -> Self {
        let routes = Self::build_routes(&extensions);
        Self { extensions, routes, deserializers }
    }

    fn build_routes(extensions: &[Box<dyn ClientExtension>]) -> HashMap<String, Vec<usize>> {
        let mut routes: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, ext) in extensions.iter().enumerate() {
            for topic in ext.subscriptions() {
                routes.entry(topic).or_default().push(idx);
            }
        }
        routes
    }

    /// Rebuild the routing table.
    ///
    /// Called if extensions change their subscriptions at runtime
    /// (e.g., after LSP connects and extension subscribes to
    /// "lsp.diagnostic").
    pub fn rebuild_routes(&mut self) {
        self.routes = Self::build_routes(&self.extensions);
    }

    /// Receive raw gRPC bytes: deserialize and dispatch.
    ///
    /// This is the entry point from the gRPC notification handler.
    /// The router deserializes ONCE, then dispatches the typed event.
    pub fn dispatch_raw(&mut self, topic: &str, bytes: &[u8]) {
        let Some(event) = self.deserializers.deserialize(topic, bytes) else {
            // No deserializer registered -- drop silently.
            // Server may emit events that no extension cares about yet.
            return;
        };
        self.dispatch(&event);
    }

    /// Dispatch a pre-built typed event (useful for tests).
    pub fn dispatch(&mut self, event: &ClientEvent) {
        let Some(indices) = self.routes.get(event.topic()) else {
            return;
        };
        // Clone indices to avoid borrow conflict with self.extensions.
        let indices = indices.clone();
        for idx in indices {
            self.extensions[idx].on_event(event);
        }
    }

    /// Find the extension that wants keyboard input (if any).
    ///
    /// Uses capability queries only -- never checks extension identity.
    /// Iterates in reverse (last registered = highest priority).
    pub fn input_target(&mut self) -> Option<&mut dyn ClientExtension> {
        // Two-pass to avoid borrow conflict:
        // Pass 1: find the index.
        let target_idx = self.extensions.iter()
            .enumerate()
            .rev()
            .find(|(_, ext)| ext.is_active() && ext.wants_input())
            .map(|(idx, _)| idx);

        // Pass 2: return mutable reference.
        target_idx.map(|idx| self.extensions[idx].as_mut())
    }

    /// Get all active renderable extensions for the compositor.
    ///
    /// Returns (index, &dyn Composable) pairs for extensions where
    /// composable() returns Some.
    pub fn active_composables(&self) -> Vec<&dyn Composable> {
        self.extensions.iter()
            .filter(|ext| ext.is_active())
            .filter_map(|ext| ext.composable())
            .collect()
    }
}
```

**Web equivalent:**

```typescript
// clients/web/src/extensions/router.ts

type Deserializer = (bytes: Uint8Array) => unknown;

export class ExtensionRouter {
  private extensions: ClientExtension[] = [];
  private routes: Map<string, number[]> = new Map();
  private deserializers: Map<string, Deserializer> = new Map();

  constructor(
    extensions: ClientExtension[],
    deserializers: Map<string, Deserializer>,
  ) {
    this.extensions = extensions;
    this.deserializers = deserializers;
    this.buildRoutes();
  }

  private buildRoutes(): void {
    this.routes.clear();
    for (const [idx, ext] of this.extensions.entries()) {
      for (const topic of ext.subscriptions()) {
        const list = this.routes.get(topic) ?? [];
        list.push(idx);
        this.routes.set(topic, list);
      }
    }
  }

  rebuildRoutes(): void { this.buildRoutes(); }

  dispatchRaw(topic: string, bytes: Uint8Array): void {
    const deserialize = this.deserializers.get(topic);
    if (!deserialize) return;
    this.dispatch({ topic, payload: deserialize(bytes) });
  }

  dispatch(event: ClientEvent): void {
    const indices = this.routes.get(event.topic);
    if (!indices) return;
    for (const idx of indices) {
      this.extensions[idx].onEvent(event);
    }
  }

  inputTarget(): ClientExtension | undefined {
    return [...this.extensions].reverse()
      .find(ext => ext.isActive() && ext.wantsInput());
  }

  activeRenderables(): DomRenderer[] {
    return this.extensions
      .filter(ext => ext.isActive())
      .map(ext => ext.getRenderer())
      .filter((r): r is DomRenderer => r !== null);
  }
}
```

**Why this works:**

| Concern | How it's handled |
|---------|-----------------|
| Which extension gets which event? | Subscription table (built at startup, rebuildable) |
| Does the engine know extension names? | NO -- only trait methods |
| Can the engine route by identity? | NO -- no `ext.id()` calls in dispatch |
| Is broadcast wasteful? | NO -- only subscribers receive events |
| Can new extensions be added? | YES -- add crate + register in defaults |
| Does engine code change? | NO -- router is generic over all extensions |
| Who deserializes gRPC bytes? | DeserializerRegistry (once per event) |
| Can subscriptions change at runtime? | YES -- call `rebuild_routes()` |
| Does the router code compile? | YES -- no trait downcasting, no borrow violations |
| How does router find renderables? | `ext.composable()` bridge method |
| How does router find input targets? | `ext.wants_input()` capability query |


## 6. Concrete Feature Mappings

### 6.1 Which-Key (#468)

Which-key shows a popup of available keybindings after a timeout when
the user has typed a partial key sequence.

**Architecture: Client extension only (no server module needed)**

The which-key popup is purely a client-side visualization of data
that already exists on the server (keybinding registrations). The
server exposes keymap data via the new `KeymapService`.

```
Server Side                          Client Side
===========                          ===========

[Existing] KeybindingStore           [NEW] WhichKeyExtension
  - Stores all keybindings             - Watches for key timeout
  - Queryable by prefix/mode           - Queries KeymapService.GetBindings()
                                       - Renders popup with categories
[NEW] KeymapService (gRPC)
  - GetBindings(prefix, mode)        TUI: FrameBuffer popup at bottom
  - Returns matching bindings        Web: DOM floating panel
```

**Subscriptions:** `["keymap.timeout", "mode.changed"]`

**Data flow:**
1. User presses `<Space>` (or any prefix key)
2. Server emits `keymap.timeout` event (after configurable delay)
3. Router dispatches to WhichKeyExtension (subscribed to `keymap.timeout`)
4. WhichKeyExtension calls `KeymapService.GetBindings(prefix="<Space>", mode="normal")`
5. Server returns matching bindings with descriptions
6. WhichKeyExtension renders popup, sets `is_active() = true`
7. User presses next key: extension hides popup, sets `is_active() = false`
8. `mode.changed` events also trigger hide (e.g., user pressed `<Esc>`)

**Why no server module?**
The keymap data is already registered by existing modules via
`KeybindingRegistration`. The server just needs a query endpoint.
The popup is entirely a client rendering concern.

### 6.2 Command Line (#469)

Command line provides the `:`, `/`, and `?` prompts.

**Architecture: Server module (state) + Client extension (rendering)**

```
Server Side                          Client Side
===========                          ===========

[Existing] CmdlineState              [NEW] CmdlineExtension
  - SessionExtension                   - Renders prompt + input + cursor
  - Stores: active, prompt,           - Captures keyboard in cmdline mode
    input, cursor position            - Shows at bottom of screen
  - TextInputSink for char routing

[Existing] VimModule                 [NEW] CmdlineBridge (server)
  - EnterCommandLineMode               - ExtensionStateBridge impl
  - ExitCommandLineMode                - Snapshots CmdlineState to JSON
  - EnterSearchForward/Backward

[NEW] ExtensionService (gRPC)
  - GetState("cmdline")
  - Returns: {active, prompt, input, cursor}

[NEW] ExtensionUpdated notification
  - kind: "cmdline"
  - Pushed on every CmdlineState change
```

**Subscriptions:** `["mode.changed", "cmdline.updated"]`

**Data flow (`:wq<Enter>`):**
1. User presses `:` in normal mode
2. VimModule dispatches `EnterCommandLineMode`
3. CmdlineState.enter(Command) -- server state change
4. Server emits `mode.changed` notification (mode = "command_line")
5. Server emits `cmdline.updated` notification (active, prompt, input, cursor)
6. Router dispatches both to CmdlineExtension (subscribed to both topics)
7. CmdlineExtension.on_event("mode.changed", ...) -- self-activates, `is_active() = true`
8. CmdlineExtension.on_event("cmdline.updated", ...) -- renders prompt ":" at bottom
9. User types `w`: input routed to CmdlineState via TextInputSink
10. Server emits `cmdline.updated` with updated input
11. CmdlineExtension re-renders with ":wq" and cursor
12. User presses Enter: `ExitCommandLineMode` fires
13. Server processes `:wq`, emits `mode.changed` (normal)
14. CmdlineExtension.on_event("mode.changed", ...) -- self-deactivates, `is_active() = false`

Note: The engine never tells CmdlineExtension to activate or deactivate.
The extension self-manages its state based on the events it receives.

### 6.3 Completion (Future)

**Architecture: Server module + Client extension**

```
Server Side                          Client Side
===========                          ===========

[Future] CompletionModule            [Future] CompletionExtension
  - Collects completion sources        - Renders popup near cursor
  - Ranks/filters candidates           - Handles navigation (C-n, C-p)
  - Stores CompletionState             - Shows documentation preview
    as SessionExtension

[Future] CompletionState             TUI: Floating FrameBuffer popup
  - items: Vec<CompletionItem>       Web: DOM listbox with scroll
  - selected_index: usize
  - filter_text: String

[Future] CompletionBridge
  - ExtensionStateBridge impl
  - Snapshots items + selection
```

### 6.4 LSP Diagnostics (Future)

**Architecture: Server driver + Client extension**

```
Server Side                          Client Side
===========                          ===========

[Existing] LspDriver                 [Future] DiagnosticsExtension
  - Manages LSP connections            - Renders gutter signs
  - Receives diagnostics               - Renders inline hints
  - Stores in DiagnosticStore          - Renders diagnostic popup on hover

[Future] DiagnosticBridge            TUI: Gutter column + virtual text
  - ExtensionStateBridge impl        Web: DOM gutter + inline spans
  - Snapshots per-buffer diagnostics
```

### 6.5 File Explorer (Future)

**Architecture: Server module + Client extension**

```
Server Side                          Client Side
===========                          ===========

[Future] ExplorerModule              [Future] ExplorerExtension
  - Tree state (expanded nodes)        - Renders tree in side panel
  - VFS integration for listing        - Handles navigation
  - ExplorerState as SessionExt        - File open on Enter

[Future] ExplorerBridge              TUI: Panel composable (left side)
  - Snapshots tree state             Web: DOM tree component
```


## 7. Architecture Diagram

```
+===================================================================+
|                        CLIENT LAYER                                |
+===================================================================+
|                                                                    |
|  clients/tui/                          clients/web/                |
|  +------------------------------+     +---------------------------+|
|  | ENGINE (zero extension         |     | ENGINE (zero extension    ||
|  |         knowledge)             |     |         knowledge)        ||
|  | src/                          |     | src/                      ||
|  |   extension.rs  (trait only)  |     |   extensions/             ||
|  |   router.rs     (topic-based) |     |     types.ts  (interface) ||
|  |                               |     |     router.ts (topic-based||
|  | Only imports:                 |     |                           ||
|  |   create_extensions()         |     | Only imports:             ||
|  |     -> Vec<Box<dyn            |     |   createExtensions()      ||
|  |          ClientExtension>>    |     |     -> ClientExtension[]  ||
|  +------------------------------+     +---------------------------+|
|                                                                    |
|  EXTENSIONS (separate crates, engine never imports these)          |
|  +------------------------------+     +---------------------------+|
|  | extensions/                   |     | extensions/               ||
|  |   defaults/   (meta-crate)   |     |   defaults/  (meta-module)||
|  |   which-key/  (crate)        |     |   which-key/ (module)     ||
|  |   cmdline/    (crate)        |     |   cmdline/   (module)     ||
|  |   completion/ (crate)        |     |   completion/(module)     ||
|  +------------------------------+     +---------------------------+|
|  | lib/drivers/display/          |     | src/render/               ||
|  |   compositor/                 |     |   overlay.ts              ||
|  |     composable.rs             |     |   layout.ts               ||
|  |     layer.rs                  |     |   buffer.ts               ||
|  +------------------------------+     +---------------------------+|
|                                                                    |
+============================ gRPC ===================================+
|                                                                    |
|  shared/protocol/proto/reovim/v2/                                  |
|  +--------------------------------------------------------------+ |
|  | notification.proto   -- ExtensionUpdatedPayload (NEW)         | |
|  | extension.proto      -- ExtensionService (NEW)                | |
|  | keymap.proto         -- KeymapService (NEW)                   | |
|  | input.proto          -- InputService (existing)               | |
|  | state.proto          -- StateService (existing)               | |
|  | notification.proto   -- NotificationService (existing)        | |
|  +--------------------------------------------------------------+ |
|                                                                    |
+============================ Server ==================================+
|                                                                    |
|  shared/clients/model/              (Platform-Agnostic Types)      |
|  +--------------------------------------------------------------+ |
|  | wire/      -- LogicalOverlay, Anchor, OverlayState            | |
|  | rendered/  -- RenderedOverlay, WindowTree                     | |
|  | traits/    -- OverlayRenderer, OverlayManager                 | |
|  | interaction/ -- Interaction, InteractionResult                | |
|  +--------------------------------------------------------------+ |
|                                                                    |
|  server/lib/server/                 (Server Runtime)               |
|  +--------------------------------------------------------------+ |
|  | grpc/                                                         | |
|  |   extension.rs    -- ExtensionService handler (NEW)           | |
|  |   keymap.rs       -- KeymapService handler (NEW)              | |
|  |   input.rs        -- InputService handler (existing)          | |
|  |   state.rs        -- StateService handler (existing)          | |
|  +--------------------------------------------------------------+ |
|                                                                    |
|  server/modules/                    (Policy)                       |
|  +--------------------------------------------------------------+ |
|  | vim/       -- VimModule (existing, 19 modules)                | |
|  | editor/    -- EditorModule                                    | |
|  | commands/  -- CommandsModule                                  | |
|  | keymap/    -- KeymapModule                                    | |
|  | ...                                                           | |
|  +--------------------------------------------------------------+ |
|                                                                    |
|  server/lib/drivers/                (Mechanism)                    |
|  +--------------------------------------------------------------+ |
|  | session/   -- SessionExtension, ExtensionMap, CmdlineState    | |
|  |   bridges/ -- ExtensionStateBridge trait (NEW)                | |
|  |              CmdlineBridge, CompletionBridge (future)         | |
|  | input/     -- KeybindingStore, ResolverRegistry               | |
|  | command/   -- CommandHandler, CommandProvider                  | |
|  | buffer/    -- BufferOps                                       | |
|  | ...                                                           | |
|  +--------------------------------------------------------------+ |
|                                                                    |
|  server/lib/kernel/                 (Mechanism)                    |
|  +--------------------------------------------------------------+ |
|  | api/       -- Module trait, ServiceRegistry                   | |
|  | mm/        -- Buffer, BufferId                                | |
|  | ipc/       -- EventBus                                        | |
|  | core/      -- MotionEngine, RegisterBank, etc.                | |
|  +--------------------------------------------------------------+ |
|                                                                    |
+====================================================================+
```


## 8. Design Decisions and Tradeoffs

### 8.1 Why Not Server-Side Overlays?

The archived v0.8.x `WhichKeyPlugin` rendered directly in the server.
This violated the server/client separation established in Epic #465.

**Decision**: All rendering is client-side.

**Tradeoffs**:
- (+) Server remains a pure data provider
- (+) Each platform can render optimally
- (+) No rendering code in server tests
- (-) Duplicated rendering logic across platforms
- (-) More gRPC traffic for state sync

**Mitigation**: The `shared/clients/model/` crate provides platform-agnostic
logic (positioning, z-ordering, interaction handling) that all platforms share.
Only the final rendering step (terminal cells vs DOM nodes) is platform-specific.

### 8.2 Why JSON for Extension State?

Extension state is serialized as JSON over gRPC.

**Decision**: Use `string data` (JSON) in proto messages rather than
typed proto messages per extension.

**Tradeoffs**:
- (+) New extensions don't require proto changes
- (+) Schema evolution without breaking wire compat
- (+) Simpler server-side bridge implementation
- (-) No compile-time type safety on the wire
- (-) Parsing overhead (minimal for small payloads)

**Mitigation**: Each `ExtensionStateBridge` impl defines the JSON schema
in documentation. The client extension knows what shape to expect from
its paired server module.

### 8.3 Why Not a Plugin Trait in the Kernel?

We could add a `ClientPlugin` trait to the kernel, mirroring `Module`.

**Decision**: NO plugin trait in kernel. Client extensions are NOT kernel
concepts.

**Reasoning**:
- Kernel purity: kernel knows about server-side modules, not client UI
- Client independence: each client defines its own extension interface
- WASM compatibility: the web client cannot load `.so` files
- Platform divergence: TUI extensions work with `FrameBuffer`,
  web extensions work with DOM -- they share no rendering interface

Client extensions are a client-layer concept, defined by each client.

### 8.4 Why Push + Pull Instead of Pure Push?

Extensions use both push notifications (for real-time updates) and
pull queries (for initial state and on-demand refresh).

**Decision**: Dual communication mode.

**Reasoning**:
- Push alone requires tracking connection state for initial sync
- Pull alone has latency -- user sees stale state
- Push + Pull: client pulls on connect, then receives push updates
- This matches the existing notification model used by TUI and web

### 8.5 Why Compiled-In Extensions (Not Dynamic)?

TUI extensions are compiled into the client binary. Web extensions
are bundled at build time.

**Decision**: No dynamic client-side plugin loading (initially).

**Reasoning**:
- WASM dynamic loading is complex and fragile
- TUI `.so` plugins require a separate FFI boundary (complexity)
- The v0.8.x dynamic plugin system was removed for good reason
- Compiled-in extensions are simpler, faster, and fully type-safe

**Future**: If dynamic client extensions become needed, they can be
added as a second loading mechanism without changing the trait.


### 8.6 Why Game-Mod Separation (Defaults Meta-Crate)?

The engine could directly import and construct extensions:
```rust
// BAD: engine knows about specific extensions
use which_key::WhichKeyExtension;
use cmdline::CmdlineExtension;
let extensions = vec![
    Box::new(WhichKeyExtension::new()),
    Box::new(CmdlineExtension::new()),
];
```

**Decision**: Engine imports ONLY `create_extensions()` from a `defaults`
meta-crate. It never names or constructs individual extensions.

**Reasoning**:
- Mirrors server's `DefaultsModule` pattern -- consistency across layers
- Adding/removing extensions requires NO engine code changes
- Engine binary has a single dependency point, not N extension crates
- Prevents "just this one import" slippery slope into tight coupling
- Custom builds can swap `defaults` meta-crate for different extension sets

**Parallel to server architecture**:
- Server: `ModuleLoader` loads `.so` files, calls `reovim_module_probe()`
- Client: Engine calls `create_extensions()`, never knows what's inside
- Both: The aggregation layer (defaults) is the only place that lists modules/extensions

### 8.7 Why Subscription-Based Routing (Not Broadcast)?

Events could be broadcast to all extensions:
```rust
// BAD: every extension receives every event, wastes cycles
for ext in &mut extensions {
    ext.on_event(event);
}
```

Or routed by identity:
```rust
// BAD: engine knows extension identities, leaks coupling
for ext in &mut extensions {
    if ext.id() == event.topic().split('.').next().unwrap() {
        ext.on_event(event);
    }
}
```

**Decision**: Extensions declare `subscriptions()` at registration time.
Router builds `HashMap<topic, Vec<index>>` and dispatches only to
subscribers. Subscriptions return `Vec<String>` to allow config-dependent
topics, and `rebuild_routes()` supports runtime resubscription.

**Reasoning**:
- Extensions self-select which events matter -- engine is passive
- O(subscribers) dispatch, not O(all_extensions) broadcast
- Engine treats topics as opaque keys -- no string parsing or matching
- Mirrors kernel's `EventBus` pattern: `TypeId -> handlers` in kernel,
  `topic -> extensions` in client
- Extensions can subscribe to multiple topics, topics can have multiple
  subscribers -- pure many-to-many pub/sub
- Adding new event topics requires NO router changes
- `Vec<String>` allows config-dependent subscriptions (e.g., LSP topics
  only when LSP is enabled)
- `rebuild_routes()` allows dynamic resubscription at runtime

**Anti-coupling guarantee**: The engine code NEVER contains:
- `ext.id() == "something"` -- identity-based routing
- `match topic { "mode.changed" => ... }` -- topic interpretation
- `if ext.is::<SomeConcreteType>()` -- downcasting

The engine is a dumb pipe: `event -> routing table lookup -> deliver`.

### 8.8 Why Typed Events (Not Raw Bytes)?

Event payloads could be raw bytes:
```rust
// BAD: every extension deserializes manually, error-prone
fn on_event(&mut self, topic: &str, payload: &[u8]) {
    let state: CmdlineSnapshot = serde_json::from_slice(payload).unwrap();
}
```

**Decision**: Events use `ClientEvent` with type-erased `Box<dyn Any>`
payload. Extensions use `event.downcast_ref::<T>()` to recover typed data.

**Reasoning**:
- Router deserializes gRPC bytes ONCE, not once-per-extension
- Extensions get type-safe payloads (compiler catches type mismatches)
- Mirrors kernel's EventBus: handlers receive `&BufferChanged`, not `&[u8]`
- No serialization format knowledge leaks into extensions
- `downcast_ref()` returns `Option<&T>` -- safe, no panics on mismatch

**Deserialization ownership:**
```
gRPC bytes → DeserializerRegistry (topic lookup) → ClientEvent → Extensions (downcast)
```

The router owns a `DeserializerRegistry`: a `HashMap<String, DeserializerFn>`
mapping topic strings to deserialization functions. Each function converts
`&[u8]` into `Box<dyn Any + Send + Sync>`. The registry is provided by the
`defaults` meta-crate alongside extensions:

```rust
// clients/tui/extensions/defaults/src/lib.rs
pub fn create_deserializers() -> DeserializerRegistry {
    let mut reg = DeserializerRegistry::new();
    reg.register::<ModeChangedPayload>(topics::MODE_CHANGED, |bytes| {
        serde_json::from_slice(bytes).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    });
    reg.register::<CmdlineUpdatedPayload>(topics::CMDLINE_UPDATED, |bytes| {
        serde_json::from_slice(bytes).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    });
    reg
}
```

This keeps deserialization knowledge in the defaults meta-crate (alongside
extension registration), not in the engine.

### 8.9 Why Bridge Methods (Not Sub-Traits or God Trait)?

Three approaches were considered for optional capabilities:

```rust
// OPTION 1 (rejected): God trait -- forces ALL Composable methods on all extensions
pub trait ClientExtension: Composable { /* ... */ }

// OPTION 2 (rejected): Sub-traits -- can't downcast dyn ClientExtension to dyn Composable
pub trait RenderableExtension: ClientExtension + Composable {}
// Router stores Vec<Box<dyn ClientExtension>>, can't recover dyn Composable

// OPTION 3 (chosen): Bridge methods -- opt-in capability via self-reference
pub trait ClientExtension {
    fn composable(&self) -> Option<&dyn Composable> { None }
    fn wants_input(&self) -> bool { false }
    fn handle_key(&mut self, key: &KeyEvent) -> bool { false }
}
```

**Decision**: Single `ClientExtension` trait with bridge methods that
return `Option<&dyn Composable>`. Extensions that render override
`composable()` to return `Some(self)`.

**Why Option 2 (sub-traits) was rejected**:
- Rust's trait objects don't support upcasting/downcasting between traits
- `Box<dyn ClientExtension>` cannot be cast to `Box<dyn Composable>`
- Would require `enum Extension { Bg(Box<dyn ClientExtension>), Ui(Box<dyn RenderableExtension>) }`
  which is a combinatorial explosion
- Or `Any`-based downcasting which is fragile

**Why bridge methods work**:
- Extensions implement `Composable` on their CONCRETE type
- `composable()` returns `Some(self)` -- a self-reference that the
  compiler can verify (concrete type implements both traits)
- Router calls `ext.composable()` -- returns `Option<&dyn Composable>`
  without knowing the concrete type
- Same pattern as `std::error::Error::source()` returning `Option<&dyn Error>`

**Result**:
- Non-rendering extensions: don't override `composable()`, get `None` default
- Rendering extensions: implement `Composable` + override `composable()` → `Some(self)`
- No stub implementations, no trait downcasting, compiles cleanly


## 9. Implementation Phases

### Phase 1: Foundation (Extension Bridge + Protocol)

1. Define `ExtensionStateBridge` trait in `server/lib/drivers/session/src/bridges/`
2. Add `extension.proto` with `ExtensionService`
3. Add `ExtensionUpdatedPayload` to `notification.proto`
4. Implement `CmdlineBridge` as first concrete bridge
5. Add gRPC handler for `ExtensionService`
6. Emit `ExtensionUpdated` notifications when CmdlineState changes

### Phase 2: TUI Client Extensions

1. Define `ClientEvent`, `DeserializerRegistry`, and `ClientExtension` trait in `clients/tui/src/extension.rs`
2. Implement `ExtensionRouter` with subscription-based dispatch in `clients/tui/src/router.rs`
3. Create `defaults` meta-crate in `clients/tui/extensions/defaults/`
4. Integrate `ExtensionRouter` with TUI event loop (engine calls `create_extensions()` + `Router::new()`)
5. Wire gRPC notification handler to call `router.dispatch_raw(topic, bytes)`
6. Implement `CmdlineExtension` as first extension crate (#469)

### Phase 3: Which-Key (#468)

1. Add `keymap.proto` with `KeymapService`
2. Implement `KeymapService` gRPC handler (queries KeybindingStore)
3. Implement `WhichKeyExtension` for TUI
4. Wire timeout-based activation into TUI input loop
5. Implement `WhichKeyExtension` for web client

### Phase 4: Web Client Extensions

1. Define `ClientExtension` TypeScript interface
2. Implement `ExtensionRouter` for web (same subscription-based dispatch)
3. Create `defaults` meta-module with `createExtensions()`
4. Integrate with web client notification handler
5. Port `CmdlineExtension` to web
6. Port `WhichKeyExtension` to web

### Phase 5: Completion (Future)

1. `CompletionModule` server module
2. `CompletionState` session extension
3. `CompletionBridge` server bridge
4. `CompletionExtension` for TUI and web


## 10. Relationship to Existing Systems

### 10.1 How Client Extensions Relate to Composable

`ClientExtension` is the single trait for all extensions. `Composable`
is a separate trait that rendering extensions implement on their
concrete type. The connection is via the `composable()` bridge method:

```
ClientExtension (all extensions implement this)
  ├── composable() -> None          (non-rendering: bg sync, macro recorder)
  └── composable() -> Some(self)    (rendering: which-key, cmdline, completion)
        └── self also implements Composable (6 required + 2 default methods)
```

`Composable` has 8 methods (from `clients/tui/lib/drivers/display/`):
- **Required (6)**: `id`, `z_order`, `set_z_order`, `is_visible`, `bounds`, `render`
- **Default (2)**: `captures_keyboard` (true), `cursor_position` (None)

Extensions that return `Some(self)` from `composable()` participate in
the compositor (z-order rendering, hit-testing, cursor ownership).
Extensions that return `None` have zero rendering footprint.

**`captures_keyboard()` vs `wants_input()` -- two different layers:**

These two methods serve different purposes and live in different traits:

| Method | Trait | Default | Purpose |
|--------|-------|---------|---------|
| `captures_keyboard()` | `Composable` | `true` | Compositor-level: prevents keystrokes from reaching layers beneath this composable |
| `wants_input()` | `ClientExtension` | `false` | Router-level: opts this extension into the `input_target()` selection |

The relationship: `wants_input()` is a PREREQUISITE for receiving keys at
the extension router level. `captures_keyboard()` determines whether the
compositor stops propagating keys AFTER the extension has consumed them.
An extension that returns `wants_input() = true` but `captures_keyboard() = false`
would receive keys but allow them to also reach lower z-order composables.

Typical patterns:
- **cmdline** (modal): `wants_input() = true` + `captures_keyboard() = true` (exclusive input)
- **completion popup**: `wants_input() = true` + `captures_keyboard() = true` (C-n/C-p captured)
- **status indicator**: `wants_input() = false` + `captures_keyboard() = false` (display only)
- **background sync**: `wants_input() = false` (no composable, no input)

The engine discovers renderables via `router.active_composables()` which
calls `ext.composable()` on each active extension. No trait downcasting,
no enum matching, no identity checks.

### 10.2 How ExtensionStateBridge Relates to SessionExtension

`SessionExtension` is the mechanism: type-erased state storage in `ExtensionMap`.
`ExtensionStateBridge` is the adapter: serializes extension state to JSON
for gRPC transmission. Both live in the driver layer (`server/lib/drivers/session/`)
because bridges adapt driver-layer state.

Each bridge declares its `ExtensionScope` (Shared or Client). The runner
passes the correct `ExtensionMap` based on scope:

```
SessionExtension (mechanism)     ExtensionStateBridge (adapter)
  |                                |
  | ExtensionMap stores state      | Reads from ExtensionMap
  | Modules read/write directly    | scope() declares which map
  | In-process, type-safe          | Cross-process, JSON-encoded
  |                                |
  v                                v
CmdlineState                     CmdlineBridge
  active: bool                     scope() = Client (per-client)
  prompt: CmdlinePrompt            snapshot(client_ext) -> json!({
  input: String                       "active": true,
  cursor: usize                       "prompt": ":",
  (in client_extensions)              "input": "wq",
                                      "cursor": 2
                                   })
```

### 10.3 How LogicalOverlay Relates to ClientExtension

`LogicalOverlay` from `shared/clients/model/` is a generic overlay
descriptor. Client extensions that produce popup-style UI can use
`LogicalOverlay` as their data model, but it is NOT required.

- `LogicalOverlay`: generic overlay with JSON data and anchor
- `ClientExtension`: specific, typed extension with known behavior

For simple overlays (hover, tooltip), `LogicalOverlay` + `OverlayRenderer`
is sufficient. For complex interactive UI (cmdline, completion, which-key),
a dedicated `ClientExtension` provides better type safety and interaction
handling.

### 10.4 Backward Compatibility with 19 Existing Modules

All 19 existing server modules continue to work unchanged:

- `Module` trait is untouched
- `declare_module!` is untouched
- `ServiceRegistry` is untouched
- `ExtensionMap` is untouched

The new work adds:
- `ExtensionStateBridge` (opt-in, no existing module needs it yet)
- New gRPC services (additive, no breaking changes)
- New notification type (additive)
- Client-side extension infrastructure (client-only, no server impact)


## 11. Summary: Decision Matrix

| Question | Answer |
|----------|--------|
| Where does rendering happen? | Client only (TUI, web) |
| Where does state live? | Server (SessionExtension) |
| How do clients learn state? | Push (notifications) + Pull (gRPC) |
| Are client extensions dynamic? | No (compiled-in for now) |
| Is there a kernel Plugin trait? | No (kernel purity) |
| How is state serialized? | JSON via ExtensionStateBridge |
| Do all features need both halves? | No (see taxonomy in section 1.3) |
| How do platforms share code? | shared/clients/model/ crate |
| Is this backward compatible? | Yes (all additive changes) |
| WASM compatible? | Yes (no .so loading, JSON wire format) |
| How does the engine know extensions? | Via defaults meta-crate ONLY |
| How are events dispatched? | Subscription-based routing table |
| Does engine match on ext identity? | NEVER (anti-coupling invariant) |
| Can extensions be added without engine changes? | Yes (add crate + register in defaults) |
| Must all extensions render UI? | No (only those with composable() -> Some) |
| Who deserializes gRPC bytes? | Router (once), extensions get typed ClientEvent |
| Can subscriptions change at runtime? | Yes (Vec\<String\> + rebuild_routes()) |
| Where do bridges live? | Driver layer (server/lib/drivers/session/) |
| Which ExtensionMap do bridges read? | Declared via scope() -- Shared or Client |


## 12. File Inventory

New files to create:

```
shared/protocol/proto/reovim/v2/
  extension.proto                    -- ExtensionService
  keymap.proto                       -- KeymapService

server/lib/drivers/session/src/
  bridges/
    mod.rs                           -- ExtensionStateBridge trait
    cmdline.rs                       -- CmdlineBridge

server/lib/server/src/
  grpc/
    extension.rs                     -- ExtensionService handler
    keymap.rs                        -- KeymapService handler

clients/tui/
  src/
    extension.rs                     -- ClientExtension trait, ClientEvent, DeserializerRegistry
    router.rs                        -- ExtensionRouter (subscription-based dispatch)
  extensions/                        -- SEPARATE CRATES (not in src/)
    defaults/
      Cargo.toml                     -- depends on which-key, cmdline, etc.
      src/lib.rs                     -- create_extensions() + create_deserializers()
    which-key/
      Cargo.toml                     -- standalone crate
      src/lib.rs                     -- WhichKeyExtension
    cmdline/
      Cargo.toml                     -- standalone crate
      src/lib.rs                     -- CmdlineExtension

clients/web/src/
  extensions/
    types.ts                         -- ClientExtension interface
    router.ts                        -- ExtensionRouter (subscription-based)
    defaults/
      index.ts                       -- export function createExtensions(): ClientExtension[]
    which-key/
      index.ts                       -- WhichKeyExtension
    cmdline/
      index.ts                       -- CmdlineExtension
```

Files to modify:

```
shared/protocol/proto/reovim/v2/
  notification.proto                 -- Add ExtensionUpdatedPayload

server/lib/server/src/
  grpc/mod.rs                        -- Register new services
  session/                           -- Emit extension_updated notifications

clients/tui/
  Cargo.toml                         -- Depend on tui-ext-defaults (NOT individual extensions)
```

**Dependency graph (TUI):**
```
clients/tui (engine)
  └── clients/tui/extensions/defaults (meta-crate)
        ├── clients/tui/extensions/which-key
        ├── clients/tui/extensions/cmdline
        └── clients/tui/extensions/completion (future)
```

The engine crate depends on `defaults` only. Individual extension crates
are dependencies of `defaults`, invisible to the engine.

No files in kernel, drivers, or existing modules need modification.
