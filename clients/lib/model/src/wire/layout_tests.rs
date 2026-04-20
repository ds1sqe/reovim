use super::*;

#[test]
fn test_single_layout() {
    let layout = LogicalLayout::single(1, 100);
    assert!(layout.is_leaf());
    assert_eq!(layout.buffer_id(), Some(1));
    assert_eq!(layout.window_count(), 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_hsplit_layout() {
    let layout =
        LogicalLayout::hsplit(vec![LogicalLayout::single(1, 100), LogicalLayout::single(2, 101)]);
    assert!(!layout.is_leaf());
    assert_eq!(layout.buffer_id(), None);
    assert_eq!(layout.window_count(), 2);

    if let LogicalLayout::Split {
        direction, ratios, ..
    } = layout
    {
        assert_eq!(direction, SplitDirection::Horizontal);
        assert_eq!(ratios.len(), 2);
        assert!((ratios[0] - 0.5).abs() < f32::EPSILON);
    } else {
        panic!("Expected Split layout");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_vsplit_layout() {
    let layout = LogicalLayout::vsplit(vec![
        LogicalLayout::single(1, 100),
        LogicalLayout::single(2, 101),
        LogicalLayout::single(3, 102),
    ]);
    assert_eq!(layout.window_count(), 3);

    if let LogicalLayout::Split {
        direction, ratios, ..
    } = layout
    {
        assert_eq!(direction, SplitDirection::Vertical);
        assert_eq!(ratios.len(), 3);
    } else {
        panic!("Expected Split layout");
    }
}

#[test]
fn test_nested_layout() {
    // Two columns, left column has two rows
    let layout = LogicalLayout::vsplit(vec![
        LogicalLayout::hsplit(vec![LogicalLayout::single(1, 100), LogicalLayout::single(2, 101)]),
        LogicalLayout::single(3, 102),
    ]);
    assert_eq!(layout.window_count(), 3);
}

#[test]
fn test_tabs_layout() {
    let layout =
        LogicalLayout::tabs(vec![LogicalLayout::single(1, 100), LogicalLayout::single(2, 101)], 0);
    assert_eq!(layout.window_count(), 2);
    assert!(!layout.is_leaf());
}

#[test]
fn test_find_viewport() {
    let layout = LogicalLayout::vsplit(vec![
        LogicalLayout::hsplit(vec![LogicalLayout::single(1, 100), LogicalLayout::single(2, 101)]),
        LogicalLayout::single(3, 102),
    ]);

    assert_eq!(layout.find_viewport(100), Some(vec![0, 0]));
    assert_eq!(layout.find_viewport(101), Some(vec![0, 1]));
    assert_eq!(layout.find_viewport(102), Some(vec![1]));
    assert_eq!(layout.find_viewport(999), None);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_split_with_ratios() {
    let layout = LogicalLayout::split_with_ratios(
        SplitDirection::Horizontal,
        vec![LogicalLayout::single(1, 100), LogicalLayout::single(2, 101)],
        vec![0.7, 0.3],
    );
    if let LogicalLayout::Split {
        direction,
        ratios,
        children,
    } = layout
    {
        assert_eq!(direction, SplitDirection::Horizontal);
        assert_eq!(ratios.len(), 2);
        assert!((ratios[0] - 0.7).abs() < f32::EPSILON);
        assert!((ratios[1] - 0.3).abs() < f32::EPSILON);
        assert_eq!(children.len(), 2);
    } else {
        panic!("Expected Split layout");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_tabs_constructor() {
    let layout = LogicalLayout::tabs(
        vec![
            LogicalLayout::single(1, 100),
            LogicalLayout::single(2, 101),
            LogicalLayout::single(3, 102),
        ],
        1,
    );
    if let LogicalLayout::Tabs { tabs, active } = &layout {
        assert_eq!(tabs.len(), 3);
        assert_eq!(*active, 1);
    } else {
        panic!("Expected Tabs layout");
    }
}

#[test]
fn test_tabs_window_count() {
    let layout = LogicalLayout::tabs(
        vec![
            LogicalLayout::single(1, 100),
            LogicalLayout::vsplit(vec![
                LogicalLayout::single(2, 101),
                LogicalLayout::single(3, 102),
            ]),
        ],
        0,
    );
    assert_eq!(layout.window_count(), 3);
}

#[test]
fn test_find_viewport_in_tabs() {
    let layout =
        LogicalLayout::tabs(vec![LogicalLayout::single(1, 100), LogicalLayout::single(2, 200)], 0);
    assert_eq!(layout.find_viewport(100), Some(vec![0]));
    assert_eq!(layout.find_viewport(200), Some(vec![1]));
    assert_eq!(layout.find_viewport(999), None);
}

#[test]
fn test_find_viewport_single_not_found() {
    let layout = LogicalLayout::single(1, 100);
    assert_eq!(layout.find_viewport(200), None);
}

#[test]
fn test_layout_serialize_deserialize() {
    let layout =
        LogicalLayout::vsplit(vec![LogicalLayout::single(1, 100), LogicalLayout::single(2, 101)]);
    let json = serde_json::to_string(&layout).unwrap();
    let restored: LogicalLayout = serde_json::from_str(&json).unwrap();
    assert_eq!(layout, restored);
}

#[test]
fn test_tabs_serialize_deserialize() {
    let layout = LogicalLayout::tabs(vec![LogicalLayout::single(1, 100)], 0);
    let json = serde_json::to_string(&layout).unwrap();
    let restored: LogicalLayout = serde_json::from_str(&json).unwrap();
    assert_eq!(layout, restored);
}
