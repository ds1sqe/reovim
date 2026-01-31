//! Anchor type conversion between wire format and TUI.
//!
//! The wire format uses normalized coordinates and string overlay IDs,
//! while TUI uses absolute coordinates and `WindowId` references.

use std::collections::HashMap;

use {
    reovim_client_model::{Size, wire::Anchor as WireAnchor},
    reovim_driver_display::{
        WindowId,
        layout::{Anchor as TuiAnchor, ColIndex, LineIndex},
    },
};

/// Context required for anchor conversion.
///
/// Converting a wire anchor to a TUI anchor requires context:
/// - The focused window (for `Cursor` anchor)
/// - Current cursor position (for `Cursor` anchor)
/// - Screen size (for `Screen` anchor coordinate conversion)
/// - Mapping from overlay IDs to window IDs (for `Below` anchor)
#[derive(Debug, Clone)]
pub struct AnchorContext {
    /// Currently focused window (for `Cursor` anchor).
    pub focused_window: WindowId,
    /// Cursor position in focused window (line, col as usize).
    pub cursor_position: (usize, usize),
    /// Screen size for normalized coordinate conversion.
    pub screen_size: Size,
    /// Mapping from overlay string IDs to window IDs.
    pub overlay_windows: HashMap<String, WindowId>,
}

impl AnchorContext {
    /// Create a new anchor context.
    #[must_use]
    pub fn new(
        focused_window: WindowId,
        cursor_position: (usize, usize),
        screen_size: Size,
    ) -> Self {
        Self {
            focused_window,
            cursor_position,
            screen_size,
            overlay_windows: HashMap::new(),
        }
    }

    /// Register an overlay window for `Below` anchor resolution.
    pub fn register_overlay(&mut self, overlay_id: impl Into<String>, window_id: WindowId) {
        self.overlay_windows.insert(overlay_id.into(), window_id);
    }

    /// Look up a window ID for an overlay.
    #[must_use]
    pub fn lookup_overlay(&self, overlay_id: &str) -> Option<WindowId> {
        self.overlay_windows.get(overlay_id).copied()
    }
}

/// Convert a wire anchor to a TUI anchor.
///
/// # Arguments
///
/// * `wire` - The wire format anchor to convert
/// * `ctx` - Context for conversion (screen size, cursor position, etc.)
///
/// # Anchor Mapping
///
/// | Wire | TUI | Notes |
/// |------|-----|-------|
/// | `Cursor` | `Cursor{window,line,col}` | Uses focused window and cursor position |
/// | `Buffer{line,col}` | `Cursor{window,line,col}` | Maps to focused window cursor |
/// | `Screen{x,y}` | `Screen{x,y}` | Converts normalized (0.0-1.0) to absolute |
/// | `Center` | `Center` | Direct mapping |
/// | `Below(id)` | `Below(WindowId)` | Looks up overlay, falls back to Center |
///
/// # Edge Cases
///
/// - Screen coordinates outside 0.0-1.0 are clamped
/// - Unknown overlay IDs in `Below` fall back to `Center`
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn convert_anchor(wire: &WireAnchor, ctx: &AnchorContext) -> TuiAnchor {
    match wire {
        WireAnchor::Cursor => TuiAnchor::Cursor {
            window: ctx.focused_window,
            line: LineIndex::new(ctx.cursor_position.0),
            col: ColIndex::new(ctx.cursor_position.1),
        },

        WireAnchor::Buffer { line, col, .. } => {
            // Buffer position maps to cursor in focused window.
            // In a full implementation, we'd look up which window displays
            // this buffer and calculate the actual screen position.
            // For now, we map to the focused window cursor position.
            TuiAnchor::Cursor {
                window: ctx.focused_window,
                line: LineIndex::new(*line as usize),
                col: ColIndex::new(*col as usize),
            }
        }

        WireAnchor::Screen { x, y } => {
            // Convert normalized coordinates (0.0-1.0) to absolute screen coordinates.
            // Clamp values to valid range to handle edge cases.
            let clamped_x = x.clamp(0.0, 1.0);
            let clamped_y = y.clamp(0.0, 1.0);

            TuiAnchor::Screen {
                x: (clamped_x * f32::from(ctx.screen_size.width)) as u16,
                y: (clamped_y * f32::from(ctx.screen_size.height)) as u16,
            }
        }

        WireAnchor::Center => TuiAnchor::Center,

        WireAnchor::Below(overlay_id) => {
            // Look up the overlay window ID, fall back to Center if not found.
            ctx.overlay_windows
                .get(overlay_id)
                .map_or(TuiAnchor::Center, |&window_id| TuiAnchor::Below(window_id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> AnchorContext {
        AnchorContext::new(
            WindowId::from_raw(1),
            (10, 5), // cursor at line 10, col 5
            Size::new(80, 24),
        )
    }

    #[test]
    fn test_convert_cursor_anchor() {
        let ctx = test_context();
        let wire = WireAnchor::Cursor;
        let tui = convert_anchor(&wire, &ctx);

        match tui {
            TuiAnchor::Cursor { window, line, col } => {
                assert_eq!(window, WindowId::from_raw(1));
                assert_eq!(line.as_usize(), 10);
                assert_eq!(col.as_usize(), 5);
            }
            _ => panic!("Expected Cursor anchor"),
        }
    }

    #[test]
    fn test_convert_buffer_anchor() {
        let ctx = test_context();
        let wire = WireAnchor::Buffer {
            buffer_id: 42,
            line: 20,
            col: 15,
        };
        let tui = convert_anchor(&wire, &ctx);

        match tui {
            TuiAnchor::Cursor { window, line, col } => {
                assert_eq!(window, WindowId::from_raw(1));
                assert_eq!(line.as_usize(), 20);
                assert_eq!(col.as_usize(), 15);
            }
            _ => panic!("Expected Cursor anchor"),
        }
    }

    #[test]
    fn test_convert_screen_anchor_center() {
        let ctx = test_context();
        let wire = WireAnchor::Screen { x: 0.5, y: 0.5 };
        let tui = convert_anchor(&wire, &ctx);

        match tui {
            TuiAnchor::Screen { x, y } => {
                assert_eq!(x, 40); // 0.5 * 80
                assert_eq!(y, 12); // 0.5 * 24
            }
            _ => panic!("Expected Screen anchor"),
        }
    }

    #[test]
    fn test_convert_screen_anchor_origin() {
        let ctx = test_context();
        let wire = WireAnchor::Screen { x: 0.0, y: 0.0 };
        let tui = convert_anchor(&wire, &ctx);

        match tui {
            TuiAnchor::Screen { x, y } => {
                assert_eq!(x, 0);
                assert_eq!(y, 0);
            }
            _ => panic!("Expected Screen anchor"),
        }
    }

    #[test]
    fn test_convert_screen_anchor_bottom_right() {
        let ctx = test_context();
        let wire = WireAnchor::Screen { x: 1.0, y: 1.0 };
        let tui = convert_anchor(&wire, &ctx);

        match tui {
            TuiAnchor::Screen { x, y } => {
                assert_eq!(x, 80); // 1.0 * 80
                assert_eq!(y, 24); // 1.0 * 24
            }
            _ => panic!("Expected Screen anchor"),
        }
    }

    #[test]
    fn test_convert_screen_anchor_clamped_negative() {
        let ctx = test_context();
        let wire = WireAnchor::Screen { x: -0.5, y: -0.5 };
        let tui = convert_anchor(&wire, &ctx);

        match tui {
            TuiAnchor::Screen { x, y } => {
                assert_eq!(x, 0); // Clamped to 0.0
                assert_eq!(y, 0); // Clamped to 0.0
            }
            _ => panic!("Expected Screen anchor"),
        }
    }

    #[test]
    fn test_convert_screen_anchor_clamped_overflow() {
        let ctx = test_context();
        let wire = WireAnchor::Screen { x: 1.5, y: 2.0 };
        let tui = convert_anchor(&wire, &ctx);

        match tui {
            TuiAnchor::Screen { x, y } => {
                assert_eq!(x, 80); // Clamped to 1.0
                assert_eq!(y, 24); // Clamped to 1.0
            }
            _ => panic!("Expected Screen anchor"),
        }
    }

    #[test]
    fn test_convert_center_anchor() {
        let ctx = test_context();
        let wire = WireAnchor::Center;
        let tui = convert_anchor(&wire, &ctx);

        assert!(matches!(tui, TuiAnchor::Center));
    }

    #[test]
    fn test_convert_below_anchor_known() {
        let mut ctx = test_context();
        ctx.register_overlay("completion", WindowId::from_raw(42));

        let wire = WireAnchor::Below("completion".to_string());
        let tui = convert_anchor(&wire, &ctx);

        match tui {
            TuiAnchor::Below(window_id) => {
                assert_eq!(window_id, WindowId::from_raw(42));
            }
            _ => panic!("Expected Below anchor"),
        }
    }

    #[test]
    fn test_convert_below_anchor_unknown_falls_back_to_center() {
        let ctx = test_context();
        let wire = WireAnchor::Below("unknown_overlay".to_string());
        let tui = convert_anchor(&wire, &ctx);

        // Unknown overlay ID falls back to Center
        assert!(matches!(tui, TuiAnchor::Center));
    }

    #[test]
    fn test_anchor_context_new() {
        let ctx = AnchorContext::new(WindowId::from_raw(5), (100, 50), Size::new(120, 40));
        assert_eq!(ctx.focused_window, WindowId::from_raw(5));
        assert_eq!(ctx.cursor_position, (100, 50));
        assert_eq!(ctx.screen_size.width, 120);
        assert_eq!(ctx.screen_size.height, 40);
        assert!(ctx.overlay_windows.is_empty());
    }

    #[test]
    fn test_anchor_context_register_overlay() {
        let mut ctx = test_context();
        ctx.register_overlay("hover", WindowId::from_raw(100));
        ctx.register_overlay("completion", WindowId::from_raw(101));

        assert_eq!(ctx.lookup_overlay("hover"), Some(WindowId::from_raw(100)));
        assert_eq!(ctx.lookup_overlay("completion"), Some(WindowId::from_raw(101)));
        assert_eq!(ctx.lookup_overlay("nonexistent"), None);
    }
}
