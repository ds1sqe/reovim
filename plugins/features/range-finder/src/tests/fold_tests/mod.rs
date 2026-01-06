//! Tests for the fold plugin

use std::sync::Arc;

use reovim_core::{
    component::RenderContext,
    highlight::{ColorMode, Theme},
    render::{LineVisibility, RenderData, RenderStage},
};

use crate::{
    stage::FoldRenderStage,
    state::{FoldKind, FoldRange, FoldState, SharedFoldManager},
};

#[test]
fn test_fold_range() {
    let range = FoldRange::new(5, 10, FoldKind::Function, "fn foo()".to_string());

    assert!(range.is_foldable());
    assert_eq!(range.line_count(), 6);
    assert!(!range.contains_line(5)); // Start line not contained
    assert!(range.contains_line(6));
    assert!(range.contains_line(10));
    assert!(!range.contains_line(11));
}

#[test]
fn test_fold_state_toggle() {
    let mut state = FoldState::new();
    state.set_ranges(vec![
        FoldRange::new(0, 5, FoldKind::Function, "fn one()".to_string()),
        FoldRange::new(10, 15, FoldKind::Function, "fn two()".to_string()),
    ]);

    // Initially not collapsed
    assert!(!state.is_collapsed(0));
    assert!(!state.is_line_hidden(3));

    // Toggle to collapse
    assert!(state.toggle(0));
    assert!(state.is_collapsed(0));
    assert!(state.is_line_hidden(3));

    // Toggle to expand
    assert!(state.toggle(0));
    assert!(!state.is_collapsed(0));
    assert!(!state.is_line_hidden(3));
}

#[test]
fn test_fold_marker() {
    let mut state = FoldState::new();
    state.set_ranges(vec![FoldRange::new(
        0,
        5,
        FoldKind::Function,
        "fn foo()".to_string(),
    )]);

    // Not collapsed - no marker
    assert!(state.get_fold_marker(0).is_none());

    // Collapse
    state.close(0);

    // Should have marker
    let marker = state.get_fold_marker(0);
    assert!(marker.is_some());
    let (count, preview) = marker.unwrap();
    assert_eq!(count, 5); // 5 hidden lines (1-5)
    assert_eq!(preview, "fn foo()");
}

#[test]
fn test_open_close_all() {
    let mut state = FoldState::new();
    state.set_ranges(vec![
        FoldRange::new(0, 5, FoldKind::Function, "fn one()".to_string()),
        FoldRange::new(10, 15, FoldKind::Function, "fn two()".to_string()),
    ]);

    state.close_all();
    assert!(state.is_collapsed(0));
    assert!(state.is_collapsed(10));

    state.open_all();
    assert!(!state.is_collapsed(0));
    assert!(!state.is_collapsed(10));
}

#[test]
fn test_display_line_mapping() {
    let mut state = FoldState::new();
    state.set_ranges(vec![FoldRange::new(
        2,
        5,
        FoldKind::Function,
        "fn test()".to_string(),
    )]);

    // Close the fold
    state.close(2);

    // Lines 0, 1, 2 are visible (2 is fold header)
    // Lines 3, 4, 5 are hidden
    // Line 6+ visible again

    assert!(!state.is_line_hidden(0));
    assert!(!state.is_line_hidden(1));
    assert!(!state.is_line_hidden(2)); // Fold header is visible
    assert!(state.is_line_hidden(3));
    assert!(state.is_line_hidden(4));
    assert!(state.is_line_hidden(5));
    assert!(!state.is_line_hidden(6));
}

#[test]
fn test_visible_line_count() {
    let mut state = FoldState::new();
    state.set_ranges(vec![FoldRange::new(
        2,
        5,
        FoldKind::Function,
        "fn test()".to_string(),
    )]);

    // 10 total lines, no folds collapsed
    assert_eq!(state.visible_line_count(10), 10);

    // Close fold (hides lines 3, 4, 5)
    state.close(2);
    assert_eq!(state.visible_line_count(10), 7);
}

// === Render Stage Tests ===

#[test]
fn test_fold_render_stage_no_folds() {
    let manager = Arc::new(SharedFoldManager::new());
    let stage = FoldRenderStage::new(manager);

    // Create mock render data directly
    let render_data = RenderData {
        lines: vec![
            "line 1".to_string(),
            "line 2".to_string(),
            "line 3".to_string(),
        ],
        visibility: vec![LineVisibility::Visible; 3],
        highlights: vec![Vec::new(); 3],
        decorations: vec![Vec::new(); 3],
        signs: vec![None; 3],
        virtual_texts: vec![None; 3],
        buffer_id: 1,
        window_id: 0,
        window_bounds: reovim_core::render::Bounds {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        },
        cursor: (0, 0),
    };

    // Create render context
    let theme = Theme::default();
    let ctx = RenderContext::new(80, 24, &theme, ColorMode::TrueColor);

    // Transform should preserve all lines as visible
    let result = stage.transform(render_data, &ctx);

    assert_eq!(result.visibility.len(), 3);
    assert!(matches!(result.visibility[0], LineVisibility::Visible));
    assert!(matches!(result.visibility[1], LineVisibility::Visible));
    assert!(matches!(result.visibility[2], LineVisibility::Visible));
}

#[test]
fn test_fold_render_stage_with_collapsed_fold() {
    let manager = Arc::new(SharedFoldManager::new());

    // Set up a fold range and collapse it
    manager.with_mut(|m| {
        m.set_ranges(
            1,
            vec![FoldRange::new(
                1,
                3,
                FoldKind::Function,
                "fn test()".to_string(),
            )],
        );
        m.close(1, 1);
    });

    let stage = FoldRenderStage::new(manager);

    // Create mock render data with 5 lines
    let render_data = RenderData {
        lines: vec![
            "line 0".to_string(),
            "line 1".to_string(),
            "line 2".to_string(),
            "line 3".to_string(),
            "line 4".to_string(),
        ],
        visibility: vec![LineVisibility::Visible; 5],
        highlights: vec![Vec::new(); 5],
        decorations: vec![Vec::new(); 5],
        signs: vec![None; 5],
        virtual_texts: vec![None; 5],
        buffer_id: 1,
        window_id: 0,
        window_bounds: reovim_core::render::Bounds {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        },
        cursor: (0, 0),
    };

    let theme = Theme::default();
    let ctx = RenderContext::new(80, 24, &theme, ColorMode::TrueColor);

    // Transform should mark lines as hidden/fold marker
    let result = stage.transform(render_data, &ctx);

    assert_eq!(result.visibility.len(), 5);
    assert!(matches!(result.visibility[0], LineVisibility::Visible));

    // Line 1 should be fold marker
    match &result.visibility[1] {
        LineVisibility::FoldMarker {
            preview,
            hidden_lines,
        } => {
            assert_eq!(preview, "fn test()");
            assert_eq!(*hidden_lines, 2); // Lines 2-3 are hidden
        }
        _ => panic!("Expected FoldMarker at line 1"),
    }

    // Lines 2-3 should be hidden
    assert!(matches!(result.visibility[2], LineVisibility::Hidden));
    assert!(matches!(result.visibility[3], LineVisibility::Hidden));

    // Line 4 should be visible
    assert!(matches!(result.visibility[4], LineVisibility::Visible));
}

#[test]
fn test_fold_render_stage_multiple_folds() {
    let manager = Arc::new(SharedFoldManager::new());

    // Set up multiple fold ranges
    manager.with_mut(|m| {
        m.set_ranges(
            2,
            vec![
                FoldRange::new(1, 3, FoldKind::Function, "fn one()".to_string()),
                FoldRange::new(5, 7, FoldKind::Function, "fn two()".to_string()),
            ],
        );
        m.close(2, 1); // Collapse first fold
        m.close(2, 5); // Collapse second fold
    });

    let stage = FoldRenderStage::new(manager);

    // Create mock render data with 9 lines
    let render_data = RenderData {
        lines: (0..9).map(|i| format!("line {i}")).collect(),
        visibility: vec![LineVisibility::Visible; 9],
        highlights: vec![Vec::new(); 9],
        decorations: vec![Vec::new(); 9],
        signs: vec![None; 9],
        virtual_texts: vec![None; 9],
        buffer_id: 2,
        window_id: 0,
        window_bounds: reovim_core::render::Bounds {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        },
        cursor: (0, 0),
    };

    let theme = Theme::default();
    let ctx = RenderContext::new(80, 24, &theme, ColorMode::TrueColor);

    let result = stage.transform(render_data, &ctx);

    assert_eq!(result.visibility.len(), 9);

    // Line 0 visible
    assert!(matches!(result.visibility[0], LineVisibility::Visible));

    // Line 1 is fold marker
    assert!(matches!(result.visibility[1], LineVisibility::FoldMarker { .. }));

    // Lines 2-3 hidden (first fold)
    assert!(matches!(result.visibility[2], LineVisibility::Hidden));
    assert!(matches!(result.visibility[3], LineVisibility::Hidden));

    // Line 4 visible
    assert!(matches!(result.visibility[4], LineVisibility::Visible));

    // Line 5 is fold marker
    assert!(matches!(result.visibility[5], LineVisibility::FoldMarker { .. }));

    // Lines 6-7 hidden (second fold)
    assert!(matches!(result.visibility[6], LineVisibility::Hidden));
    assert!(matches!(result.visibility[7], LineVisibility::Hidden));

    // Line 8 visible
    assert!(matches!(result.visibility[8], LineVisibility::Visible));
}
