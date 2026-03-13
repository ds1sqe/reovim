# Data Flow

## Server -> Modules (state changes)

```
gRPC stream
    |
    v
CORE: decode notification
    |
    ├── ExtensionUpdated { kind, data }
    |       -> route to module where module.kind() == kind:
    |          module.on_notification(data)
    |
    ├── OptionChanged { name, value }
    |       -> broadcast to ALL: module.on_option_changed(name, value)
    |          (modules filter internally — CORE does not match names)
    |
    ├── BufferContent { buffer_id, changes }
    |       -> broadcast: module.on_buffer_update(event)
    |          event carries incremental changes (see BufferUpdateEvent)
    |
    ├── CursorMoved { buffer_id, line, col }
    |       -> broadcast: module.on_cursor_update(buffer_id, line, col)
    |
    ├── BufferFocusChanged { buffer_id }
    |       -> broadcast: module.on_buffer_focus(buffer_id)
    |
    ├── ModeChanged { mode }
    |       -> broadcast: module.on_mode_change(mode)
    |       -> CORE caches mode to replay current state to modules that
    |          initialize after session start (late-joining modules).
    |          CORE never uses the cached mode for rendering decisions.
    |
    └── ThemeChanged { theme }
            -> broadcast: module.on_theme_changed(theme)
```

`kind()` vs `id()`: CORE routes `ExtensionUpdated` by matching `kind` against
each module's `kind()` method. `kind()` defaults to `id()`. Override when
the server kind string differs from the module's identity.

## Module -> Server (commands, queries)

Modules need ongoing server access. An explorer opens files, a cmdline submits
commands, completion queries LSP.

```rust
/// Handle for server communication. Defined in CLIENT DRIVER.
/// Modules receive Arc<dyn ServerHandle> at init and store it.
trait ServerHandle: Send + Sync {
    /// Query options. Blocking — call during init/on_all_loaded, not render.
    fn get_options(&self, names: &[&str]) -> Vec<(String, OptionValue)>;

    /// Get buffer metadata. Blocking.
    fn get_buffer_metadata(&self, buffer_id: BufferId) -> Option<BufferMetadata>;

    /// Send an ex-command to the server. Fire-and-forget.
    /// Results arrive via on_notification() routed by kind.
    fn execute_command(&self, command: &str);

    /// Send synthetic keys to the server.
    fn send_keys(&self, keys: &[KeyEvent]);
}
```

### Fire-and-forget model

`execute_command` does not return a result. This is intentional:

1. The gRPC protocol is already notification-based. The server processes
   the command and sends result notifications through the event stream.
2. Modules that need results watch for them in `on_notification()`.
3. This avoids blocking the event loop on synchronous RPC calls.
4. It matches the existing server architecture where command results are
   dispatched as events, not return values.

`get_options` and `get_buffer_metadata` ARE blocking because they are
expected to be called at init/on_all_loaded time, not during rendering.

## Module -> Screen (rendering)

See [Rendering](rendering.md) for the full compositor flow.

## Input -> Server

```
Platform input (keyboard, mouse, touch)
    |
    v
PLATFORM ADAPTER: translate to InputEvent, send to channel
    |
    v
CORE: receive from InputSource channel
    |
    v
CORE: forward to server via gRPC send_keys()
```

Modules do NOT intercept input. The server decides what keys mean.

## Module <-> Module (cross-module communication)

Modules cannot import each other. They communicate through the **ServiceRegistry**
(pull-based lookup). Push-based cross-module events are NOT supported by design.

```
git-signs module (init):
    let providers = ctx.services.get_or_create::<ComponentProviderRegistry>();
    providers.register(
        ComponentProviderKey::new("git-branch"),
        Arc::new(GitBranchProvider { state: Arc::clone(&self.state) }),
    );

statusline module (rebuild_label, Phase 1):
    if let Some(provider) = providers.get(&ComponentProviderKey::new("git-branch")) {
        let output = provider.render();
    }
    // if git-signs not loaded -> returns None -> graceful degradation
```

See [Examples](examples.md) for the full worked example with safe shared
state (no raw pointers).

Why no push-based cross-module events:
- Keeps the model simple — no event cycles, no ordering dependencies
- Every module reacts to server events and queries registries at render time
- Module A's state change affects Module B automatically at next render
  (B reads A's latest state from registry)
