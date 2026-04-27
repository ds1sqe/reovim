use super::*;

#[test]
fn test_render_data_new() {
    let lines = vec!["Hello".to_string(), "World".to_string()];
    let data = RenderData::new(lines);

    assert_eq!(data.line_count(), 2);
    assert_eq!(data.highlights.len(), 2);
    assert_eq!(data.gutter_decorations.len(), 2);
}

#[test]
fn test_render_data_empty() {
    let data = RenderData::empty();
    assert_eq!(data.line_count(), 0);
}

#[test]
fn test_render_data_add_highlight() {
    let lines = vec!["Hello World".to_string()];
    let mut data = RenderData::new(lines);

    data.add_highlight(0, 0, 5, Style::default());
    data.add_highlight(0, 6, 11, Style::default());

    assert_eq!(data.highlights[0].len(), 2);
    assert_eq!(data.highlights[0][0], (0, 5, Style::default()));
}

#[test]
fn test_render_data_add_highlight_out_of_bounds() {
    let lines = vec!["Hello".to_string()];
    let mut data = RenderData::new(lines);

    // Should not panic, just ignored
    data.add_highlight(10, 0, 5, Style::default());

    assert_eq!(data.highlights[0].len(), 0);
}

#[test]
fn test_render_data_set_gutter() {
    let lines = vec!["Line 1".to_string(), "Line 2".to_string()];
    let mut data = RenderData::new(lines);

    let decoration = GutterDecoration {
        char: 'E',
        style: Style::default(),
        priority: 10,
    };

    data.set_gutter(0, decoration);

    assert!(data.gutter_decorations[0].is_some());
    assert_eq!(data.gutter_decorations[0].as_ref().unwrap().char, 'E');
}

#[test]
fn test_render_data_gutter_priority() {
    let lines = vec!["Line 1".to_string()];
    let mut data = RenderData::new(lines);

    // Set low priority decoration
    data.set_gutter(
        0,
        GutterDecoration {
            char: 'W',
            style: Style::default(),
            priority: 5,
        },
    );

    // Higher priority should replace
    data.set_gutter(
        0,
        GutterDecoration {
            char: 'E',
            style: Style::default(),
            priority: 10,
        },
    );

    assert_eq!(data.gutter_decorations[0].as_ref().unwrap().char, 'E');

    // Lower priority should not replace
    data.set_gutter(
        0,
        GutterDecoration {
            char: 'I',
            style: Style::default(),
            priority: 1,
        },
    );

    assert_eq!(data.gutter_decorations[0].as_ref().unwrap().char, 'E');
}

#[test]
fn test_render_data_add_inline() {
    let lines = vec!["Hello".to_string()];
    let mut data = RenderData::new(lines);

    data.add_inline(InlineDecoration {
        line: 0,
        column: 5,
        text: " (world)".to_string(),
        style: Style::default(),
        is_virtual: true,
    });

    assert_eq!(data.inline_decorations.len(), 1);
    assert!(data.inline_decorations[0].is_virtual);
}

#[test]
fn test_render_context_new() {
    let bounds = Rect::new(0, 0, 80, 24);
    let ctx = RenderContext::new(bounds);

    assert_eq!(ctx.bounds.width, 80);
    assert_eq!(ctx.bounds.height, 24);
    assert!(!ctx.focused);
}

#[test]
fn test_render_context_builder() {
    let bounds = Rect::new(0, 0, 80, 24);
    let ctx = RenderContext::new(bounds)
        .focused(true)
        .cursor(10, 5)
        .viewport(5, 20)
        .total_lines(100);

    assert!(ctx.focused);
    assert_eq!(ctx.cursor_line, 10);
    assert_eq!(ctx.cursor_column, 5);
    assert_eq!(ctx.viewport_top, 5);
    assert_eq!(ctx.visible_lines, 20);
    assert_eq!(ctx.total_lines, 100);
}

#[test]
fn test_render_context_is_line_visible() {
    let bounds = Rect::new(0, 0, 80, 24);
    let ctx = RenderContext::new(bounds).viewport(10, 20);

    assert!(!ctx.is_line_visible(9)); // Before viewport
    assert!(ctx.is_line_visible(10)); // First visible
    assert!(ctx.is_line_visible(20)); // Middle
    assert!(ctx.is_line_visible(29)); // Last visible
    assert!(!ctx.is_line_visible(30)); // After viewport
}

#[test]
fn test_render_context_line_to_row() {
    let bounds = Rect::new(0, 0, 80, 24);
    let ctx = RenderContext::new(bounds).viewport(10, 20);

    assert_eq!(ctx.line_to_row(9), None);
    assert_eq!(ctx.line_to_row(10), Some(0));
    assert_eq!(ctx.line_to_row(15), Some(5));
    assert_eq!(ctx.line_to_row(29), Some(19));
    assert_eq!(ctx.line_to_row(30), None);
}

// Test stage for pipeline testing
struct TestStage {
    name: &'static str,
    priority: u32,
    highlight: Option<(usize, usize, usize)>, // line, start, end
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl RenderStage for TestStage {
    fn name(&self) -> &str {
        self.name
    }

    fn process(&self, data: &mut RenderData, _context: &RenderContext) {
        if let Some((line, start, end)) = self.highlight {
            data.add_highlight(line, start, end, Style::default());
        }
    }

    fn priority(&self) -> u32 {
        self.priority
    }
}

#[test]
fn test_execute_pipeline() {
    let lines = vec!["Hello World".to_string()];
    let mut data = RenderData::new(lines);
    let bounds = Rect::new(0, 0, 80, 24);
    let context = RenderContext::new(bounds);

    let mut stages: Vec<Box<dyn RenderStage>> = vec![
        Box::new(TestStage {
            name: "Stage1",
            priority: 20,
            highlight: Some((0, 6, 11)),
        }),
        Box::new(TestStage {
            name: "Stage2",
            priority: 10,
            highlight: Some((0, 0, 5)),
        }),
    ];

    execute_pipeline(&mut stages, &mut data, &context);

    // Both highlights should be added
    assert_eq!(data.highlights[0].len(), 2);
}

#[test]
fn test_execute_pipeline_sorted() {
    let lines = vec!["Test".to_string()];
    let mut data = RenderData::new(lines);
    let bounds = Rect::new(0, 0, 80, 24);
    let context = RenderContext::new(bounds);

    let stages: Vec<Box<dyn RenderStage>> = vec![Box::new(TestStage {
        name: "TestStage",
        priority: 50,
        highlight: Some((0, 0, 4)),
    })];

    execute_pipeline_sorted(&stages, &mut data, &context);

    assert_eq!(data.highlights[0].len(), 1);
}

#[test]
fn test_gutter_decoration() {
    let decoration = GutterDecoration {
        char: '●',
        style: Style::default(),
        priority: 5,
    };

    assert_eq!(decoration.char, '●');
    assert_eq!(decoration.priority, 5);
}

#[test]
fn test_inline_decoration() {
    let decoration = InlineDecoration {
        line: 5,
        column: 10,
        text: "virtual text".to_string(),
        style: Style::default(),
        is_virtual: true,
    };

    assert_eq!(decoration.line, 5);
    assert_eq!(decoration.column, 10);
    assert!(decoration.is_virtual);
}

#[test]
fn test_set_gutter_out_of_bounds() {
    let lines = vec!["Line 1".to_string()];
    let mut data = RenderData::new(lines);

    // Line index 10 is out of bounds (only 1 line exists)
    data.set_gutter(
        10,
        GutterDecoration {
            char: 'X',
            style: Style::default(),
            priority: 5,
        },
    );

    // Original line should be unaffected
    assert!(data.gutter_decorations[0].is_none());
}

// Stage that uses the default priority (100)
struct DefaultPriorityStage;

#[cfg_attr(coverage_nightly, coverage(off))]
impl RenderStage for DefaultPriorityStage {
    #[allow(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        "DefaultPriority"
    }

    fn process(&self, data: &mut RenderData, _context: &RenderContext) {
        data.add_highlight(0, 0, 1, Style::default());
    }

    // Does NOT override priority() - exercises the default impl returning 100
}

#[test]
fn test_render_stage_default_priority() {
    let stage = DefaultPriorityStage;
    assert_eq!(stage.priority(), 100);
}

#[test]
fn test_execute_pipeline_with_default_priority() {
    let lines = vec!["Test".to_string()];
    let mut data = RenderData::new(lines);
    let bounds = Rect::new(0, 0, 80, 24);
    let context = RenderContext::new(bounds);

    let mut stages: Vec<Box<dyn RenderStage>> = vec![
        Box::new(DefaultPriorityStage),
        Box::new(TestStage {
            name: "HighPriority",
            priority: 200,
            highlight: Some((0, 1, 4)),
        }),
    ];

    execute_pipeline(&mut stages, &mut data, &context);

    // Both stages should have run: default priority (100) first, then 200
    assert_eq!(data.highlights[0].len(), 2);
}
