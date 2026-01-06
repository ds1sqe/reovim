//! Integration tests for sign column rendering

use reovim_core::{highlight::Style, render::RenderData, sign::Sign};

#[test]
fn test_sign_column_width_configuration() {
    let mut window = common::create_test_window();

    // Default should be enabled with width 2
    assert_eq!(window.sign_column_width, Some(2));

    // Can be disabled
    window.sign_column_width = None;
    assert!(window.sign_column_width.is_none());

    // Can be set to different widths
    window.sign_column_width = Some(1);
    assert_eq!(window.sign_column_width, Some(1));

    window.sign_column_width = Some(3);
    assert_eq!(window.sign_column_width, Some(3));
}

#[test]
fn test_sign_priority_resolution() {
    // Test that higher priority signs replace lower priority signs
    let error_sign = Sign {
        icon: "●".to_string(),
        style: Style::new(),
        priority: 304, // LSP error
    };

    let warning_sign = Sign {
        icon: "◐".to_string(),
        style: Style::new(),
        priority: 303, // LSP warning
    };

    let git_sign = Sign {
        icon: "+".to_string(),
        style: Style::new(),
        priority: 50, // Git addition
    };

    // Simulate priority-based replacement
    let mut current_sign: Option<Sign> = Some(git_sign);

    // Warning should replace git
    if warning_sign.priority > current_sign.as_ref().unwrap().priority {
        current_sign = Some(warning_sign);
    }
    assert_eq!(current_sign.as_ref().unwrap().icon, "◐");
    assert_eq!(current_sign.as_ref().unwrap().priority, 303);

    // Error should replace warning
    if error_sign.priority > current_sign.as_ref().unwrap().priority {
        current_sign = Some(error_sign);
    }
    assert_eq!(current_sign.as_ref().unwrap().icon, "●");
    assert_eq!(current_sign.as_ref().unwrap().priority, 304);
}

#[test]
fn test_render_data_contains_signs() {
    use reovim_core::{
        buffer::{Buffer, Line},
        modd::ModeState,
    };

    let mut buffer = Buffer::empty(0);
    // Add some lines to the buffer
    buffer.contents.push(Line::from("line 1"));
    buffer.contents.push(Line::from("line 2"));
    buffer.contents.push(Line::from("line 3"));

    let mode = ModeState::default();
    let window = common::create_test_window();

    let render_data = RenderData::from_buffer(&window, &buffer, &mode);

    // RenderData should have signs field initialized
    assert!(!render_data.signs.is_empty());
    assert_eq!(render_data.signs.len(), buffer.contents.len());

    // All signs should be None initially
    for sign in &render_data.signs {
        assert!(sign.is_none());
    }
}

#[test]
fn test_multiple_signs_on_different_lines() {
    use reovim_core::{
        buffer::{Buffer, Line},
        modd::ModeState,
    };

    let mut buffer = Buffer::empty(0);
    // Add enough lines to test signs on different lines
    buffer.contents.push(Line::from("line 1"));
    buffer.contents.push(Line::from("line 2"));
    buffer.contents.push(Line::from("line 3"));
    buffer.contents.push(Line::from("line 4"));
    buffer.contents.push(Line::from("line 5"));

    let mode = ModeState::default();
    let window = common::create_test_window();

    let mut render_data = RenderData::from_buffer(&window, &buffer, &mode);

    // Add signs to different lines
    render_data.signs[0] = Some(Sign {
        icon: "●".to_string(),
        style: Style::new(),
        priority: 304,
    });

    render_data.signs[2] = Some(Sign {
        icon: "◐".to_string(),
        style: Style::new(),
        priority: 303,
    });

    render_data.signs[4] = Some(Sign {
        icon: "+".to_string(),
        style: Style::new(),
        priority: 50,
    });

    // Verify signs are on correct lines
    assert!(render_data.signs[0].is_some());
    assert!(render_data.signs[1].is_none());
    assert!(render_data.signs[2].is_some());
    assert!(render_data.signs[3].is_none());
    assert!(render_data.signs[4].is_some());

    // Verify icons
    assert_eq!(render_data.signs[0].as_ref().unwrap().icon, "●");
    assert_eq!(render_data.signs[2].as_ref().unwrap().icon, "◐");
    assert_eq!(render_data.signs[4].as_ref().unwrap().icon, "+");
}

mod common {
    use reovim_core::{
        content::WindowContentSource,
        screen::{
            Position,
            window::{Anchor, Window},
        },
    };

    pub const fn create_test_window() -> Window {
        Window {
            id: 0,
            source: WindowContentSource::FileBuffer {
                buffer_id: 0,
                buffer_anchor: Anchor { x: 0, y: 0 },
            },
            anchor: Anchor { x: 0, y: 0 },
            width: 80,
            height: 24,
            z_order: 100,
            is_active: true,
            is_floating: false,
            line_number: None,
            scrollbar_enabled: false,
            sign_column_width: Some(2),
            cursor: Position { x: 0, y: 0 },
            desired_col: None,
            border_config: None,
        }
    }
}
