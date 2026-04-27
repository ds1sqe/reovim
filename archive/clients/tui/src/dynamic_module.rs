//! Dynamic client module wrapper — bridges [`ClientModuleHandle`] (from the
//! driver crate) into the full [`ClientModule`] trait.
//!
//! # Role within flights #724 and #723
//!
//! `ClientModuleHandle` already dispatches 23 FFI trampolines for lifecycle,
//! events, role declaration, and chrome metadata. It does NOT implement the
//! `ClientModule` trait directly because the TUI still needs a concrete wrapper
//! for discovery/loading and because three borrowed-slice trait methods remain
//! unsafely unrepresentable for dynamic modules.
//!
//! `DynamicClientModule` is a thin wrapper that:
//!
//! - Delegates every FFI-backed identity/lifecycle/event/render method to the
//!   underlying handle.
//! - Leaves `fold_ranges`, `virtual_lines`, and `inline_decorations` as empty
//!   slices for dynamic modules because the trait returns `&[T]` borrowed from
//!   `&self`, while the dynamic path only produces owned `Vec<T>` snapshots.
//!
//! This exists so dynamic `.so` client modules can sit inside the same
//! `Vec<Box<dyn ClientModule>>` that the TUI's `ClientModuleLoader` manages
//! for builtin factories. After #723 landed, the wrapper stayed in place: it
//! still provides the `is_dynamic` assertion in `new()` and is the right place
//! to hold any future wrapper-local cache if the borrowed-slice trait methods
//! are redesigned.
//!
//! # Design fork
//!
//! See the #724 countdown addendum: this wrapper starts as scaffolding and is
//! revisited in flight 3 (#723).

#![allow(unsafe_code)] // discovery + loading path inherits FFI unsafety from ClientModuleHandle

use std::collections::HashSet;
#[cfg(test)]
use std::path::Path;

use reovim_client_driver::{
    AnnotationContext, BufferId, BufferUpdateEvent, ChromePosition, ChromeSurface, ClientModule,
    ClientModuleError, ColumnWidth, GutterCell, InlineDecoration, ModuleContext, OptionValue,
    ProbeResult, Rect, RenderBehavior, TransformedLine, Version, VirtualLine,
    handle::ClientModuleHandle,
    traits::{PlatformCapabilities, ThemeProvider},
};

/// Wraps a dynamically-loaded [`ClientModuleHandle`] as a full [`ClientModule`].
///
/// FFI-backed methods delegate to the handle. The only deliberate limitation is
/// the three borrowed-slice render hooks (`fold_ranges`, `virtual_lines`,
/// `inline_decorations`): dynamic modules currently return empty slices there
/// until the trait is redesigned to return owned data.
///
/// # Safety and threading
///
/// [`ClientModuleHandle`] already carries `unsafe impl Send + Sync` justified
/// by the TUI's single-dispatch guarantee. This wrapper inherits that
/// guarantee verbatim — it owns the handle and performs no additional
/// synchronization. Any refactor that multi-threads module dispatch must
/// revisit both `ClientModuleHandle`'s impls AND this wrapper.
pub struct DynamicClientModule {
    handle: ClientModuleHandle,
}

impl DynamicClientModule {
    /// Wrap a dynamic [`ClientModuleHandle`].
    ///
    /// # Panics
    ///
    /// Panics if `handle` wraps a static module — the wrapper exists solely
    /// for dynamic modules. Static modules go through the factory path in
    /// `ClientModuleLoader::new`.
    #[must_use]
    pub fn new(handle: ClientModuleHandle) -> Self {
        assert!(handle.is_dynamic(), "DynamicClientModule requires a dynamic ClientModuleHandle");
        Self { handle }
    }

    /// Test-only constructor that accepts a static handle for use in cross-type
    /// dependency resolution tests (T4). Skips the `is_dynamic` assertion.
    #[cfg(test)]
    #[allow(dead_code)] // consumed by T4 cross-type dependency test
    pub(crate) const fn new_for_test(handle: ClientModuleHandle) -> Self {
        Self { handle }
    }

    /// Borrow the underlying handle (used by discovery filter tests).
    #[cfg(test)]
    #[allow(dead_code)] // consumed by T3 discovery filter test
    pub(crate) const fn handle(&self) -> &ClientModuleHandle {
        &self.handle
    }
}

#[allow(unused_variables)]
impl ClientModule for DynamicClientModule {
    // ========================================================================
    // Identity — 6 methods, all delegate to handle (leaked 'static strings)
    // ========================================================================

    fn id(&self) -> &'static str {
        self.handle.kind()
    }

    fn kind(&self) -> &'static str {
        self.handle.kind()
    }

    fn name(&self) -> &'static str {
        self.handle.name()
    }

    fn version(&self) -> Version {
        self.handle.version()
    }

    fn dependencies(&self) -> &[&str] {
        self.handle.dependencies()
    }

    fn optional_dependencies(&self) -> &[&str] {
        self.handle.optional_dependencies()
    }

    // `server_kinds` — trait default is `vec![self.kind()]`, which is the
    // correct behavior for dynamic modules that do not override it. No FFI
    // trampoline exists for `server_kinds`, and #723 has no plan to add one.

    // ========================================================================
    // Lifecycle — 3 methods, all delegate via FFI
    // ========================================================================

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        self.handle.init(ctx)
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        self.handle.exit()
    }

    fn on_all_loaded(&mut self, ctx: &ModuleContext) {
        self.handle.on_all_loaded(ctx);
    }

    // ========================================================================
    // Role declaration — 3 methods, all delegate via FFI
    // ========================================================================

    fn has_chrome(&self) -> bool {
        self.handle.has_chrome()
    }

    fn has_buffer_contrib(&self) -> bool {
        self.handle.has_buffer_contrib()
    }

    fn has_annotations(&self) -> bool {
        self.handle.has_annotations()
    }

    // ========================================================================
    // Events — 9 methods with FFI
    // ========================================================================

    fn on_notification(&mut self, data: &str) {
        self.handle.on_notification(data);
    }

    fn on_option_changed(&mut self, name: &str, value: &OptionValue) {
        self.handle.on_option_changed(name, value);
    }

    fn on_buffer_update(&mut self, event: &BufferUpdateEvent) {
        self.handle.on_buffer_update(event);
    }

    fn on_cursor_update(&mut self, buffer_id: BufferId, line: usize, col: usize) {
        self.handle.on_cursor_update(buffer_id, line, col);
    }

    fn on_buffer_focus(&mut self, buffer_id: BufferId) {
        self.handle.on_buffer_focus(buffer_id);
    }

    fn on_mode_change(&mut self, mode: &str) {
        self.handle.on_mode_change(mode);
    }

    fn on_capabilities_changed(&mut self, caps: &dyn PlatformCapabilities) {
        self.handle.on_capabilities_changed(caps);
    }

    fn on_theme_changed(&mut self, theme: &dyn ThemeProvider) {
        self.handle.on_theme_changed(theme);
    }

    fn tick(&mut self) -> bool {
        self.handle.tick()
    }

    // ========================================================================
    // Chrome metadata — 4 methods with FFI
    // ========================================================================

    fn chrome_position(&self) -> ChromePosition {
        self.handle.chrome_position()
    }

    fn chrome_requested_size(&self, caps: &dyn PlatformCapabilities) -> u16 {
        self.handle.chrome_requested_size(caps)
    }

    fn chrome_priority(&self) -> u16 {
        self.handle.chrome_priority()
    }

    fn chrome_z_order(&self) -> u16 {
        self.handle.chrome_z_order()
    }

    // ========================================================================
    // Chrome render — 1 FFI-backed method
    // ========================================================================

    fn chrome_render(
        &self,
        surface: &mut dyn ChromeSurface,
        bounds: Rect,
        caps: &dyn PlatformCapabilities,
    ) {
        self.handle.chrome_render(surface, bounds, caps);
    }

    // ========================================================================
    // Buffer contribution — 4 FFI-backed methods + 3 safe limitations
    // ========================================================================

    fn buffer_contrib_priority(&self) -> u16 {
        self.handle.buffer_contrib_priority()
    }

    fn classify_token(&self, category: &str) -> Option<RenderBehavior> {
        self.handle.classify_token(category)
    }

    fn transform_line(&self, buf: BufferId, line: usize, text: &str) -> Option<TransformedLine> {
        self.handle.transform_line(buf, line, text)
    }

    fn map_cursor_column(&self, buf: BufferId, line: usize, col: usize) -> Option<u16> {
        self.handle.map_cursor_column(buf, line, col)
    }

    // `fold_ranges`, `virtual_lines`, `inline_decorations` return `&[T]`
    // borrowed from `&self` in the trait signature. This assumes the
    // owning storage lives in the module instance, which dynamic modules
    // cannot honor without Sync-compatible interior mutability (the
    // `ClientModule` trait requires `Send + Sync`, ruling out `RefCell`).
    //
    // For flight #723 we return empty slices for dynamic modules. Dynamic
    // modules still contribute `chrome_render`, `transform_line`,
    // `annotate`, `classify_token`, `cursor_position`, and the 4 chrome
    // metadata methods — that covers the render-path core. Fold/virtual/
    // inline-decoration contributions are deferred until the trait is
    // revised to return owned `Vec<T>` (follow-on issue; the #723
    // countdown addendum acknowledges this limitation).

    fn fold_ranges(&self) -> &[(usize, usize)] {
        &[]
    }

    fn virtual_lines(&self) -> &[VirtualLine] {
        &[]
    }

    fn inline_decorations(&self, _line: usize) -> &[InlineDecoration] {
        &[]
    }

    fn cursor_position(&self, w: u16, h: u16) -> Option<(u16, u16)> {
        self.handle.cursor_position(w, h)
    }

    // ========================================================================
    // Annotations — 2 FFI-backed methods + priority metadata
    // ========================================================================

    fn annotation_column_width(
        &self,
        ctx: &AnnotationContext,
        caps: &dyn PlatformCapabilities,
    ) -> ColumnWidth {
        self.handle.annotation_column_width(ctx, caps)
    }

    fn annotate(&self, line: usize, ctx: &AnnotationContext) -> Option<GutterCell> {
        self.handle.annotate(line, ctx)
    }

    fn annotation_priority(&self) -> u16 {
        self.handle.annotation_priority()
    }
}

// =============================================================================
// Discovery + loading (Phase 2)
// =============================================================================

/// Discover and wrap dynamic client modules for injection into the loader.
///
/// Scans default search paths, loads each discovered `.so`, wraps successful
/// handles in [`DynamicClientModule`], and returns the filtered vector ready
/// for [`reovim_client_driver::ClientModuleLoader::new_with_dynamic`].
///
/// Filters applied, in order:
///
/// 1. **builtin conflict** — dynamic module whose kind matches a builtin is
///    skipped (builtins win).
/// 2. **disabled kind** — modules listed in `disabled_kinds` (from
///    `modules.toml`) are skipped.
/// 3. **duplicate dynamic** — if two discovered `.so` files report the same
///    kind, the first one in search-path order wins.
///
/// Skip reasons are logged via `tracing` so the T3 discovery filter test can
/// verify they were applied correctly.
///
/// # Safety
///
/// This function loads `.so` files from user-trusted paths (see
/// `REOVIM_CLIENT_MODULE_PATH`, `$XDG_DATA_HOME/reovim/client-modules`, and
/// system paths). The trust model matches server-side
/// `discover_and_load_externals()` in `apps/bin/src/bootstrap.rs`. The caller
/// is responsible for respecting that trust model.
///
/// The single-threaded TUI dispatch invariant documented on
/// [`DynamicClientModule`] is required — do not call this from a background
/// task or on multiple threads concurrently.
// Filesystem + .so integration — exercised by T1/T3/T5 integration tests.
#[cfg_attr(coverage_nightly, coverage(off))]
pub unsafe fn discover_dynamic_client_modules<
    S: std::hash::BuildHasher,
    B: std::hash::BuildHasher,
>(
    disabled_kinds: &HashSet<String, S>,
    builtin_kinds: &HashSet<&str, B>,
) -> Vec<Box<dyn ClientModule>> {
    let search_paths = reovim_client_driver::discovery::default_client_search_paths();
    let mut discovered = reovim_client_driver::discovery::discover_client_modules(&search_paths);
    // Deterministic ordering — T3 asserts filename-sort order.
    discovered.sort();

    if discovered.is_empty() {
        return Vec::new();
    }

    tracing::info!(count = discovered.len(), "discovered dynamic client module files",);

    // SAFETY: caller accepted the FFI trust model documented above; we
    // forward that invariant into the filter loop, which itself only calls
    // `ClientModuleHandle::load_from_path` (also unsafe for the same reason).
    unsafe { load_filtered_dynamic_modules(&discovered, disabled_kinds, builtin_kinds) }
}

/// Shared filter loop used by [`discover_dynamic_client_modules`] and the T3
/// test (which passes a temp-dir path list directly).
///
/// # Safety
///
/// Same trust model as [`discover_dynamic_client_modules`].
// Filesystem + .so integration — tested by T3 discovery filter combination test.
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) unsafe fn load_filtered_dynamic_modules<
    S: std::hash::BuildHasher,
    B: std::hash::BuildHasher,
>(
    discovered: &[std::path::PathBuf],
    disabled_kinds: &HashSet<String, S>,
    builtin_kinds: &HashSet<&str, B>,
) -> Vec<Box<dyn ClientModule>> {
    let mut modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let mut seen_dynamic_kinds: HashSet<String> = HashSet::new();

    for path in discovered {
        let handle = match unsafe { ClientModuleHandle::load_from_path(path) } {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "failed to load dynamic client module, skipping",
                );
                continue;
            }
        };

        let kind = handle.kind();

        if builtin_kinds.contains(kind) {
            tracing::debug!(
                %kind,
                path = %path.display(),
                "dynamic client module duplicates builtin, skipping",
            );
            continue;
        }

        if disabled_kinds.contains(kind) {
            tracing::info!(
                %kind,
                path = %path.display(),
                "dynamic client module disabled by config, skipping",
            );
            continue;
        }

        if seen_dynamic_kinds.contains(kind) {
            tracing::debug!(
                %kind,
                path = %path.display(),
                "duplicate dynamic client module kind, skipping (first discovered wins)",
            );
            continue;
        }

        seen_dynamic_kinds.insert(kind.to_string());

        tracing::info!(
            %kind,
            path = %path.display(),
            "loaded dynamic client module",
        );
        modules.push(Box::new(DynamicClientModule::new(handle)));
    }

    modules
}

/// Convenience helper for single-path loading (used by T1 + T5 tests).
///
/// # Safety
///
/// Same as [`ClientModuleHandle::load_from_path`].
#[cfg(test)]
#[allow(dead_code)] // consumed by T1/T5 integration tests
pub(crate) unsafe fn load_single_for_test(
    path: &Path,
) -> Result<ClientModuleHandle, reovim_client_driver::handle::LoadError> {
    unsafe { ClientModuleHandle::load_from_path(path) }
}

#[cfg(test)]
#[path = "dynamic_module_tests.rs"]
mod tests;
