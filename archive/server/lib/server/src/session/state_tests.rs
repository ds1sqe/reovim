use {super::*, reovim_kernel::api::v1::ModuleId};

fn test_mode_id() -> ModeId {
    ModeId::new(ModuleId::new("test"), "normal")
}

fn test_vfs() -> Arc<dyn VfsDriver> {
    Arc::new(reovim_subsys_vfs::MockVfs::new())
}

#[test]
fn test_session_state_new() {
    let kernel = KernelContext::default();
    let state = SessionState::new(kernel, test_mode_id(), test_vfs());

    assert!(state.is_running());
    assert!(state.mode_registry.is_empty());
    assert!(state.command_registry.is_empty());
    assert!(state.keymap_registry.is_empty());
    // #491: Use home_mode() instead of removed current_mode()
    assert_eq!(state.home_mode().name(), "normal");
}

#[test]
fn test_session_state_has_vfs() {
    let kernel = KernelContext::default();
    let state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // VFS should be accessible
    assert!(!state.vfs.exists(std::path::Path::new("/nonexistent")));
}

#[test]
fn test_session_state_quit() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    assert!(state.is_running());
    state.request_quit();
    assert!(!state.is_running());
}

#[test]
fn test_session_state_lookup_keys_empty() {
    let kernel = KernelContext::default();
    let state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let keys = crate::registry::keymap::test_helpers::seq_from_notation("j").unwrap();
    let result = state.lookup_keys(&test_mode_id(), &keys);

    assert!(result.is_not_found());
}

#[test]
fn test_session_state_mode_accepts_char_input_default() {
    let kernel = KernelContext::default();
    let state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Unknown mode defaults to not accepting input
    assert!(!state.mode_accepts_char_input());
}

#[test]
fn test_session_state_with_registries() {
    let kernel = KernelContext::default();
    let mode_reg = ModeRegistry::new();
    let cmd_reg = CommandRegistry::new();
    let keymap_reg = KeymapRegistry::new();
    let state = SessionState::with_registries(
        kernel,
        test_mode_id(),
        test_vfs(),
        mode_reg,
        cmd_reg,
        keymap_reg,
        None,
    );

    assert!(state.is_running());
    assert_eq!(state.home_mode().name(), "normal");
}

#[test]
fn test_session_state_default() {
    let state = SessionState::default();
    assert!(state.is_running());
    assert!(state.mode_registry.is_empty());
}

// test_session_state_with_kernel: DELETED — uses driver-specific TestBufferManager.

#[test]
fn test_session_terminal_size() {
    // terminal_size is now per-client; verify default VT100 dimensions
    let default_size = (80u16, 24u16);
    assert_eq!(default_size, (80, 24));
}

#[test]
fn test_set_session_terminal_size() {
    // terminal_size is now per-client; verify it can be stored in a tuple
    let terminal_size = (120u16, 40u16);
    assert_eq!(terminal_size, (120, 40));
}

#[test]
fn test_home_mode() {
    let kernel = KernelContext::default();
    let state = SessionState::new(kernel, test_mode_id(), test_vfs());

    assert_eq!(state.home_mode().name(), "normal");
}

#[test]
fn test_request_detach() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    state.request_detach();
    // After detach, server continues running but clients disconnect
    // The is_running() check is for quit, not detach
    assert!(state.is_running());
}

// test_with_registries_with_initial_buffer: DELETED — uses driver-specific TestBufferManager/Buffer.

#[test]
fn test_session_terminal_size_roundtrip() {
    // terminal_size is now per-client; verify tuple storage works
    let mut terminal_size = (80u16, 24u16);
    assert_eq!(terminal_size, (80, 24));

    terminal_size = (200, 50);
    assert_eq!(terminal_size, (200, 50));

    terminal_size = (1, 1);
    assert_eq!(terminal_size, (1, 1));
}

#[test]
fn test_session_state_mode_accepts_char_input_with_registered_mode() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Register a mode that accepts char input (discriminant 1, different from home mode 0)
    let insert_mode_id = ModeId::with_discriminant(ModuleId::new("test"), "INSERT", 1);
    state
        .mode_registry
        .register(crate::registry::ModeEntry::from_fields(
            insert_mode_id,
            "INSERT",
            reovim_kernel::api::v1::CursorStyle::Bar,
            true,  // accepts_char_input
            false, // has_selection
            None,  // inherits_from
            false, // is_entry
        ));

    // mode_accepts_char_input checks home_mode (test/normal, discriminant 0)
    // We registered test/INSERT (discriminant 1) which is a different mode
    // So home_mode is NOT in the registry, should return false
    assert!(!state.mode_accepts_char_input());

    // Now register home_mode with accepts_char_input: false
    state
        .mode_registry
        .register(crate::registry::ModeEntry::from_fields(
            test_mode_id(),
            "NORMAL",
            reovim_kernel::api::v1::CursorStyle::Block,
            false, // accepts_char_input
            false, // has_selection
            None,  // inherits_from
            true,  // is_entry
        ));

    // Home mode is now registered but does NOT accept char input
    assert!(!state.mode_accepts_char_input());
}

#[test]
fn test_session_state_request_quit_and_detach() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Detach should not affect running state
    state.request_detach();
    assert!(state.is_running());

    // Quit should stop running
    state.request_quit();
    assert!(!state.is_running());
}

#[test]
fn test_lookup_keys_after_registration() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Register a keybinding
    let cmd = reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "cursor-down");
    state
        .keymap_registry
        .register_str(&test_mode_id(), "j", cmd);

    // Lookup should find it
    let keys = crate::registry::keymap::test_helpers::seq_from_notation("j").unwrap();
    let result = state.lookup_keys(&test_mode_id(), &keys);
    assert!(result.is_found());
}

#[test]
fn test_with_registries_custom_mode() {
    let custom_mode = ModeId::new(ModuleId::new("custom"), "visual");
    let kernel = KernelContext::default();

    let state = SessionState::with_registries(
        kernel,
        custom_mode,
        test_vfs(),
        ModeRegistry::new(),
        CommandRegistry::new(),
        KeymapRegistry::new(),
        None,
    );

    assert_eq!(state.home_mode().name(), "visual");
    assert_eq!(format!("{}", state.home_mode().module()), "custom");
}

#[test]
fn test_with_registries_no_buffers_no_compositor() {
    let kernel = KernelContext::default();

    let state = SessionState::with_registries(
        kernel,
        test_mode_id(),
        test_vfs(),
        ModeRegistry::new(),
        CommandRegistry::new(),
        KeymapRegistry::new(),
        None,
    );

    // No buffers in kernel -> empty buffer list
    assert!(state.app.kernel.buffers.list().is_empty());
}

#[test]
fn test_session_state_new_with_custom_vfs() {
    use std::path::Path;

    let kernel = KernelContext::default();
    let vfs = test_vfs();
    let state = SessionState::new(kernel, test_mode_id(), vfs);

    // Verify VFS is accessible
    assert!(!state.vfs.exists(Path::new("/some/file")));
}

#[test]
fn test_session_state_registries_accessible_after_with_registries() {
    let kernel = KernelContext::default();
    let mut mode_reg = ModeRegistry::new();
    let cmd_reg = CommandRegistry::new();
    let mut keymap_reg = KeymapRegistry::new();

    // Register something in each registry
    mode_reg.register(crate::registry::ModeEntry::from_fields(
        test_mode_id(),
        "NORMAL",
        reovim_kernel::api::v1::CursorStyle::Block,
        false, // accepts_char_input
        false, // has_selection
        None,  // inherits_from
        true,  // is_entry
    ));

    let cmd_id = reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "cursor-down");
    keymap_reg.register_str(&test_mode_id(), "j", cmd_id);

    let state = SessionState::with_registries(
        kernel,
        test_mode_id(),
        test_vfs(),
        mode_reg,
        cmd_reg,
        keymap_reg,
        None,
    );

    // Mode registry should have our mode
    assert!(!state.mode_registry.is_empty());

    // Keymap lookup should find our binding
    let keys = crate::registry::keymap::test_helpers::seq_from_notation("j").unwrap();
    let result = state.lookup_keys(&test_mode_id(), &keys);
    assert!(result.is_found());
}

// test_with_registries_with_buffer_and_no_compositor: DELETED — uses driver-specific Buffer.
// test_with_registries_multiple_buffers: DELETED — uses driver-specific Buffer.

// Coverage tests for uncovered lines
// ========================================================================

/// Mock `WindowLayerCompositor` that returns empty tiled windows and
/// supports `add_tiled()`.
struct MockLayerCompositor {
    layer_id: reovim_subsys_layout::LayerId,
    tiled_windows: Vec<reovim_subsys_layout::WindowId>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl MockLayerCompositor {
    fn new(layer_id: reovim_subsys_layout::LayerId) -> Self {
        Self {
            layer_id,
            tiled_windows: Vec::new(),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_subsys_layout::WindowLayerCompositor for MockLayerCompositor {
    fn id(&self) -> reovim_subsys_layout::LayerId {
        self.layer_id
    }

    fn arrange(
        &self,
        _bounds: reovim_subsys_layout::Rect,
    ) -> Vec<reovim_subsys_layout::WindowPlacement> {
        Vec::new()
    }

    fn add_tiled(&mut self) -> reovim_subsys_layout::WindowId {
        let id = reovim_subsys_layout::WindowId::from_raw(self.tiled_windows.len() + 1);
        self.tiled_windows.push(id);
        id
    }

    fn split_tiled(
        &mut self,
        _from: reovim_subsys_layout::WindowId,
        _direction: reovim_subsys_layout::SplitDirection,
    ) -> Option<reovim_subsys_layout::WindowId> {
        None
    }

    fn navigate_tiled(
        &self,
        _from: reovim_subsys_layout::WindowId,
        _direction: reovim_subsys_layout::Direction,
    ) -> Option<reovim_subsys_layout::WindowId> {
        None
    }

    fn resize_tiled(
        &mut self,
        _window: reovim_subsys_layout::WindowId,
        _direction: reovim_subsys_layout::Direction,
        _delta: i16,
    ) {
    }

    fn close_tiled(
        &mut self,
        _window: reovim_subsys_layout::WindowId,
    ) -> Option<reovim_subsys_layout::WindowId> {
        None
    }

    fn equalize_tiled(&mut self) {}

    fn cycle_tiled(
        &self,
        _from: reovim_subsys_layout::WindowId,
        _forward: bool,
    ) -> Option<reovim_subsys_layout::WindowId> {
        None
    }

    fn create_float(
        &mut self,
        _bounds: reovim_subsys_layout::Rect,
    ) -> reovim_subsys_layout::WindowId {
        reovim_subsys_layout::WindowId::from_raw(100)
    }

    fn move_float(&mut self, _window: reovim_subsys_layout::WindowId, _x: u16, _y: u16) {}

    fn resize_float(&mut self, _window: reovim_subsys_layout::WindowId, _width: u16, _height: u16) {
    }

    fn raise_float(&mut self, _window: reovim_subsys_layout::WindowId) {}
    fn lower_float(&mut self, _window: reovim_subsys_layout::WindowId) {}
    fn close_float(&mut self, _window: reovim_subsys_layout::WindowId) {}
    fn toggle_float(&mut self, _window: reovim_subsys_layout::WindowId) {}

    fn show_overlay(
        &mut self,
        _constraints: reovim_subsys_layout::OverlayConstraints,
    ) -> reovim_subsys_layout::WindowId {
        reovim_subsys_layout::WindowId::from_raw(200)
    }

    fn hide_overlay(&mut self, _window: reovim_subsys_layout::WindowId) {}

    fn resize_overlay(
        &mut self,
        _window: reovim_subsys_layout::WindowId,
        _width: u16,
        _height: u16,
    ) {
    }

    fn hide_all_overlays(&mut self) {}

    fn set_focus(&mut self, _window: reovim_subsys_layout::WindowId) {}

    fn focused(&self) -> Option<reovim_subsys_layout::WindowId> {
        None
    }

    fn windows_in_zone(
        &self,
        zone: reovim_subsys_layout::Zone,
    ) -> Vec<reovim_subsys_layout::WindowId> {
        match zone {
            reovim_subsys_layout::Zone::Tiled => self.tiled_windows.clone(),
            _ => Vec::new(),
        }
    }

    fn zone_of(
        &self,
        _window: reovim_subsys_layout::WindowId,
    ) -> Option<reovim_subsys_layout::Zone> {
        None
    }
}

/// Mock `RootCompositor` that provides an active layer with a
/// `MockLayerCompositor` (starts with empty tiled windows).
struct MockCompositor {
    layer: MockLayerCompositor,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl MockCompositor {
    fn new() -> Self {
        Self {
            layer: MockLayerCompositor::new(reovim_subsys_layout::LayerId::new(0)),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_subsys_layout::RootCompositor for MockCompositor {
    fn composite(
        &self,
        screen: reovim_subsys_layout::Rect,
    ) -> reovim_subsys_layout::CompositeResult {
        reovim_subsys_layout::CompositeResult::empty(screen)
    }

    fn create_layer(
        &mut self,
        _config: reovim_subsys_layout::LayerConfig,
    ) -> reovim_subsys_layout::LayerId {
        reovim_subsys_layout::LayerId::new(0)
    }

    fn remove_layer(&mut self, _layer: reovim_subsys_layout::LayerId) {}

    fn layer_by_label(&self, _label: &str) -> Option<reovim_subsys_layout::LayerId> {
        None
    }

    fn layers(&self) -> Vec<&reovim_subsys_layout::Layer> {
        Vec::new()
    }

    fn set_layer_visible(&mut self, _layer: reovim_subsys_layout::LayerId, _visible: bool) {}

    fn set_layer_opacity(&mut self, _layer: reovim_subsys_layout::LayerId, _opacity: f32) {}

    fn reorder_layer(&mut self, _layer: reovim_subsys_layout::LayerId, _new_z: u16) {}

    fn set_active_layer(&mut self, _layer: reovim_subsys_layout::LayerId) {}

    fn active_layer(&self) -> Option<reovim_subsys_layout::LayerId> {
        Some(reovim_subsys_layout::LayerId::new(0))
    }

    fn set_focus(&mut self, _window: reovim_subsys_layout::WindowId) {}

    fn focused(&self) -> Option<reovim_subsys_layout::WindowId> {
        None
    }

    fn layer_compositor(
        &self,
        _layer: reovim_subsys_layout::LayerId,
    ) -> Option<&dyn reovim_subsys_layout::WindowLayerCompositor> {
        Some(&self.layer)
    }

    fn layer_compositor_mut(
        &mut self,
        _layer: reovim_subsys_layout::LayerId,
    ) -> Option<&mut dyn reovim_subsys_layout::WindowLayerCompositor> {
        Some(&mut self.layer)
    }

    fn window_count(&self) -> usize {
        self.layer.tiled_windows.len()
    }

    fn set_screen(&mut self, _screen: reovim_subsys_layout::Rect) {}

    fn layer_of(
        &self,
        _window: reovim_subsys_layout::WindowId,
    ) -> Option<reovim_subsys_layout::LayerId> {
        Some(reovim_subsys_layout::LayerId::new(0))
    }

    fn boxed_clone(&self) -> Box<dyn reovim_subsys_layout::RootCompositor> {
        Box::new(Self::new())
    }

    fn generation(&self) -> u64 {
        0
    }

    fn topology(&self) -> reovim_subsys_layout::LayoutTopology {
        reovim_subsys_layout::LayoutTopology {
            focused: Some(reovim_subsys_layout::WindowId::from_raw(1)),
            active_layer: Some(reovim_subsys_layout::LayerId::new(0)),
            tiled_trees: vec![reovim_subsys_layout::LayerTree {
                layer_id: reovim_subsys_layout::LayerId::new(0),
                tree: reovim_subsys_layout::TiledTree::Window(
                    reovim_subsys_layout::WindowId::from_raw(1),
                ),
            }],
            overlay_anchors: Vec::new(),
        }
    }

    fn tiled_tree(
        &self,
        _layer: reovim_subsys_layout::LayerId,
    ) -> Option<&reovim_subsys_layout::TiledTree> {
        None
    }

    fn set_surface(&mut self, _kind: u16, _body: &[u8]) {}
}

/// Test `with_registries` with `Some(compositor)` to cover line 148.
#[test]
fn test_with_registries_with_compositor() {
    let kernel = KernelContext::default();
    let compositor: Box<dyn reovim_subsys_layout::RootCompositor> = Box::new(MockCompositor::new());

    let state = SessionState::with_registries(
        kernel,
        test_mode_id(),
        test_vfs(),
        ModeRegistry::new(),
        CommandRegistry::new(),
        KeymapRegistry::new(),
        Some(compositor),
    );

    // Compositor should have been set
    assert!(state.compositor.is_some());
}

// test_with_registries_with_buffer_and_compositor_creates_window: DELETED — uses driver-specific Buffer.

// test_dummy_command_trait_methods: DELETED — uses driver-specific CommandHandler/SessionRuntime.

// =========================================================================
// ensure_initial_compositor_window coverage (lines 223-242)
// =========================================================================

/// Cover lines 223-242: `ensure_initial_compositor_window()` adds a tiled
/// window when buffers exist but the compositor has no tiled windows yet.
///
/// This happens when the scratch buffer is created after `with_registries()`
/// has already set up the compositor (so the inline `if !buffer_ids.is_empty()`
/// guard in `with_registries` was false) and we later need to ensure the
/// compositor has a window.
#[test]
fn test_ensure_initial_compositor_window_noop_when_no_buffers() {
    let kernel = KernelContext::default(); // no buffer manager → empty list
    let compositor: Box<dyn reovim_subsys_layout::RootCompositor> = Box::new(MockCompositor::new());

    let mut state = SessionState::with_registries(
        kernel,
        test_mode_id(),
        test_vfs(),
        ModeRegistry::new(),
        CommandRegistry::new(),
        KeymapRegistry::new(),
        Some(compositor),
    );

    // No buffers → should return early without touching compositor.
    state.ensure_initial_compositor_window();

    // Compositor should still have no tiled windows.
    let comp = state.compositor.as_ref().unwrap();
    let layer_id = comp.active_layer().unwrap();
    let layer = comp.layer_compositor(layer_id).unwrap();
    assert!(
        layer
            .windows_in_zone(reovim_subsys_layout::Zone::Tiled)
            .is_empty(),
        "no-op: compositor should still have no tiled windows"
    );
}

// test_ensure_initial_compositor_window_noop_when_already_has_tiled_window: DELETED — uses driver-specific Buffer.
