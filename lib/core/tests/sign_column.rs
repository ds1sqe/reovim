//! Integration tests for sign column rendering

use reovim_core::{
    highlight::Style, render::RenderData, screen::window::SignColumnMode, sign::Sign,
};

#[test]
fn test_sign_column_mode_configuration() {
    let mut window = common::create_test_window();

    // Default should be Yes(2) mode (always show with width 2)
    assert_eq!(window.config.sign_column_mode, SignColumnMode::Yes(2));
    assert_eq!(window.config.sign_column_mode.effective_width(true), 2);
    assert_eq!(window.config.sign_column_mode.effective_width(false), 2);

    // Can be set to No (disabled)
    window.config.sign_column_mode = SignColumnMode::No;
    assert_eq!(window.config.sign_column_mode, SignColumnMode::No);
    assert_eq!(window.config.sign_column_mode.effective_width(true), 0);
    assert_eq!(window.config.sign_column_mode.effective_width(false), 0);

    // Can be set to Yes with different widths
    window.config.sign_column_mode = SignColumnMode::Yes(1);
    assert_eq!(window.config.sign_column_mode.effective_width(true), 1);

    window.config.sign_column_mode = SignColumnMode::Yes(3);
    assert_eq!(window.config.sign_column_mode.effective_width(true), 3);
}

#[test]
fn test_sign_column_auto_mode() {
    let mut window = common::create_test_window();

    // Auto mode: width depends on whether signs exist
    window.config.sign_column_mode = SignColumnMode::Auto;
    assert_eq!(window.config.sign_column_mode.effective_width(false), 0);
    assert_eq!(window.config.sign_column_mode.effective_width(true), 2);
}

#[test]
fn test_sign_column_number_mode() {
    let mut window = common::create_test_window();

    // Number mode: signs displayed in line number column
    window.config.sign_column_mode = SignColumnMode::Number;
    assert_eq!(window.config.sign_column_mode.effective_width(true), 0);
    assert_eq!(window.config.sign_column_mode.effective_width(false), 0);
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
            WindowId, WindowRect,
            window::{SignColumnMode, Viewport, Window, WindowConfig},
        },
    };

    pub fn create_test_window() -> Window {
        Window {
            id: WindowId::new(0),
            source: WindowContentSource::FileBuffer { buffer_id: 0 },
            bounds: WindowRect::new(0, 0, 80, 24),
            z_order: 100,
            is_active: true,
            is_floating: false,
            viewport: Viewport::default(),
            config: WindowConfig {
                line_number: None,
                scrollbar_enabled: false,
                sign_column_mode: SignColumnMode::Yes(2),
                border_config: None,
            },
        }
    }
}
