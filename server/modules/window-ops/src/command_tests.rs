use {
    super::*,
    reovim_driver_command::Command,
    reovim_driver_text_session::testing::TestSessionRuntime,
    reovim_subsys_layout::{
        Layer, LayerConfig, LayerId, OverlayConstraints, Rect, WindowId, WindowPlacement, Zone,
    },
};

// =========================================================================
// Mock compositor for success-path testing
// =========================================================================

struct MockLayerCompositor {
    id: LayerId,
    windows: Vec<WindowId>,
    focused: Option<WindowId>,
    next_id: usize,
}

impl MockLayerCompositor {
    fn new() -> Self {
        let first = WindowId::from_raw(1);
        let second = WindowId::from_raw(2);
        Self {
            id: LayerId::new(0),
            windows: vec![first, second],
            focused: Some(first),
            next_id: 3,
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_subsys_layout::WindowLayerCompositor for MockLayerCompositor {
    fn id(&self) -> LayerId {
        self.id
    }

    fn arrange(&self, _bounds: Rect) -> Vec<WindowPlacement> {
        Vec::new()
    }

    fn add_tiled(&mut self) -> WindowId {
        let id = WindowId::from_raw(self.next_id);
        self.next_id += 1;
        self.windows.push(id);
        if self.focused.is_none() {
            self.focused = Some(id);
        }
        id
    }

    fn split_tiled(&mut self, _from: WindowId, _direction: SplitDirection) -> Option<WindowId> {
        let id = WindowId::from_raw(self.next_id);
        self.next_id += 1;
        self.windows.push(id);
        self.focused = Some(id);
        Some(id)
    }

    fn navigate_tiled(&self, from: WindowId, _direction: NavigateDirection) -> Option<WindowId> {
        self.windows.iter().find(|&&w| w != from).copied()
    }

    fn resize_tiled(&mut self, _window: WindowId, _direction: NavigateDirection, _delta: i16) {}

    fn close_tiled(&mut self, window: WindowId) -> Option<WindowId> {
        self.windows.retain(|&w| w != window);
        let next = self.windows.first().copied();
        if self.focused == Some(window) {
            self.focused = next;
        }
        next
    }

    fn equalize_tiled(&mut self) {}

    fn cycle_tiled(&self, from: WindowId, _forward: bool) -> Option<WindowId> {
        self.windows.iter().find(|&&w| w != from).copied()
    }

    fn create_float(&mut self, _bounds: Rect) -> WindowId {
        let id = WindowId::from_raw(self.next_id);
        self.next_id += 1;
        id
    }

    fn move_float(&mut self, _window: WindowId, _x: u16, _y: u16) {}
    fn resize_float(&mut self, _window: WindowId, _width: u16, _height: u16) {}
    fn raise_float(&mut self, _window: WindowId) {}
    fn lower_float(&mut self, _window: WindowId) {}
    fn close_float(&mut self, _window: WindowId) {}
    fn toggle_float(&mut self, _window: WindowId) {}

    fn show_overlay(&mut self, _constraints: OverlayConstraints) -> WindowId {
        let id = WindowId::from_raw(self.next_id);
        self.next_id += 1;
        id
    }

    fn hide_overlay(&mut self, _window: WindowId) {}
    fn resize_overlay(&mut self, _window: WindowId, _width: u16, _height: u16) {}
    fn hide_all_overlays(&mut self) {}

    fn set_focus(&mut self, window: WindowId) {
        self.focused = Some(window);
    }

    fn focused(&self) -> Option<WindowId> {
        self.focused
    }

    fn windows_in_zone(&self, zone: Zone) -> Vec<WindowId> {
        if zone == Zone::Tiled {
            self.windows.clone()
        } else {
            Vec::new()
        }
    }

    fn zone_of(&self, _window: WindowId) -> Option<Zone> {
        Some(Zone::Tiled)
    }
}

struct MockRootCompositor {
    layer: MockLayerCompositor,
    focused: Option<WindowId>,
}

impl MockRootCompositor {
    fn new() -> Self {
        let layer = MockLayerCompositor::new();
        let focused = layer.focused;
        Self { layer, focused }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_subsys_layout::RootCompositor for MockRootCompositor {
    fn composite(&self, screen: Rect) -> reovim_subsys_layout::CompositeResult {
        reovim_subsys_layout::CompositeResult::empty(screen)
    }

    fn create_layer(&mut self, _config: LayerConfig) -> LayerId {
        LayerId::new(0)
    }

    fn remove_layer(&mut self, _layer: LayerId) {}

    fn layer_by_label(&self, _label: &str) -> Option<LayerId> {
        None
    }

    fn layers(&self) -> Vec<&Layer> {
        Vec::new()
    }

    fn set_layer_visible(&mut self, _layer: LayerId, _visible: bool) {}
    fn set_layer_opacity(&mut self, _layer: LayerId, _opacity: f32) {}
    fn reorder_layer(&mut self, _layer: LayerId, _new_z: u16) {}
    fn set_active_layer(&mut self, _layer: LayerId) {}

    fn active_layer(&self) -> Option<LayerId> {
        Some(LayerId::new(0))
    }

    fn set_focus(&mut self, window: WindowId) {
        self.focused = Some(window);
        reovim_subsys_layout::WindowLayerCompositor::set_focus(&mut self.layer, window);
    }

    fn focused(&self) -> Option<WindowId> {
        self.focused
    }

    fn layer_compositor(
        &self,
        _layer: LayerId,
    ) -> Option<&dyn reovim_subsys_layout::WindowLayerCompositor> {
        Some(&self.layer)
    }

    fn layer_compositor_mut(
        &mut self,
        _layer: LayerId,
    ) -> Option<&mut dyn reovim_subsys_layout::WindowLayerCompositor> {
        Some(&mut self.layer)
    }

    fn window_count(&self) -> usize {
        self.layer.windows.len()
    }

    fn set_screen(&mut self, _screen: Rect) {}

    fn layer_of(&self, _window: WindowId) -> Option<LayerId> {
        Some(LayerId::new(0))
    }

    fn boxed_clone(&self) -> Box<dyn reovim_subsys_layout::RootCompositor> {
        Box::new(Self {
            layer: MockLayerCompositor {
                id: self.layer.id,
                windows: self.layer.windows.clone(),
                focused: self.layer.focused,
                next_id: self.layer.next_id,
            },
            focused: self.focused,
        })
    }

    fn generation(&self) -> u64 {
        0
    }

    fn topology(&self) -> reovim_subsys_layout::LayoutTopology {
        reovim_subsys_layout::LayoutTopology::Single(WindowId::from_raw(0))
    }
}

// =========================================================================
// Helper: execute with compositor and assert success
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
fn assert_execute_succeeds(cmd: &dyn CommandHandler) {
    let mut test = TestSessionRuntime::new();
    test.set_compositor(Box::new(MockRootCompositor::new()));
    let ctx = CommandContext::new();
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &ctx));
    assert!(
        result.is_success(),
        "Expected success for command '{}', got: {result:?}",
        cmd.id().name()
    );
}

// =========================================================================
// Helper: execute a command and assert it returns an error
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
fn assert_execute_errors(cmd: &dyn CommandHandler) {
    let mut test = TestSessionRuntime::new();
    let ctx = CommandContext::new();
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &ctx));
    assert!(result.is_error(), "Expected error for command '{}'", cmd.id().name());
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn assert_error_mentions_layer(cmd: &dyn CommandHandler) {
    let mut test = TestSessionRuntime::new();
    let ctx = CommandContext::new();
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &ctx));
    if let CommandResult::Error(msg) = result {
        assert!(
            msg.contains("layer"),
            "Command '{}' error should mention 'layer', got: {msg}",
            cmd.id().name(),
        );
    } else {
        panic!("Expected error for command '{}'", cmd.id().name());
    }
}

// =========================================================================
// all_commands() collection tests
// =========================================================================

#[test]
fn test_all_commands_returns_28_handlers() {
    let commands = all_commands();
    assert_eq!(commands.len(), 28);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_all_commands_have_unique_ids() {
    let commands = all_commands();
    let ids: Vec<_> = commands.iter().map(|c| c.id()).collect();
    for (i, a) in ids.iter().enumerate() {
        for (j, b) in ids.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "Commands at positions {i} and {j} share an ID");
            }
        }
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_all_commands_have_non_empty_descriptions() {
    let commands = all_commands();
    for cmd in &commands {
        let desc = cmd.description();
        assert!(!desc.is_empty(), "Command '{}' has empty description", cmd.id().name());
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_commands_with_args_are_expected() {
    // Only these commands should have args
    let expected_with_args = ["layer-opacity-set", "tab-goto"];
    let commands = all_commands();
    for cmd in &commands {
        let has_args = !cmd.args().is_empty();
        let id = cmd.id();
        let name = id.name();
        if expected_with_args.contains(&name) {
            assert!(has_args, "Command '{name}' should have args");
        } else {
            assert!(!has_args, "Command '{name}' unexpectedly has args");
        }
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_commands_with_aliases_are_expected() {
    // Only these commands should have aliases (ex-command names)
    let expected_with_aliases = [
        "layer-opacity-set",
        "layer-opacity-increase",
        "layer-opacity-decrease",
        "tab-new",
        "tab-close",
        "tab-next",
        "tab-prev",
    ];
    let commands = all_commands();
    for cmd in &commands {
        let has_aliases = !cmd.names().is_empty();
        let id = cmd.id();
        let name = id.name();
        if expected_with_aliases.contains(&name) {
            assert!(has_aliases, "Command '{name}' should have aliases");
        } else {
            assert!(!has_aliases, "Command '{name}' unexpectedly has aliases");
        }
    }
}

// =========================================================================
// Per-command metadata and execute tests
// =========================================================================

#[test]
fn test_focus_left_metadata() {
    let cmd = FocusLeft;
    assert_eq!(cmd.id(), ids::FOCUS_LEFT);
    assert_eq!(cmd.description(), "Move focus to left window");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_focus_left_execute_no_compositor() {
    assert_execute_errors(&FocusLeft);
}

#[test]
fn test_focus_down_metadata() {
    let cmd = FocusDown;
    assert_eq!(cmd.id(), ids::FOCUS_DOWN);
    assert_eq!(cmd.description(), "Move focus to window below");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_focus_down_execute_no_compositor() {
    assert_execute_errors(&FocusDown);
}

#[test]
fn test_focus_up_metadata() {
    let cmd = FocusUp;
    assert_eq!(cmd.id(), ids::FOCUS_UP);
    assert_eq!(cmd.description(), "Move focus to window above");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_focus_up_execute_no_compositor() {
    assert_execute_errors(&FocusUp);
}

#[test]
fn test_focus_right_metadata() {
    let cmd = FocusRight;
    assert_eq!(cmd.id(), ids::FOCUS_RIGHT);
    assert_eq!(cmd.description(), "Move focus to right window");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_focus_right_execute_no_compositor() {
    assert_execute_errors(&FocusRight);
}

#[test]
fn test_focus_next_metadata() {
    let cmd = FocusNext;
    assert_eq!(cmd.id(), ids::FOCUS_NEXT);
    assert_eq!(cmd.description(), "Cycle focus to next window");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_focus_next_execute_no_compositor() {
    assert_execute_errors(&FocusNext);
}

#[test]
fn test_focus_prev_metadata() {
    let cmd = FocusPrev;
    assert_eq!(cmd.id(), ids::FOCUS_PREV);
    assert_eq!(cmd.description(), "Cycle focus to previous window");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_focus_prev_execute_no_compositor() {
    assert_execute_errors(&FocusPrev);
}

#[test]
fn test_split_horizontal_metadata() {
    let cmd = SplitHorizontal;
    assert_eq!(cmd.id(), ids::SPLIT_HORIZONTAL);
    assert_eq!(cmd.description(), "Split window horizontally");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_split_horizontal_execute_no_compositor() {
    assert_execute_errors(&SplitHorizontal);
}

#[test]
fn test_split_vertical_metadata() {
    let cmd = SplitVertical;
    assert_eq!(cmd.id(), ids::SPLIT_VERTICAL);
    assert_eq!(cmd.description(), "Split window vertically");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_split_vertical_execute_no_compositor() {
    assert_execute_errors(&SplitVertical);
}

#[test]
fn test_split_new_metadata() {
    let cmd = SplitNew;
    assert_eq!(cmd.id(), ids::SPLIT_NEW);
    assert_eq!(cmd.description(), "Create new window with empty buffer");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_split_new_execute_no_compositor() {
    assert_execute_errors(&SplitNew);
}

#[test]
fn test_close_window_metadata() {
    let cmd = CloseWindow;
    assert_eq!(cmd.id(), ids::CLOSE_WINDOW);
    assert_eq!(cmd.description(), "Close current window");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_close_window_execute_no_compositor() {
    assert_execute_errors(&CloseWindow);
}

#[test]
fn test_close_others_metadata() {
    let cmd = CloseOthers;
    assert_eq!(cmd.id(), ids::CLOSE_OTHERS);
    assert_eq!(cmd.description(), "Close all other windows");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_close_others_execute_no_compositor() {
    assert_execute_errors(&CloseOthers);
}

#[test]
fn test_resize_height_increase_metadata() {
    let cmd = ResizeHeightIncrease;
    assert_eq!(cmd.id(), ids::RESIZE_HEIGHT_INCREASE);
    assert_eq!(cmd.description(), "Increase window height");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_resize_height_increase_execute_no_compositor() {
    assert_execute_errors(&ResizeHeightIncrease);
}

#[test]
fn test_resize_height_decrease_metadata() {
    let cmd = ResizeHeightDecrease;
    assert_eq!(cmd.id(), ids::RESIZE_HEIGHT_DECREASE);
    assert_eq!(cmd.description(), "Decrease window height");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_resize_height_decrease_execute_no_compositor() {
    assert_execute_errors(&ResizeHeightDecrease);
}

#[test]
fn test_resize_width_increase_metadata() {
    let cmd = ResizeWidthIncrease;
    assert_eq!(cmd.id(), ids::RESIZE_WIDTH_INCREASE);
    assert_eq!(cmd.description(), "Increase window width");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_resize_width_increase_execute_no_compositor() {
    assert_execute_errors(&ResizeWidthIncrease);
}

#[test]
fn test_resize_width_decrease_metadata() {
    let cmd = ResizeWidthDecrease;
    assert_eq!(cmd.id(), ids::RESIZE_WIDTH_DECREASE);
    assert_eq!(cmd.description(), "Decrease window width");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_resize_width_decrease_execute_no_compositor() {
    assert_execute_errors(&ResizeWidthDecrease);
}

#[test]
fn test_resize_equal_metadata() {
    let cmd = ResizeEqual;
    assert_eq!(cmd.id(), ids::RESIZE_EQUAL);
    assert_eq!(cmd.description(), "Equalize all window sizes");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_resize_equal_execute_no_compositor() {
    assert_execute_errors(&ResizeEqual);
}

#[test]
fn test_toggle_float_metadata() {
    let cmd = ToggleFloat;
    assert_eq!(cmd.id(), ids::TOGGLE_FLOAT);
    assert_eq!(cmd.description(), "Toggle window between tiled and floating");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_toggle_float_execute_no_compositor() {
    assert_execute_errors(&ToggleFloat);
}

#[test]
fn test_raise_float_metadata() {
    let cmd = RaiseFloat;
    assert_eq!(cmd.id(), ids::RAISE_FLOAT);
    assert_eq!(cmd.description(), "Raise floating window to front");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_raise_float_execute_no_compositor() {
    assert_execute_errors(&RaiseFloat);
}

#[test]
fn test_lower_float_metadata() {
    let cmd = LowerFloat;
    assert_eq!(cmd.id(), ids::LOWER_FLOAT);
    assert_eq!(cmd.description(), "Lower floating window to back");
    assert!(cmd.args().is_empty());
}

#[test]
fn test_lower_float_execute_no_compositor() {
    assert_execute_errors(&LowerFloat);
}

// =========================================================================
// Error message content validation
// =========================================================================

#[test]
fn test_focus_commands_error_contains_layer_message() {
    let cmds: Vec<Box<dyn CommandHandler>> = vec![
        Box::new(FocusLeft),
        Box::new(FocusDown),
        Box::new(FocusUp),
        Box::new(FocusRight),
        Box::new(FocusNext),
        Box::new(FocusPrev),
    ];
    for cmd in &cmds {
        assert_error_mentions_layer(cmd.as_ref());
    }
}

#[test]
fn test_split_commands_error_contains_layer_message() {
    let cmds: Vec<Box<dyn CommandHandler>> = vec![
        Box::new(SplitHorizontal),
        Box::new(SplitVertical),
        Box::new(SplitNew),
    ];
    for cmd in &cmds {
        assert_error_mentions_layer(cmd.as_ref());
    }
}

#[test]
fn test_close_commands_error_contains_layer_message() {
    let cmds: Vec<Box<dyn CommandHandler>> = vec![Box::new(CloseWindow), Box::new(CloseOthers)];
    for cmd in &cmds {
        assert_error_mentions_layer(cmd.as_ref());
    }
}

#[test]
fn test_resize_commands_error_contains_layer_message() {
    let cmds: Vec<Box<dyn CommandHandler>> = vec![
        Box::new(ResizeHeightIncrease),
        Box::new(ResizeHeightDecrease),
        Box::new(ResizeWidthIncrease),
        Box::new(ResizeWidthDecrease),
        Box::new(ResizeEqual),
    ];
    for cmd in &cmds {
        assert_error_mentions_layer(cmd.as_ref());
    }
}

#[test]
fn test_float_commands_error_contains_layer_message() {
    let cmds: Vec<Box<dyn CommandHandler>> = vec![
        Box::new(ToggleFloat),
        Box::new(RaiseFloat),
        Box::new(LowerFloat),
    ];
    for cmd in &cmds {
        assert_error_mentions_layer(cmd.as_ref());
    }
}

// =========================================================================
// Trait object safety
// =========================================================================

#[test]
fn test_all_commands_usable_as_trait_objects() {
    let commands = all_commands();
    for cmd in &commands {
        // Verify CommandHandler can be used as trait object
        let _: &dyn CommandHandler = cmd.as_ref();
        // Verify it also implements Command
        let _: &dyn Command = cmd.as_ref();
    }
}

// =========================================================================
// Debug implementations
// =========================================================================

#[test]
fn test_command_structs_implement_debug() {
    // Verify all command structs have Debug formatting
    assert!(!format!("{FocusLeft:?}").is_empty());
    assert!(!format!("{FocusDown:?}").is_empty());
    assert!(!format!("{FocusUp:?}").is_empty());
    assert!(!format!("{FocusRight:?}").is_empty());
    assert!(!format!("{FocusNext:?}").is_empty());
    assert!(!format!("{FocusPrev:?}").is_empty());
    assert!(!format!("{SplitHorizontal:?}").is_empty());
    assert!(!format!("{SplitVertical:?}").is_empty());
    assert!(!format!("{SplitNew:?}").is_empty());
    assert!(!format!("{CloseWindow:?}").is_empty());
    assert!(!format!("{CloseOthers:?}").is_empty());
    assert!(!format!("{ResizeHeightIncrease:?}").is_empty());
    assert!(!format!("{ResizeHeightDecrease:?}").is_empty());
    assert!(!format!("{ResizeWidthIncrease:?}").is_empty());
    assert!(!format!("{ResizeWidthDecrease:?}").is_empty());
    assert!(!format!("{ResizeEqual:?}").is_empty());
    assert!(!format!("{ToggleFloat:?}").is_empty());
    assert!(!format!("{RaiseFloat:?}").is_empty());
    assert!(!format!("{LowerFloat:?}").is_empty());
    assert!(!format!("{MoveToNewTab:?}").is_empty());
}

// =========================================================================
// Copy trait verification
// =========================================================================

#[test]
fn test_command_structs_implement_copy() {
    // Copy trait: after copy, both original and copy are valid
    let a = FocusLeft;
    let b = a; // copy
    assert_eq!(a.id(), b.id());

    let a = FocusDown;
    let b = a;
    assert_eq!(a.id(), b.id());

    let a = SplitHorizontal;
    let b = a;
    assert_eq!(a.id(), b.id());

    let a = CloseWindow;
    let b = a;
    assert_eq!(a.id(), b.id());

    let a = ResizeHeightIncrease;
    let b = a;
    assert_eq!(a.id(), b.id());

    let a = ToggleFloat;
    let b = a;
    assert_eq!(a.id(), b.id());

    let a = MoveToNewTab;
    let b = a;
    assert_eq!(a.id(), b.id());
}

// =========================================================================
// RESIZE_DELTA constant validation
// =========================================================================

#[test]
fn test_resize_delta_is_positive() {
    assert_eq!(RESIZE_DELTA, 1);
}

// =========================================================================
// Default trait implementations
// =========================================================================

#[test]
fn test_all_commands_implement_default() {
    fn assert_default<T: Default>() {}
    assert_default::<FocusLeft>();
    assert_default::<FocusDown>();
    assert_default::<FocusUp>();
    assert_default::<FocusRight>();
    assert_default::<FocusNext>();
    assert_default::<FocusPrev>();
    assert_default::<SplitHorizontal>();
    assert_default::<SplitVertical>();
    assert_default::<SplitNew>();
    assert_default::<CloseWindow>();
    assert_default::<CloseOthers>();
    assert_default::<ResizeHeightIncrease>();
    assert_default::<ResizeHeightDecrease>();
    assert_default::<ResizeWidthIncrease>();
    assert_default::<ResizeWidthDecrease>();
    assert_default::<ResizeEqual>();
    assert_default::<ToggleFloat>();
    assert_default::<RaiseFloat>();
    assert_default::<LowerFloat>();
    assert_default::<MoveToNewTab>();
}

// =========================================================================
// Clone trait implementations (these structs are Copy, so clone == copy)
// =========================================================================

#[test]
fn test_command_structs_implement_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<FocusLeft>();
    assert_clone::<FocusDown>();
    assert_clone::<FocusUp>();
    assert_clone::<FocusRight>();
    assert_clone::<FocusNext>();
    assert_clone::<FocusPrev>();
    assert_clone::<SplitHorizontal>();
    assert_clone::<SplitVertical>();
    assert_clone::<SplitNew>();
    assert_clone::<CloseWindow>();
    assert_clone::<CloseOthers>();
    assert_clone::<ResizeHeightIncrease>();
    assert_clone::<ResizeHeightDecrease>();
    assert_clone::<ResizeWidthIncrease>();
    assert_clone::<ResizeWidthDecrease>();
    assert_clone::<ResizeEqual>();
    assert_clone::<ToggleFloat>();
    assert_clone::<RaiseFloat>();
    assert_clone::<LowerFloat>();
    assert_clone::<MoveToNewTab>();
}

// =========================================================================
// Success path tests (with compositor)
// =========================================================================

#[test]
fn test_focus_left_execute_with_compositor() {
    assert_execute_succeeds(&FocusLeft);
}

#[test]
fn test_focus_down_execute_with_compositor() {
    assert_execute_succeeds(&FocusDown);
}

#[test]
fn test_focus_up_execute_with_compositor() {
    assert_execute_succeeds(&FocusUp);
}

#[test]
fn test_focus_right_execute_with_compositor() {
    assert_execute_succeeds(&FocusRight);
}

#[test]
fn test_focus_next_execute_with_compositor() {
    assert_execute_succeeds(&FocusNext);
}

#[test]
fn test_focus_prev_execute_with_compositor() {
    assert_execute_succeeds(&FocusPrev);
}

#[test]
fn test_split_horizontal_execute_with_compositor() {
    assert_execute_succeeds(&SplitHorizontal);
}

#[test]
fn test_split_vertical_execute_with_compositor() {
    assert_execute_succeeds(&SplitVertical);
}

#[test]
fn test_split_new_execute_with_compositor() {
    assert_execute_succeeds(&SplitNew);
}

#[test]
fn test_close_window_execute_with_compositor() {
    assert_execute_succeeds(&CloseWindow);
}

#[test]
fn test_close_others_execute_with_compositor() {
    assert_execute_succeeds(&CloseOthers);
}

#[test]
fn test_resize_height_increase_execute_with_compositor() {
    assert_execute_succeeds(&ResizeHeightIncrease);
}

#[test]
fn test_resize_height_decrease_execute_with_compositor() {
    assert_execute_succeeds(&ResizeHeightDecrease);
}

#[test]
fn test_resize_width_increase_execute_with_compositor() {
    assert_execute_succeeds(&ResizeWidthIncrease);
}

#[test]
fn test_resize_width_decrease_execute_with_compositor() {
    assert_execute_succeeds(&ResizeWidthDecrease);
}

#[test]
fn test_resize_equal_execute_with_compositor() {
    assert_execute_succeeds(&ResizeEqual);
}

#[test]
fn test_toggle_float_execute_with_compositor() {
    assert_execute_succeeds(&ToggleFloat);
}

#[test]
fn test_raise_float_execute_with_compositor() {
    assert_execute_succeeds(&RaiseFloat);
}

#[test]
fn test_lower_float_execute_with_compositor() {
    assert_execute_succeeds(&LowerFloat);
}
