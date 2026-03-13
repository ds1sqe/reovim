# ClientModule Trait

The single trait that CORE interacts with. CORE holds `Vec<Box<dyn ClientModule>>`.

Organized into method groups. Each group is independent. A module only overrides
the groups it needs. All methods have default no-op implementations.

## Why one trait

A statusline that needs both chrome rendering AND event reception is ONE object
in ONE Vec. No Arc, no Mutex, no shared ownership. The two-phase contract is
enforced structurally — event methods take `&mut self`, render methods take `&self`.
The phases are sequential in the event loop, never concurrent.

**Single-task ownership model**: CORE runs all module interactions within a
single async task. Phase 1 (events, `&mut self`) and Phase 2 (render, `&self`)
never overlap because they are sequential steps within one task's event loop
iteration. `ClientModule: Send + Sync` is required for the module to be owned
by the async runtime, but no concurrent access occurs. If an implementer
splits CORE into multiple tasks, the borrow discipline breaks — this is why
all module dispatch MUST stay in one task.

This is NOT a god-trait. It is a **role-based interface with explicit opt-in**.
A line-numbers module overrides `has_annotations()` + 3 annotation methods.
A statusline overrides `has_chrome()` + 4 chrome methods + event handlers.
Each module touches only its role's methods.

## The Trait

```rust
trait ClientModule: Send + Sync + 'static {

    // ====================================================================
    // Identity
    // ====================================================================

    /// Unique identifier. Kebab-case: "statusline", "line-numbers".
    /// Returns &'static str — module identifiers are always string literals.
    fn id(&self) -> &'static str;

    /// Notification routing key. Matches ExtensionUpdated { kind } from server.
    /// Defaults to id(). Override when the server kind differs.
    fn kind(&self) -> &'static str { self.id() }

    fn name(&self) -> &'static str;
    fn version(&self) -> Version;

    /// Required dependencies (must be loaded before this module).
    fn dependencies(&self) -> &[&str] { &[] }

    /// Optional dependencies (load before if available, don't fail if missing).
    fn optional_dependencies(&self) -> &[&str] { &[] }

    /// Server-side kinds this module expects to receive notifications for.
    /// Defaults to `vec![self.kind()]`. Override for 1:N client-to-server
    /// kind mapping. CORE validates at startup that the server has matching
    /// modules loaded, logging warnings for unmatched kinds.
    fn server_kinds(&self) -> Vec<&'static str> { vec![self.kind()] }

    // ====================================================================
    // Lifecycle
    // ====================================================================

    /// Initialize module.
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult;

    /// Cleanup. Called in reverse dependency order during shutdown.
    fn exit(&mut self) -> Result<(), ClientModuleError>;

    /// Called after ALL modules have completed init().
    /// Safe to query ServiceRegistry for services from other modules.
    fn on_all_loaded(&mut self, _ctx: &ModuleContext) {}

    // ====================================================================
    // Role Declaration
    // ====================================================================
    //
    // CORE checks these EVERY FRAME (cheap bool). This enables runtime
    // activation/dormancy — a capability-gated module can return true
    // after on_capabilities_changed delivers a newly available capability.

    /// This module contributes chrome — fixed UI regions around the buffer
    /// viewport (statusline, cmdline, sidebar, overlay, etc).
    /// Chrome is allocated from screen edges inward by CORE's compositor.
    fn has_chrome(&self) -> bool { false }

    /// This module contributes to the buffer viewport.
    fn has_buffer_contrib(&self) -> bool { false }

    /// This module contributes gutter annotations.
    fn has_annotations(&self) -> bool { false }

    // ====================================================================
    // Events — Phase 1 (data collection, &mut self)
    // ====================================================================
    //
    // Called on state changes. Modules update internal caches.
    // Allocation permitted. O(changes), not O(frames).

    /// Server notification routed by kind(). Only called if kind() matches.
    fn on_notification(&mut self, _data: &str) {}

    /// Option changed. Broadcast to ALL modules. Modules filter internally.
    fn on_option_changed(&mut self, _name: &str, _value: &OptionValue) {}

    /// Buffer content changed (incremental).
    fn on_buffer_update(&mut self, _event: &BufferUpdateEvent) {}

    /// Cursor moved.
    fn on_cursor_update(&mut self, _buffer_id: BufferId, _line: usize, _col: usize) {}

    /// Active buffer changed. Fired when user switches to a different buffer.
    fn on_buffer_focus(&mut self, _buffer_id: BufferId) {}

    /// Editor mode changed.
    fn on_mode_change(&mut self, _mode: &str) {}

    /// Platform capabilities changed (resize, focus, dark mode, etc).
    /// This is the SINGLE event for all capability changes.
    /// Modules that check capabilities at init should re-evaluate here.
    fn on_capabilities_changed(&mut self, _caps: &dyn PlatformCapabilities) {}

    /// Theme changed.
    fn on_theme_changed(&mut self, _theme: &dyn ThemeProvider) {}

    /// Per-frame tick. Must be O(1). Return true if visual output changed.
    fn tick(&mut self) -> bool { false }

    // ====================================================================
    // Chrome — Phase 2 (render, &self)
    // ====================================================================
    //
    // Only called when has_chrome() returns true.

    fn chrome_position(&self) -> ChromePosition { ChromePosition::Bottom }
    fn chrome_requested_size(&self, _caps: &dyn PlatformCapabilities) -> u16 { 1 }
    fn chrome_priority(&self) -> u16 { 0 }
    fn chrome_z_order(&self) -> u16 { 0 }  // for Overlay, higher = on top

    fn chrome_render(
        &self,
        _surface: &mut dyn RenderSurface,
        _bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {}

    // ====================================================================
    // Buffer Contribution — Phase 2 (render, &self)
    // ====================================================================
    //
    // Only called when has_buffer_contrib() returns true.
    // ViewportRenderer orchestrates these calls.

    /// Priority for classify_token merge. Higher wins. See rendering.md.
    fn buffer_contrib_priority(&self) -> u16 { 0 }

    fn classify_token(&self, _category: &str) -> Option<RenderBehavior> { None }
    fn transform_line(&self, _buf: BufferId, _line: usize, _text: &str)
        -> Option<TransformedLine> { None }
    fn map_cursor_column(&self, _buf: BufferId, _line: usize, _col: usize)
        -> Option<u16> { None }
    fn fold_ranges(&self) -> &[(usize, usize)] { &[] }
    fn virtual_lines(&self) -> &[VirtualLine] { &[] }
    fn inline_decorations(&self, _line: usize) -> &[InlineDecoration] { &[] }
    fn cursor_position(&self, _w: u16, _h: u16) -> Option<(u16, u16)> { None }

    // ====================================================================
    // Annotations — Phase 2 (render, &self)
    // ====================================================================
    //
    // Only called when has_annotations() returns true.
    // The Gutter framework (inside ViewportRenderer) orchestrates these.

    fn annotation_column_width(
        &self,
        _ctx: &AnnotationContext,
        _caps: &dyn PlatformCapabilities,
    ) -> ColumnWidth { ColumnWidth::Fixed(0) }

    fn annotate(&self, _line: usize, _ctx: &AnnotationContext)
        -> Option<GutterCell> { None }

    fn annotation_priority(&self) -> u16 { 0 }  // column ordering
}
```

## Module Init Context

```rust
struct ModuleContext<'a> {
    /// Cross-module service registry.
    pub services: &'a ServiceRegistry,

    /// Platform capabilities — the ground truth.
    pub capabilities: &'a dyn PlatformCapabilities,

    /// Handle for ongoing server communication. Clone and store for later use.
    pub server: Arc<dyn ServerHandle>,

    /// Active theme.
    pub theme: &'a dyn ThemeProvider,
}
```

## Lifecycle

```
Startup:
    1. Platform adapter starts, declares PlatformCapabilities
    2. Platform binary registers modules via register_modules()
    3. CORE sorts by dependency graph (topological order)
    4. CORE calls module.init(ctx) in dependency order
       - If init() returns Defer: CORE queues the module for retry
       - After all modules have been attempted, CORE retries deferred modules
         (up to 3 rounds). If a module still defers after all rounds, it is
         treated as Failed and logged. Circular Defer is detected by checking
         if zero modules succeeded in a round (no progress = give up).
    5. Modules query capabilities, decide behavior, register services
    6. CORE calls module.on_all_loaded(ctx) on all modules
       (safe to query ServiceRegistry for other modules' services)
    7. CORE enters event loop
    8. First frame: CORE renders immediately after entering the event loop.
       Modules that have not yet received any on_* events render with their
       default/empty state (empty label, zero lines, etc). This produces a
       valid frame — the first server events arrive shortly after and trigger
       a re-render with real data.

Runtime events:
    on_capabilities_changed(caps) — resize, focus, dark mode, rotation
    on_theme_changed(theme)       — user switched colorscheme
    on_buffer_focus(buffer_id)    — user switched active buffer

Shutdown:
    1. CORE calls module.exit() in reverse dependency order
    2. CORE drops all modules
```
