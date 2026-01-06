//! Comprehensive tests for the virtual text system
//!
//! Tests cover:
//! - `VirtualTextEntry` creation and properties
//! - `VirtualTextStyles` theme integration
//! - Priority-based resolution
//! - `RenderData` integration

use reovim_core::{
    highlight::{Attributes, Style, Theme},
    render::{RenderData, VirtualTextEntry},
};

// === VirtualTextEntry Tests ===

#[test]
fn test_virtual_text_entry_creation() {
    let vt = VirtualTextEntry {
        text: "● error message".to_string(),
        style: Style::new(),
        priority: 404,
    };

    assert_eq!(vt.text, "● error message");
    assert_eq!(vt.priority, 404);
}

#[test]
fn test_virtual_text_entry_clone() {
    let vt1 = VirtualTextEntry {
        text: "test message".to_string(),
        style: Style::new().italic(),
        priority: 100,
    };

    let vt2 = vt1.clone();

    assert_eq!(vt1.text, vt2.text);
    assert_eq!(vt1.priority, vt2.priority);
    assert_eq!(vt1.style, vt2.style);
}

#[test]
fn test_virtual_text_entry_equality() {
    let vt1 = VirtualTextEntry {
        text: "same".to_string(),
        style: Style::new(),
        priority: 100,
    };

    let vt2 = VirtualTextEntry {
        text: "same".to_string(),
        style: Style::new(),
        priority: 100,
    };

    let vt3 = VirtualTextEntry {
        text: "different".to_string(),
        style: Style::new(),
        priority: 100,
    };

    assert_eq!(vt1, vt2);
    assert_ne!(vt1, vt3);
}

#[test]
fn test_virtual_text_entry_priority_values() {
    // Test standard priority values used by LSP
    let error = VirtualTextEntry {
        text: "error".to_string(),
        style: Style::new(),
        priority: 404,
    };
    let warning = VirtualTextEntry {
        text: "warning".to_string(),
        style: Style::new(),
        priority: 403,
    };
    let info = VirtualTextEntry {
        text: "info".to_string(),
        style: Style::new(),
        priority: 402,
    };
    let hint = VirtualTextEntry {
        text: "hint".to_string(),
        style: Style::new(),
        priority: 401,
    };

    assert!(error.priority > warning.priority);
    assert!(warning.priority > info.priority);
    assert!(info.priority > hint.priority);
}

#[test]
fn test_virtual_text_with_unicode() {
    let vt = VirtualTextEntry {
        text: "● 型エラー: 期待される `i32`".to_string(),
        style: Style::new(),
        priority: 404,
    };

    assert!(vt.text.contains("●"));
    assert!(vt.text.chars().count() > 0);
}

#[test]
fn test_virtual_text_empty_message() {
    let vt = VirtualTextEntry {
        text: String::new(),
        style: Style::new(),
        priority: 100,
    };

    assert!(vt.text.is_empty());
}

// === VirtualTextStyles Tests ===

#[test]
fn test_virtual_text_styles_dark_theme() {
    let theme = Theme::dark();

    // Verify all severity styles exist and are distinct
    assert_ne!(theme.virtual_text.error, theme.virtual_text.warn);
    assert_ne!(theme.virtual_text.warn, theme.virtual_text.info);
    assert_ne!(theme.virtual_text.info, theme.virtual_text.hint);

    // All virtual text styles should be italic
    assert!(
        theme
            .virtual_text
            .error
            .attributes
            .contains(Attributes::ITALIC)
    );
    assert!(
        theme
            .virtual_text
            .warn
            .attributes
            .contains(Attributes::ITALIC)
    );
    assert!(
        theme
            .virtual_text
            .info
            .attributes
            .contains(Attributes::ITALIC)
    );
    assert!(
        theme
            .virtual_text
            .hint
            .attributes
            .contains(Attributes::ITALIC)
    );
}

#[test]
fn test_virtual_text_styles_light_theme() {
    let theme = Theme::light();

    // Verify styles exist and are italic
    assert!(
        theme
            .virtual_text
            .error
            .attributes
            .contains(Attributes::ITALIC)
    );
    assert!(
        theme
            .virtual_text
            .warn
            .attributes
            .contains(Attributes::ITALIC)
    );
    assert!(
        theme
            .virtual_text
            .info
            .attributes
            .contains(Attributes::ITALIC)
    );
    assert!(
        theme
            .virtual_text
            .hint
            .attributes
            .contains(Attributes::ITALIC)
    );
}

#[test]
fn test_virtual_text_styles_tokyo_night() {
    let theme = Theme::tokyo_night_orange();

    // Verify styles have foreground colors
    assert!(theme.virtual_text.error.fg.is_some());
    assert!(theme.virtual_text.warn.fg.is_some());
    assert!(theme.virtual_text.info.fg.is_some());
    assert!(theme.virtual_text.hint.fg.is_some());
}

// === Priority Resolution Tests ===

#[test]
fn test_priority_resolution_higher_wins() {
    let low_priority = VirtualTextEntry {
        text: "hint".to_string(),
        style: Style::new(),
        priority: 401,
    };

    let high_priority = VirtualTextEntry {
        text: "error".to_string(),
        style: Style::new(),
        priority: 404,
    };

    // Simulate priority check logic
    let should_replace = low_priority.priority < high_priority.priority;
    assert!(should_replace);
}

#[test]
fn test_priority_resolution_equal_keeps_existing() {
    let existing = VirtualTextEntry {
        text: "first".to_string(),
        style: Style::new(),
        priority: 403,
    };

    let new_entry = VirtualTextEntry {
        text: "second".to_string(),
        style: Style::new(),
        priority: 403,
    };

    // Equal priority should NOT replace (keep existing)
    let should_replace = existing.priority < new_entry.priority;
    assert!(!should_replace);
}

#[test]
fn test_priority_resolution_lower_does_not_replace() {
    let existing = VirtualTextEntry {
        text: "error".to_string(),
        style: Style::new(),
        priority: 404,
    };

    let new_entry = VirtualTextEntry {
        text: "hint".to_string(),
        style: Style::new(),
        priority: 401,
    };

    let should_replace = existing.priority < new_entry.priority;
    assert!(!should_replace);
}

// === RenderData Integration Tests ===

#[test]
fn test_render_data_virtual_texts_initialization() {
    let render_data = create_test_render_data(5);

    assert_eq!(render_data.virtual_texts.len(), 5);
    assert!(render_data.virtual_texts.iter().all(Option::is_none));
}

#[test]
fn test_render_data_virtual_texts_set_single() {
    let mut render_data = create_test_render_data(3);

    render_data.virtual_texts[1] = Some(VirtualTextEntry {
        text: "diagnostic".to_string(),
        style: Style::new(),
        priority: 400,
    });

    assert!(render_data.virtual_texts[0].is_none());
    assert!(render_data.virtual_texts[1].is_some());
    assert!(render_data.virtual_texts[2].is_none());
}

#[test]
fn test_render_data_virtual_texts_multiple_lines() {
    let mut render_data = create_test_render_data(5);

    // Set virtual text on multiple lines
    render_data.virtual_texts[0] = Some(VirtualTextEntry {
        text: "● error on line 1".to_string(),
        style: Style::new(),
        priority: 404,
    });
    render_data.virtual_texts[2] = Some(VirtualTextEntry {
        text: "◐ warning on line 3".to_string(),
        style: Style::new(),
        priority: 403,
    });
    render_data.virtual_texts[4] = Some(VirtualTextEntry {
        text: "· hint on line 5".to_string(),
        style: Style::new(),
        priority: 401,
    });

    assert!(render_data.virtual_texts[0].is_some());
    assert!(render_data.virtual_texts[1].is_none());
    assert!(render_data.virtual_texts[2].is_some());
    assert!(render_data.virtual_texts[3].is_none());
    assert!(render_data.virtual_texts[4].is_some());
}

#[test]
fn test_render_data_virtual_texts_replace_with_higher_priority() {
    let mut render_data = create_test_render_data(1);

    // Set initial low priority
    render_data.virtual_texts[0] = Some(VirtualTextEntry {
        text: "hint".to_string(),
        style: Style::new(),
        priority: 401,
    });

    // Replace with higher priority
    let new_priority = 404;
    let should_replace = render_data.virtual_texts[0]
        .as_ref()
        .is_none_or(|existing| existing.priority < new_priority);

    if should_replace {
        render_data.virtual_texts[0] = Some(VirtualTextEntry {
            text: "error".to_string(),
            style: Style::new(),
            priority: new_priority,
        });
    }

    assert_eq!(render_data.virtual_texts[0].as_ref().unwrap().text, "error");
    assert_eq!(render_data.virtual_texts[0].as_ref().unwrap().priority, 404);
}

#[test]
fn test_render_data_virtual_texts_do_not_replace_with_lower_priority() {
    let mut render_data = create_test_render_data(1);

    // Set initial high priority
    render_data.virtual_texts[0] = Some(VirtualTextEntry {
        text: "error".to_string(),
        style: Style::new(),
        priority: 404,
    });

    // Try to replace with lower priority
    let new_priority = 401;
    let should_replace = render_data.virtual_texts[0]
        .as_ref()
        .is_none_or(|existing| existing.priority < new_priority);

    if should_replace {
        render_data.virtual_texts[0] = Some(VirtualTextEntry {
            text: "hint".to_string(),
            style: Style::new(),
            priority: new_priority,
        });
    }

    // Should still be the error
    assert_eq!(render_data.virtual_texts[0].as_ref().unwrap().text, "error");
    assert_eq!(render_data.virtual_texts[0].as_ref().unwrap().priority, 404);
}

// === Truncation Logic Tests ===

#[test]
fn test_truncation_short_message_no_ellipsis() {
    let vt = VirtualTextEntry {
        text: "short".to_string(),
        style: Style::new(),
        priority: 400,
    };

    let max_chars = 20;
    let char_count = vt.text.chars().count();
    let needs_ellipsis = char_count > max_chars;

    assert!(!needs_ellipsis);
}

#[test]
fn test_truncation_long_message_needs_ellipsis() {
    let vt = VirtualTextEntry {
        text: "This is a very long diagnostic message that exceeds the available space".to_string(),
        style: Style::new(),
        priority: 400,
    };

    let max_chars = 20;
    let char_count = vt.text.chars().count();
    let needs_ellipsis = char_count > max_chars;

    assert!(needs_ellipsis);
}

#[test]
fn test_truncation_exact_length_no_ellipsis() {
    let vt = VirtualTextEntry {
        text: "exactly20characters!".to_string(),
        style: Style::new(),
        priority: 400,
    };

    let max_chars = 20;
    let vt_chars: Vec<char> = vt.text.chars().collect();
    let needs_ellipsis = vt_chars.len() > max_chars;

    assert!(!needs_ellipsis);
    assert_eq!(vt_chars.len(), 20);
}

#[test]
fn test_truncation_preserves_content_before_ellipsis() {
    let original = "This is a long message";
    let vt = VirtualTextEntry {
        text: original.to_string(),
        style: Style::new(),
        priority: 400,
    };

    let max_chars = 15;
    let vt_chars: Vec<char> = vt.text.chars().collect();

    let (text_to_render, needs_ellipsis) = if vt_chars.len() > max_chars {
        (&vt_chars[..max_chars.saturating_sub(3)], true)
    } else {
        (&vt_chars[..], false)
    };

    assert!(needs_ellipsis);
    // Should have 12 chars (15 - 3 for "...")
    assert_eq!(text_to_render.len(), 12);
    // First 12 chars should match original
    let truncated: String = text_to_render.iter().collect();
    assert_eq!(truncated, "This is a lo");
}

// === Icon Tests ===

#[test]
fn test_diagnostic_icons() {
    let error_icon = "●";
    let warning_icon = "◐";
    let info_icon = "ⓘ";
    let hint_icon = "·";

    // Verify icons are single grapheme clusters
    assert_eq!(error_icon.chars().count(), 1);
    assert_eq!(warning_icon.chars().count(), 1);
    assert_eq!(info_icon.chars().count(), 1);
    assert_eq!(hint_icon.chars().count(), 1);
}

#[test]
fn test_virtual_text_with_icon_prefix() {
    let message = "mismatched types";
    let icon = "●";
    let formatted = format!("{icon} {message}");

    let vt = VirtualTextEntry {
        text: formatted,
        style: Style::new(),
        priority: 404,
    };

    assert!(vt.text.starts_with("●"));
    assert!(vt.text.contains("mismatched types"));
    assert_eq!(vt.text, "● mismatched types");
}

// === Helper Functions ===

fn create_test_render_data(line_count: usize) -> RenderData {
    use reovim_core::render::{Bounds, LineVisibility};

    RenderData {
        lines: (0..line_count).map(|i| format!("line {i}")).collect(),
        visibility: vec![LineVisibility::Visible; line_count],
        highlights: vec![Vec::new(); line_count],
        decorations: vec![Vec::new(); line_count],
        signs: vec![None; line_count],
        virtual_texts: vec![None; line_count],
        buffer_id: 1,
        window_id: 1,
        window_bounds: Bounds {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        },
        cursor: (0, 0),
    }
}
