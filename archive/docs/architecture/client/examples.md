# Worked Examples

## Statusline Module

A chrome module that shows mode, filename, cursor position.
Demonstrates Phase 1 caching + Phase 2 read-only rendering.

```rust
struct StatuslineModule {
    mode: String,
    filename: Option<String>,
    cursor: (usize, usize),
    mode_style: Style,
    status_style: Style,
    cached_label: String,  // pre-built in Phase 1
    server: Option<Arc<dyn ServerHandle>>,
}

impl ClientModule for StatuslineModule {
    fn id(&self) -> &'static str { "statusline" }
    fn name(&self) -> &'static str { "Statusline" }
    fn version(&self) -> Version { Version::new(1, 0, 0) }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        self.mode_style = ctx.theme.highlight("StatusLineMode");
        self.status_style = ctx.theme.highlight("StatusLine");
        self.server = Some(ctx.server.clone());
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> { Ok(()) }

    // --- Roles ---
    fn has_chrome(&self) -> bool { true }

    // --- Events (Phase 1: &mut self) ---
    fn on_mode_change(&mut self, mode: &str) {
        self.mode = mode.to_string();
        self.rebuild_label();
    }
    fn on_cursor_update(&mut self, _buf: BufferId, line: usize, col: usize) {
        self.cursor = (line, col);
        self.rebuild_label();
    }
    fn on_theme_changed(&mut self, theme: &dyn ThemeProvider) {
        self.mode_style = theme.highlight("StatusLineMode");
        self.status_style = theme.highlight("StatusLine");
    }

    // --- Chrome (Phase 2: &self) ---
    fn chrome_position(&self) -> ChromePosition { ChromePosition::Bottom }
    fn chrome_priority(&self) -> u16 { 100 }

    fn chrome_requested_size(&self, caps: &dyn PlatformCapabilities) -> u16 {
        if caps.safe_area().bottom > 0 { 2 } else { 1 }
    }

    fn chrome_render(&self, surface: &mut dyn ChromeSurface, bounds: Rect,
                     _caps: &dyn PlatformCapabilities) {
        surface.fill(bounds, ' ', self.status_style);
        // No allocation — reads pre-built cached_label
        surface.write_styled(bounds.x, bounds.y, &self.cached_label,
                            self.mode_style);
    }
}

impl StatuslineModule {
    fn rebuild_label(&mut self) {
        // Phase 1: allocation OK
        self.cached_label = format!(" {} | {} | {}:{} ",
            self.mode,
            self.filename.as_deref().unwrap_or("[No Name]"),
            self.cursor.0, self.cursor.1);
    }
}
```

Result: TUI 1-row, Web 1-row, Mobile 2-row (safe area). Same crate.

## Line Numbers Module

An annotation module. Demonstrates gutter contribution.

```rust
struct LineNumbersModule {
    total_lines: usize,
    cursor_line: usize,
    relative: bool,
    col_width: u16,
    style: Style,
    cursor_style: Style,
}

impl ClientModule for LineNumbersModule {
    fn id(&self) -> &'static str { "line-numbers" }
    fn name(&self) -> &'static str { "Line Numbers" }
    fn version(&self) -> Version { Version::new(1, 0, 0) }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        self.style = ctx.theme.highlight("LineNr");
        self.cursor_style = ctx.theme.highlight("CursorLineNr");
        let opts = ctx.server.get_options(&["number", "relativenumber"]);
        // parse options...
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> { Ok(()) }

    // --- Roles ---
    fn has_annotations(&self) -> bool { true }

    // --- Events ---
    fn on_buffer_update(&mut self, event: &BufferUpdateEvent) {
        self.total_lines = event.total_lines;
        self.recompute_width();
    }
    fn on_cursor_update(&mut self, _buf: BufferId, line: usize, _col: usize) {
        self.cursor_line = line;
    }
    fn on_option_changed(&mut self, name: &str, value: &OptionValue) {
        match name {
            "number" | "relativenumber" => { /* update self.relative */ }
            _ => {}
        }
    }

    // --- Annotations ---
    fn annotation_column_width(
        &self, _ctx: &AnnotationContext, caps: &dyn PlatformCapabilities,
    ) -> ColumnWidth {
        // Adapt: hide on very narrow terminals
        if let Some((cols, _)) = caps.grid_size() {
            if cols < 40 { return ColumnWidth::Fixed(0); }
        }
        ColumnWidth::Dynamic(self.col_width)
    }

    fn annotate(&self, line: usize, _ctx: &AnnotationContext)
        -> Option<GutterCell> {
        let (text, style) = if line == self.cursor_line {
            (format!("{:>w$}", line + 1, w = self.col_width as usize),
             self.cursor_style)
        } else if self.relative {
            let rel = (line as isize - self.cursor_line as isize).unsigned_abs();
            (format!("{:>w$}", rel, w = self.col_width as usize),
             self.style)
        } else {
            (format!("{:>w$}", line + 1, w = self.col_width as usize),
             self.style)
        };
        Some(GutterCell { text, style })
    }

    fn annotation_priority(&self) -> u16 { 100 }  // leftmost column
}

impl LineNumbersModule {
    fn recompute_width(&mut self) {
        self.col_width = ((self.total_lines as f64).log10().floor() as u16) + 2;
    }
}
```

## Touch Scroll (Capability-Gated)

Demonstrates runtime activation/dormancy via `on_capabilities_changed`.

```rust
struct TouchScrollModule {
    active: bool,
}

impl ClientModule for TouchScrollModule {
    fn id(&self) -> &'static str { "touch-scroll" }
    fn name(&self) -> &'static str { "Touch Scroll" }
    fn version(&self) -> Version { Version::new(1, 0, 0) }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        self.active = ctx.capabilities.touch_input();
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> { Ok(()) }

    // Role checked every frame — becomes true when touch becomes available
    fn has_buffer_contrib(&self) -> bool { self.active }

    fn on_capabilities_changed(&mut self, caps: &dyn PlatformCapabilities) {
        self.active = caps.touch_input();
    }

    // ... buffer contrib methods for scroll adjustment
}
```

Not a "mobile module." A capability-gated module. If a TUI terminal
supports touch, this module activates there too.

## Cross-Module Communication (ServiceRegistry)

Demonstrates the pull-based pattern. git-signs registers a service,
statusline consumes it. Neither imports the other.

```rust
// --- Shared types (defined in CLIENT DRIVER, see 07-types.md) ---

// ComponentProvider trait and ComponentProviderKey are in client-driver.
// ServiceRegistry is re-exported from reovim-kernel.
// ComponentProviderRegistry is a typed sub-registry inside ServiceRegistry.

// --- git-signs module: PRODUCER ---

/// Shared state between the module and its provider.
/// The provider owns the data via Arc — no raw pointers, no aliasing.
struct GitBranchState {
    branch: RwLock<String>,
}

/// The provider reads from shared state. It is an independent object —
/// NOT a pointer back into the module. Safe even if the module is faulted.
struct GitBranchProvider {
    state: Arc<GitBranchState>,
}

impl ComponentProvider for GitBranchProvider {
    fn render(&self) -> Option<Cow<'static, str>> {
        let branch = self.state.branch.read().unwrap();
        if branch.is_empty() {
            None
        } else {
            Some(Cow::Owned(format!(" {}", *branch)))
        }
    }
}

struct GitSignsModule {
    state: Arc<GitBranchState>,  // shared with provider
    stats: (usize, usize, usize),
}

impl GitSignsModule {
    fn new() -> Self {
        Self {
            state: Arc::new(GitBranchState {
                branch: RwLock::new(String::new()),
            }),
            stats: (0, 0, 0),
        }
    }
}

impl ClientModule for GitSignsModule {
    fn id(&self) -> &'static str { "git-signs" }
    fn name(&self) -> &'static str { "Git Signs" }
    fn version(&self) -> Version { Version::new(1, 0, 0) }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register a provider that shares state with this module.
        // The provider owns the Arc — it stays valid even if
        // this module is faulted and dropped by CORE.
        let providers = ctx.services.get_or_create::<ComponentProviderRegistry>();
        providers.register(
            ComponentProviderKey::new("git-branch"),
            Arc::new(GitBranchProvider { state: Arc::clone(&self.state) }),
        );
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> { Ok(()) }

    fn has_annotations(&self) -> bool { true }

    fn on_notification(&mut self, data: &str) {
        // Parse git status from server module, update shared state.
        // Phase 1: &mut self, allocation OK.
        let mut branch = self.state.branch.write().unwrap();
        *branch = "main".to_string(); // parsed from data
    }

    // ... annotation methods for +/- signs in gutter
}

// --- statusline module: CONSUMER ---

struct StatuslineModule {
    mode: String,
    filename: Option<String>,
    cursor: (usize, usize),
    mode_style: Style,
    status_style: Style,
    cached_label: String,
    server: Option<Arc<dyn ServerHandle>>,
    providers: Option<Arc<ComponentProviderRegistry>>,  // populated in on_all_loaded
}

// (init, chrome methods, event handlers shown in the first statusline example above)

impl ClientModule for StatuslineModule {
    // ... id, name, version, init, exit, chrome methods as shown above ...

    fn on_all_loaded(&mut self, ctx: &ModuleContext) {
        // Safe to query ServiceRegistry now — all producers have registered.
        self.providers = ctx.services.get::<ComponentProviderRegistry>();
    }

    // ... event handlers ...
}

impl StatuslineModule {
    fn rebuild_label(&mut self) {
        let branch_text = self.providers.as_ref()
            .and_then(|p| p.get(&ComponentProviderKey::new("git-branch")))
            .and_then(|provider| provider.render())
            .unwrap_or(Cow::Borrowed(""));
        // If git-signs not loaded -> empty string -> graceful degradation

        self.cached_label = format!(" {} | {} | {}:{} {}",
            self.mode,
            self.filename.as_deref().unwrap_or("[No Name]"),
            self.cursor.0, self.cursor.1,
            branch_text);
    }
}
```

Key properties:
- **No raw pointers**: Provider owns shared state via `Arc<GitBranchState>`,
  never aliases CORE's module allocation
- **Fault-safe**: If CORE drops git-signs after a panic, the Arc in the
  provider keeps the shared state alive. Reads return stale but valid data.
- **No import dependency**: statusline does not `use git_signs::*`
- **Graceful degradation**: `get()` returns `None` if git-signs is absent
- **Pull-based**: statusline queries at rebuild time, not pushed to
- **Registration at init**: producer registers in `init()` or `on_all_loaded()`

## Relation to Server Model

```
SERVER                              CLIENT
------                              ------
                                    PLATFORM ADAPTER  (ground truth)
                                      |
KERNEL  (complex shared state)      COMMON CORE  (event loop + compositor)
  |                                    |
DRIVER  (service contracts)          DRIVER (display contracts)
  |                                    |
MODULE  (policy)                     MODULE (policy, adapts to platform)
```

Key asymmetries:
- Server has no platform adapter. Only clients render.
- Client modules adapt to the platform via capabilities.
- Both share kernel types (BufferId, WindowId, ServiceRegistry).
- Both use Cargo crate extensibility — modules are crates, not plugins.
