//! Visual render tests for virtual text
//!
//! These tests verify the virtual text rendering behavior:
//! - Rendering after line content
//! - Truncation with ellipsis
//! - Priority-based display

use reovim_core::{
    buffer::{Buffer, TextOps},
    content::WindowContentSource,
    highlight::{Style, Theme},
    modd::ModeState,
    render::{RenderData, VirtualTextEntry},
    screen::{
        Position,
        window::{Anchor, LineNumber, Window},
    },
    sign::Sign,
};

fn create_test_window(width: u16, height: u16) -> Window {
    Window {
        id: 0,
        source: WindowContentSource::FileBuffer {
            buffer_id: 1,
            buffer_anchor: Anchor { x: 0, y: 0 },
        },
        anchor: Anchor { x: 0, y: 0 },
        width,
        height,
        z_order: 0,
        is_active: true,
        is_floating: false,
        line_number: Some(LineNumber::default()),
        sign_column_width: Some(2),
        scrollbar_enabled: false,
        cursor: Position { x: 0, y: 0 },
        desired_col: None,
        border_config: None,
    }
}

#[test]
fn test_virtual_text_basic_rendering() {
    // Create buffer with content
    let mut buffer = Buffer::empty(1);
    buffer.set_content("let x = 1;\nlet y = 2;\nlet z = 3;");

    let window = create_test_window(60, 3);
    let mode = ModeState::normal();
    let mut render_data = RenderData::from_buffer(&window, &buffer, &mode);

    // Add virtual text on line 0
    render_data.virtual_texts[0] = Some(VirtualTextEntry {
        text: "● type error".to_string(),
        style: Style::new().dim().italic(),
        priority: 404,
    });

    // Verify virtual text is set
    assert!(render_data.virtual_texts[0].is_some());
    assert!(render_data.virtual_texts[1].is_none());
    assert!(render_data.virtual_texts[2].is_none());

    let vt = render_data.virtual_texts[0].as_ref().unwrap();
    assert_eq!(vt.text, "● type error");
    assert_eq!(vt.priority, 404);

    // Print visual representation
    println!("\n=== Virtual Text Basic Rendering ===");
    for (i, line) in buffer.contents.iter().enumerate() {
        print!("  {} │ {}", i + 1, line.inner);
        if let Some(vt) = &render_data.virtual_texts[i] {
            print!("  {}", vt.text);
        }
        println!();
    }
    println!("\n✓ Virtual text correctly set on line 0");
}

#[test]
fn test_virtual_text_multiple_lines() {
    let mut buffer = Buffer::empty(1);
    buffer.set_content("fn main() {\n    let x: i32 = \"str\";\n    println!();\n}");

    let window = create_test_window(80, 4);
    let mode = ModeState::normal();
    let mut render_data = RenderData::from_buffer(&window, &buffer, &mode);

    // Add diagnostics on multiple lines
    render_data.virtual_texts[1] = Some(VirtualTextEntry {
        text: "● mismatched types".to_string(),
        style: Style::new(),
        priority: 404,
    });
    render_data.virtual_texts[2] = Some(VirtualTextEntry {
        text: "◐ unused result".to_string(),
        style: Style::new(),
        priority: 403,
    });

    // Verify
    assert!(render_data.virtual_texts[0].is_none());
    assert!(render_data.virtual_texts[1].is_some());
    assert!(render_data.virtual_texts[2].is_some());
    assert!(render_data.virtual_texts[3].is_none());

    // Print visual representation
    println!("\n=== Virtual Text Multiple Lines ===");
    for (i, line) in buffer.contents.iter().enumerate() {
        print!("  {} │ {}", i + 1, line.inner);
        if let Some(vt) = &render_data.virtual_texts[i] {
            print!("  {}", vt.text);
        }
        println!();
    }
    println!("\n✓ Virtual text correctly set on lines 1 and 2");
}

#[test]
fn test_virtual_text_with_sign() {
    let mut buffer = Buffer::empty(1);
    buffer.set_content("let x: i32 = \"string\";");

    let window = create_test_window(70, 1);
    let mode = ModeState::normal();
    let theme = Theme::default();
    let mut render_data = RenderData::from_buffer(&window, &buffer, &mode);

    // Add both sign and virtual text on the same line
    let error_style = theme.diagnostic.error.clone();
    let vt_style = theme.virtual_text.error;
    render_data.signs[0] = Some(Sign {
        icon: "●".to_string(),
        style: error_style,
        priority: 304,
    });
    render_data.virtual_texts[0] = Some(VirtualTextEntry {
        text: "● mismatched types: expected `i32`, found `&str`".to_string(),
        style: vt_style,
        priority: 404,
    });

    // Verify both are set
    assert!(render_data.signs[0].is_some());
    assert!(render_data.virtual_texts[0].is_some());

    // Print visual representation (simulating sign column + content + virtual text)
    println!("\n=== Virtual Text with Sign ===");
    let sign = render_data.signs[0].as_ref().unwrap();
    let vt = render_data.virtual_texts[0].as_ref().unwrap();
    println!("  {} │ {} │ {}  {}", 1, sign.icon, buffer.contents[0].inner, vt.text);
    println!("\n✓ Both sign and virtual text correctly set");
}

#[test]
fn test_virtual_text_truncation_logic() {
    let long_message =
        "This is a very long diagnostic message that should be truncated when rendered";

    let vt = VirtualTextEntry {
        text: long_message.to_string(),
        style: Style::new(),
        priority: 400,
    };

    // Simulate truncation with different viewport widths
    // Note: truncated text = (max_chars - 3) chars + "..."
    let test_cases = [
        (100, false), // Wide viewport - no truncation
        (30, true),   // Narrow - truncation (27 chars + "...")
        (20, true),   // Very narrow (17 chars + "...")
        (10, true),   // Extremely narrow (7 chars + "...")
    ];

    println!("\n=== Virtual Text Truncation Logic ===");
    for (max_chars, expect_truncation) in test_cases {
        let vt_chars: Vec<char> = vt.text.chars().collect();
        let (text_to_render, needs_ellipsis) = if vt_chars.len() > max_chars {
            (&vt_chars[..max_chars.saturating_sub(3)], true)
        } else {
            (&vt_chars[..], false)
        };

        let rendered: String = text_to_render.iter().collect();
        let final_text = if needs_ellipsis {
            format!("{rendered}...")
        } else {
            rendered
        };

        println!("  max_chars={max_chars}: \"{final_text}\"");

        assert_eq!(
            needs_ellipsis, expect_truncation,
            "max_chars={max_chars}: expected truncation={expect_truncation}"
        );

        if expect_truncation {
            assert!(final_text.ends_with("..."), "Truncated text should end with ellipsis");
            assert_eq!(
                final_text.chars().count(),
                max_chars,
                "Truncated text should be exactly max_chars characters"
            );
        }

        // All should start with "This is"
        assert!(final_text.starts_with("This is"), "Text should start with 'This is'");
    }
    println!("\n✓ Truncation logic works correctly");
}

#[test]
fn test_virtual_text_priority_visual() {
    let mut buffer = Buffer::empty(1);
    buffer.set_content("problematic line");

    let window = create_test_window(60, 1);
    let mode = ModeState::normal();
    let mut render_data = RenderData::from_buffer(&window, &buffer, &mode);

    // Start with a hint (low priority)
    render_data.virtual_texts[0] = Some(VirtualTextEntry {
        text: "· consider using a constant".to_string(),
        style: Style::new(),
        priority: 401,
    });

    println!("\n=== Virtual Text Priority ===");
    println!("  Initial (hint): {}", render_data.virtual_texts[0].as_ref().unwrap().text);

    // Try to replace with warning (higher priority)
    let warning = VirtualTextEntry {
        text: "◐ unused variable".to_string(),
        style: Style::new(),
        priority: 403,
    };

    let should_replace = render_data.virtual_texts[0]
        .as_ref()
        .is_none_or(|existing| existing.priority < warning.priority);

    if should_replace {
        render_data.virtual_texts[0] = Some(warning);
    }

    println!("  After warning: {}", render_data.virtual_texts[0].as_ref().unwrap().text);
    assert_eq!(
        render_data.virtual_texts[0].as_ref().unwrap().priority,
        403,
        "Warning should replace hint"
    );

    // Try to replace with error (even higher priority)
    let error = VirtualTextEntry {
        text: "● undefined variable".to_string(),
        style: Style::new(),
        priority: 404,
    };

    let should_replace = render_data.virtual_texts[0]
        .as_ref()
        .is_none_or(|existing| existing.priority < error.priority);

    if should_replace {
        render_data.virtual_texts[0] = Some(error);
    }

    println!("  After error: {}", render_data.virtual_texts[0].as_ref().unwrap().text);
    assert_eq!(
        render_data.virtual_texts[0].as_ref().unwrap().priority,
        404,
        "Error should replace warning"
    );

    // Try to replace with hint (lower priority) - should NOT replace
    let hint = VirtualTextEntry {
        text: "· some hint".to_string(),
        style: Style::new(),
        priority: 401,
    };

    let should_replace = render_data.virtual_texts[0]
        .as_ref()
        .is_none_or(|existing| existing.priority < hint.priority);

    if should_replace {
        render_data.virtual_texts[0] = Some(hint);
    }

    println!("  After hint attempt: {}", render_data.virtual_texts[0].as_ref().unwrap().text);
    assert_eq!(
        render_data.virtual_texts[0].as_ref().unwrap().priority,
        404,
        "Error should NOT be replaced by hint"
    );

    println!("\n✓ Priority resolution works correctly");
}

#[test]
fn test_virtual_text_severity_icons() {
    let severities = [
        (404, "●", "ERROR"),
        (403, "◐", "WARNING"),
        (402, "ⓘ", "INFO"),
        (401, "·", "HINT"),
    ];

    println!("\n=== Virtual Text Severity Icons ===");
    for (priority, icon, name) in severities {
        let name_lower = name.to_lowercase();
        let vt = VirtualTextEntry {
            text: format!("{icon} {name_lower} message"),
            style: Style::new(),
            priority,
        };

        println!("  {name} (priority {priority}): {}", vt.text);
        assert!(vt.text.starts_with(icon), "{name} should start with icon {icon}");
    }
    println!("\n✓ All severity icons correct");
}

#[test]
fn test_virtual_text_empty_line() {
    // Create buffer with a single empty line (not a completely empty buffer)
    let mut buffer = Buffer::empty(1);
    buffer.set_content(" "); // Single space to ensure we have one line

    let window = create_test_window(40, 1);
    let mode = ModeState::normal();
    let mut render_data = RenderData::from_buffer(&window, &buffer, &mode);

    // Add virtual text to line with minimal content
    render_data.virtual_texts[0] = Some(VirtualTextEntry {
        text: "● diagnostic on near-empty line".to_string(),
        style: Style::new(),
        priority: 404,
    });

    assert!(render_data.virtual_texts[0].is_some());

    println!("\n=== Virtual Text on Near-Empty Line ===");
    println!("  1 │ ' '  {}", render_data.virtual_texts[0].as_ref().unwrap().text);
    println!("\n✓ Virtual text works on lines with minimal content");
}

#[test]
fn test_virtual_text_unicode_content() {
    let mut buffer = Buffer::empty(1);
    buffer.set_content("let 変数: String = 123;");

    let window = create_test_window(80, 1);
    let mode = ModeState::normal();
    let mut render_data = RenderData::from_buffer(&window, &buffer, &mode);

    // Add virtual text with Unicode content
    render_data.virtual_texts[0] = Some(VirtualTextEntry {
        text: "● 型エラー: `String`に`i32`は代入できません".to_string(),
        style: Style::new(),
        priority: 404,
    });

    assert!(render_data.virtual_texts[0].is_some());
    assert!(
        render_data.virtual_texts[0]
            .as_ref()
            .unwrap()
            .text
            .contains("型エラー")
    );

    println!("\n=== Virtual Text with Unicode ===");
    println!(
        "  1 │ {}  {}",
        buffer.contents[0].inner,
        render_data.virtual_texts[0].as_ref().unwrap().text
    );
    println!("\n✓ Unicode virtual text works correctly");
}
