use crate::{
    core::ModeId,
    ipc::events::kernel::{
        BufferCreated, BufferModified, CursorMoved, LayoutChangeKind, LayoutChanged, ModeChanged,
        Modification, Shutdown, SplitDirection, priority,
    },
};

#[test]
fn test_buffer_created() {
    let event = BufferCreated { buffer_id: 42 };
    assert_eq!(event.buffer_id, 42);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_buffer_modified_insert() {
    let event = BufferModified {
        buffer_id: 1,
        modification: Modification::Insert {
            start: (0, 0),
            text: "hello".to_string(),
            start_byte: 0,
        },
    };
    assert_eq!(event.buffer_id, 1);
    if let Modification::Insert { start, text, .. } = event.modification {
        assert_eq!(start, (0, 0));
        assert_eq!(text, "hello");
    } else {
        panic!("Expected Insert modification");
    }
}

#[test]
fn test_mode_changed() {
    let event = ModeChanged::new("Normal", "Insert");
    assert_eq!(event.from, "Normal");
    assert_eq!(event.to, "Insert");
    assert!(event.target_mode().is_none());
    assert!(!event.has_target_mode());
}

#[test]
fn test_mode_changed_with_mode_id() {
    use crate::api::ModuleId;

    let module = ModuleId::new("editor");
    let mode_id = ModeId::new(module, "visual");
    let event = ModeChanged::with_mode_id("normal", mode_id.clone());

    assert_eq!(event.from, "normal");
    assert_eq!(event.to, "visual");
    assert!(event.has_target_mode());
    assert_eq!(event.target_mode(), Some(&mode_id));
}

#[test]
fn test_mode_changed_backward_compat() {
    // Test that old-style struct initialization still works for comparison
    let event = ModeChanged::new("Normal", "Insert");
    assert_eq!(event.from, "Normal");
    assert_eq!(event.to, "Insert");
}

#[test]
fn test_cursor_moved() {
    let event = CursorMoved {
        buffer_id: 1,
        from: (0, 0),
        to: (10, 5),
    };
    assert_eq!(event.from, (0, 0));
    assert_eq!(event.to, (10, 5));
}

#[test]
fn test_shutdown_default() {
    let event = Shutdown;
    assert_eq!(event, Shutdown);
}

#[test]
fn test_priority_constants() {
    const {
        assert!(priority::CRITICAL < priority::CORE);
        assert!(priority::CORE < priority::NORMAL);
        assert!(priority::NORMAL < priority::PLUGIN);
        assert!(priority::PLUGIN < priority::LOW);
    }
}

// Layout event tests (#444)

#[test]
fn test_split_direction() {
    let h = SplitDirection::Horizontal;
    let v = SplitDirection::Vertical;
    assert_ne!(h, v);
    assert_eq!(h, SplitDirection::Horizontal);
    assert_eq!(v, SplitDirection::Vertical);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_layout_change_kind_split() {
    let kind = LayoutChangeKind::Split {
        new_window: 1,
        direction: SplitDirection::Vertical,
    };
    if let LayoutChangeKind::Split {
        new_window,
        direction,
    } = kind
    {
        assert_eq!(new_window, 1);
        assert_eq!(direction, SplitDirection::Vertical);
    } else {
        panic!("Expected Split kind");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_layout_change_kind_close() {
    let kind = LayoutChangeKind::Close {
        closed_window: 2,
        new_focus: Some(1),
    };
    if let LayoutChangeKind::Close {
        closed_window,
        new_focus,
    } = kind
    {
        assert_eq!(closed_window, 2);
        assert_eq!(new_focus, Some(1));
    } else {
        panic!("Expected Close kind");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_layout_change_kind_focus() {
    let kind = LayoutChangeKind::Focus {
        from: Some(0),
        to: 1,
    };
    if let LayoutChangeKind::Focus { from, to } = kind {
        assert_eq!(from, Some(0));
        assert_eq!(to, 1);
    } else {
        panic!("Expected Focus kind");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_layout_change_kind_resize() {
    let kind = LayoutChangeKind::Resize { window: 0 };
    if let LayoutChangeKind::Resize { window } = kind {
        assert_eq!(window, 0);
    } else {
        panic!("Expected Resize kind");
    }
}

#[test]
fn test_layout_change_kind_equalize() {
    let kind = LayoutChangeKind::Equalize;
    assert_eq!(kind, LayoutChangeKind::Equalize);
}

#[test]
fn test_layout_changed_event() {
    let event = LayoutChanged {
        kind: LayoutChangeKind::Split {
            new_window: 1,
            direction: SplitDirection::Horizontal,
        },
        window_count: 2,
        focused_window: Some(1),
    };
    assert_eq!(event.window_count, 2);
    assert_eq!(event.focused_window, Some(1));
}

#[test]
fn test_layout_changed_no_focus() {
    let event = LayoutChanged {
        kind: LayoutChangeKind::Close {
            closed_window: 0,
            new_focus: None,
        },
        window_count: 0,
        focused_window: None,
    };
    assert_eq!(event.window_count, 0);
    assert!(event.focused_window.is_none());
}
