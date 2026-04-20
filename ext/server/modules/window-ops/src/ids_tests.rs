use super::*;

// =========================================================================
// Module ID
// =========================================================================

#[test]
fn test_module_id() {
    assert_eq!(MODULE.as_str(), "window-ops");
}

// =========================================================================
// Focus Navigation IDs
// =========================================================================

#[test]
fn test_focus_left_id() {
    assert_eq!(FOCUS_LEFT.module(), &MODULE);
    assert_eq!(FOCUS_LEFT.name(), "focus-left");
}

#[test]
fn test_focus_down_id() {
    assert_eq!(FOCUS_DOWN.module(), &MODULE);
    assert_eq!(FOCUS_DOWN.name(), "focus-down");
}

#[test]
fn test_focus_up_id() {
    assert_eq!(FOCUS_UP.module(), &MODULE);
    assert_eq!(FOCUS_UP.name(), "focus-up");
}

#[test]
fn test_focus_right_id() {
    assert_eq!(FOCUS_RIGHT.module(), &MODULE);
    assert_eq!(FOCUS_RIGHT.name(), "focus-right");
}

// =========================================================================
// Focus Cycling IDs
// =========================================================================

#[test]
fn test_focus_next_id() {
    assert_eq!(FOCUS_NEXT.module(), &MODULE);
    assert_eq!(FOCUS_NEXT.name(), "focus-next");
}

#[test]
fn test_focus_prev_id() {
    assert_eq!(FOCUS_PREV.module(), &MODULE);
    assert_eq!(FOCUS_PREV.name(), "focus-prev");
}

// =========================================================================
// Window Splitting IDs
// =========================================================================

#[test]
fn test_split_horizontal_id() {
    assert_eq!(SPLIT_HORIZONTAL.module(), &MODULE);
    assert_eq!(SPLIT_HORIZONTAL.name(), "split-horizontal");
}

#[test]
fn test_split_vertical_id() {
    assert_eq!(SPLIT_VERTICAL.module(), &MODULE);
    assert_eq!(SPLIT_VERTICAL.name(), "split-vertical");
}

#[test]
fn test_split_new_id() {
    assert_eq!(SPLIT_NEW.module(), &MODULE);
    assert_eq!(SPLIT_NEW.name(), "split-new");
}

// =========================================================================
// Window Closing IDs
// =========================================================================

#[test]
fn test_close_window_id() {
    assert_eq!(CLOSE_WINDOW.module(), &MODULE);
    assert_eq!(CLOSE_WINDOW.name(), "close-window");
}

#[test]
fn test_close_others_id() {
    assert_eq!(CLOSE_OTHERS.module(), &MODULE);
    assert_eq!(CLOSE_OTHERS.name(), "close-others");
}

// =========================================================================
// Window Resizing IDs
// =========================================================================

#[test]
fn test_resize_height_increase_id() {
    assert_eq!(RESIZE_HEIGHT_INCREASE.module(), &MODULE);
    assert_eq!(RESIZE_HEIGHT_INCREASE.name(), "resize-height-increase");
}

#[test]
fn test_resize_height_decrease_id() {
    assert_eq!(RESIZE_HEIGHT_DECREASE.module(), &MODULE);
    assert_eq!(RESIZE_HEIGHT_DECREASE.name(), "resize-height-decrease");
}

#[test]
fn test_resize_width_increase_id() {
    assert_eq!(RESIZE_WIDTH_INCREASE.module(), &MODULE);
    assert_eq!(RESIZE_WIDTH_INCREASE.name(), "resize-width-increase");
}

#[test]
fn test_resize_width_decrease_id() {
    assert_eq!(RESIZE_WIDTH_DECREASE.module(), &MODULE);
    assert_eq!(RESIZE_WIDTH_DECREASE.name(), "resize-width-decrease");
}

#[test]
fn test_resize_equal_id() {
    assert_eq!(RESIZE_EQUAL.module(), &MODULE);
    assert_eq!(RESIZE_EQUAL.name(), "resize-equal");
}

#[test]
fn test_resize_max_height_id() {
    assert_eq!(RESIZE_MAX_HEIGHT.module(), &MODULE);
    assert_eq!(RESIZE_MAX_HEIGHT.name(), "resize-max-height");
}

#[test]
fn test_resize_max_width_id() {
    assert_eq!(RESIZE_MAX_WIDTH.module(), &MODULE);
    assert_eq!(RESIZE_MAX_WIDTH.name(), "resize-max-width");
}

// =========================================================================
// Window Movement IDs
// =========================================================================

#[test]
fn test_move_window_left_id() {
    assert_eq!(MOVE_WINDOW_LEFT.module(), &MODULE);
    assert_eq!(MOVE_WINDOW_LEFT.name(), "move-window-left");
}

#[test]
fn test_move_window_down_id() {
    assert_eq!(MOVE_WINDOW_DOWN.module(), &MODULE);
    assert_eq!(MOVE_WINDOW_DOWN.name(), "move-window-down");
}

#[test]
fn test_move_window_up_id() {
    assert_eq!(MOVE_WINDOW_UP.module(), &MODULE);
    assert_eq!(MOVE_WINDOW_UP.name(), "move-window-up");
}

#[test]
fn test_move_window_right_id() {
    assert_eq!(MOVE_WINDOW_RIGHT.module(), &MODULE);
    assert_eq!(MOVE_WINDOW_RIGHT.name(), "move-window-right");
}

#[test]
fn test_rotate_windows_id() {
    assert_eq!(ROTATE_WINDOWS.module(), &MODULE);
    assert_eq!(ROTATE_WINDOWS.name(), "rotate-windows");
}

#[test]
fn test_rotate_windows_reverse_id() {
    assert_eq!(ROTATE_WINDOWS_REVERSE.module(), &MODULE);
    assert_eq!(ROTATE_WINDOWS_REVERSE.name(), "rotate-windows-reverse");
}

#[test]
fn test_swap_window_id() {
    assert_eq!(SWAP_WINDOW.module(), &MODULE);
    assert_eq!(SWAP_WINDOW.name(), "swap-window");
}

// =========================================================================
// Tab Operations IDs
// =========================================================================

#[test]
fn test_move_to_new_tab_id() {
    assert_eq!(MOVE_TO_NEW_TAB.module(), &MODULE);
    assert_eq!(MOVE_TO_NEW_TAB.name(), "move-to-new-tab");
}

// =========================================================================
// Float Zone Operations IDs
// =========================================================================

#[test]
fn test_toggle_float_id() {
    assert_eq!(TOGGLE_FLOAT.module(), &MODULE);
    assert_eq!(TOGGLE_FLOAT.name(), "toggle-float");
}

#[test]
fn test_raise_float_id() {
    assert_eq!(RAISE_FLOAT.module(), &MODULE);
    assert_eq!(RAISE_FLOAT.name(), "raise-float");
}

#[test]
fn test_lower_float_id() {
    assert_eq!(LOWER_FLOAT.module(), &MODULE);
    assert_eq!(LOWER_FLOAT.name(), "lower-float");
}

// =========================================================================
// Layer Opacity Operations IDs (#400)
// =========================================================================

#[test]
fn test_layer_opacity_set_id() {
    assert_eq!(LAYER_OPACITY_SET.module(), &MODULE);
    assert_eq!(LAYER_OPACITY_SET.name(), "layer-opacity-set");
}

#[test]
fn test_layer_opacity_increase_id() {
    assert_eq!(LAYER_OPACITY_INCREASE.module(), &MODULE);
    assert_eq!(LAYER_OPACITY_INCREASE.name(), "layer-opacity-increase");
}

#[test]
fn test_layer_opacity_decrease_id() {
    assert_eq!(LAYER_OPACITY_DECREASE.module(), &MODULE);
    assert_eq!(LAYER_OPACITY_DECREASE.name(), "layer-opacity-decrease");
}

// =========================================================================
// Tab Operations IDs (#401)
// =========================================================================

#[test]
fn test_tab_new_id() {
    assert_eq!(TAB_NEW.module(), &MODULE);
    assert_eq!(TAB_NEW.name(), "tab-new");
}

#[test]
fn test_tab_close_id() {
    assert_eq!(TAB_CLOSE.module(), &MODULE);
    assert_eq!(TAB_CLOSE.name(), "tab-close");
}

#[test]
fn test_tab_next_id() {
    assert_eq!(TAB_NEXT.module(), &MODULE);
    assert_eq!(TAB_NEXT.name(), "tab-next");
}

#[test]
fn test_tab_prev_id() {
    assert_eq!(TAB_PREV.module(), &MODULE);
    assert_eq!(TAB_PREV.name(), "tab-prev");
}

#[test]
fn test_tab_goto_id() {
    assert_eq!(TAB_GOTO.module(), &MODULE);
    assert_eq!(TAB_GOTO.name(), "tab-goto");
}

// =========================================================================
// Uniqueness and equality
// =========================================================================

#[test]
fn test_all_command_ids_are_unique() {
    let all_ids = [
        &FOCUS_LEFT,
        &FOCUS_DOWN,
        &FOCUS_UP,
        &FOCUS_RIGHT,
        &FOCUS_NEXT,
        &FOCUS_PREV,
        &SPLIT_HORIZONTAL,
        &SPLIT_VERTICAL,
        &SPLIT_NEW,
        &CLOSE_WINDOW,
        &CLOSE_OTHERS,
        &RESIZE_HEIGHT_INCREASE,
        &RESIZE_HEIGHT_DECREASE,
        &RESIZE_WIDTH_INCREASE,
        &RESIZE_WIDTH_DECREASE,
        &RESIZE_EQUAL,
        &RESIZE_MAX_HEIGHT,
        &RESIZE_MAX_WIDTH,
        &MOVE_WINDOW_LEFT,
        &MOVE_WINDOW_DOWN,
        &MOVE_WINDOW_UP,
        &MOVE_WINDOW_RIGHT,
        &ROTATE_WINDOWS,
        &ROTATE_WINDOWS_REVERSE,
        &SWAP_WINDOW,
        &MOVE_TO_NEW_TAB,
        &TOGGLE_FLOAT,
        &RAISE_FLOAT,
        &LOWER_FLOAT,
        &LAYER_OPACITY_SET,
        &LAYER_OPACITY_INCREASE,
        &LAYER_OPACITY_DECREASE,
        &TAB_NEW,
        &TAB_CLOSE,
        &TAB_NEXT,
        &TAB_PREV,
        &TAB_GOTO,
    ];
    // Check that all IDs are unique (no duplicates)
    for (i, a) in all_ids.iter().enumerate() {
        for (j, b) in all_ids.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "IDs at positions {i} and {j} are duplicates");
            }
        }
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_all_command_ids_belong_to_window_ops_module() {
    let all_ids = [
        &FOCUS_LEFT,
        &FOCUS_DOWN,
        &FOCUS_UP,
        &FOCUS_RIGHT,
        &FOCUS_NEXT,
        &FOCUS_PREV,
        &SPLIT_HORIZONTAL,
        &SPLIT_VERTICAL,
        &SPLIT_NEW,
        &CLOSE_WINDOW,
        &CLOSE_OTHERS,
        &RESIZE_HEIGHT_INCREASE,
        &RESIZE_HEIGHT_DECREASE,
        &RESIZE_WIDTH_INCREASE,
        &RESIZE_WIDTH_DECREASE,
        &RESIZE_EQUAL,
        &RESIZE_MAX_HEIGHT,
        &RESIZE_MAX_WIDTH,
        &MOVE_WINDOW_LEFT,
        &MOVE_WINDOW_DOWN,
        &MOVE_WINDOW_UP,
        &MOVE_WINDOW_RIGHT,
        &ROTATE_WINDOWS,
        &ROTATE_WINDOWS_REVERSE,
        &SWAP_WINDOW,
        &MOVE_TO_NEW_TAB,
        &TOGGLE_FLOAT,
        &RAISE_FLOAT,
        &LOWER_FLOAT,
        &LAYER_OPACITY_SET,
        &LAYER_OPACITY_INCREASE,
        &LAYER_OPACITY_DECREASE,
        &TAB_NEW,
        &TAB_CLOSE,
        &TAB_NEXT,
        &TAB_PREV,
        &TAB_GOTO,
    ];
    for id in &all_ids {
        assert_eq!(
            id.module(),
            &MODULE,
            "Command '{}' does not belong to window-ops module",
            id.name()
        );
    }
}

#[test]
fn test_command_id_equality() {
    // Two identical CommandIds should be equal
    let dup = CommandId::new(MODULE, "focus-left");
    assert_eq!(FOCUS_LEFT, dup);
}

#[test]
fn test_command_id_inequality_different_name() {
    assert_ne!(FOCUS_LEFT, FOCUS_RIGHT);
}

#[test]
fn test_command_id_inequality_different_module() {
    let other_module = ModuleId::new("other-module");
    let other_cmd = CommandId::new(other_module, "focus-left");
    assert_ne!(FOCUS_LEFT, other_cmd);
}
