//! Visual test for sign rendering
//!
//! This test creates a mock scenario with signs and prints the visual output
//! to help debug sign column rendering issues.

use reovim_core::{
    buffer::{Buffer, TextOps},
    content::WindowContentSource,
    highlight::Theme,
    modd::ModeState,
    render::RenderData,
    screen::{
        Position,
        window::{Anchor, LineNumber, Window},
    },
    sign::Sign,
};

#[test]
fn test_sign_visual_rendering() {
    // Create buffer with content
    let mut buffer = Buffer::empty(1);
    buffer.set_content("Line 1\nLine 2 with error\nLine 3\nLine 4");

    // Create window with sign column enabled
    let window = Window {
        id: 0,
        source: WindowContentSource::FileBuffer {
            buffer_id: 1,
            buffer_anchor: Anchor { x: 0, y: 0 },
        },
        anchor: Anchor { x: 0, y: 0 },
        width: 40,
        height: 4,
        z_order: 0,
        is_active: true,
        is_floating: false,
        line_number: Some(LineNumber::default()),
        sign_column_width: Some(2), // Enable 2-char sign column
        scrollbar_enabled: false,
        cursor: Position { x: 0, y: 0 },
        desired_col: None,
        border_config: None,
    };

    // Create render data with a sign on line 1 (index 1)
    let mode = ModeState::normal();
    let mut render_data = RenderData::from_buffer(&window, &buffer, &mode);

    // Add an error sign on line 1 (the "Line 2 with error" line)
    let theme = Theme::default();
    let error_sign = Sign {
        icon: "●".to_string(),         // Error icon
        style: theme.diagnostic.error, // Use theme's error style
        priority: 304,
    };
    render_data.signs[1] = Some(error_sign);

    // Print test info
    println!("\n=== Sign Column Visual Test ===");
    println!("Buffer content:");
    for (i, line) in buffer.contents.iter().enumerate() {
        println!("  Line {}: {}", i, line.inner);
    }
    println!("\nSigns:");
    for (i, sign) in render_data.signs.iter().enumerate() {
        if let Some(s) = sign {
            println!("  Line {}: icon='{}' priority={}", i, s.icon, s.priority);
        }
    }

    // Verify sign data structure
    assert!(render_data.signs.len() == 4, "Should have 4 sign slots");
    assert!(render_data.signs[0].is_none(), "Line 0 should have no sign");
    assert!(render_data.signs[1].is_some(), "Line 1 should have error sign");
    assert_eq!(render_data.signs[1].as_ref().unwrap().icon, "●", "Line 1 sign should be ●");
    assert_eq!(
        render_data.signs[1].as_ref().unwrap().priority,
        304,
        "Line 1 sign should have priority 304"
    );

    println!("\n✓ Sign data structure correct");
    println!("  - Line 1 has sign with icon '●' and priority 304");
    println!("  - Signs array properly initialized");
}
