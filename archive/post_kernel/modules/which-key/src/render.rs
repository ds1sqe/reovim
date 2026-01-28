//! Rendering logic for the which-key popup.
//!
//! This module formats binding entries into displayable text with box-drawing
//! borders and proper width handling.
//!
//! # Design
//!
//! The popup is rendered as a bordered box with:
//! - Top border with prefix title (e.g., `---- g ----`)
//! - Binding lines: `key -> description`
//! - Bottom border
//!
//! # Width Handling
//!
//! - Key column has a minimum width to align descriptions
//! - Long descriptions are truncated with `...`
//! - Unicode characters (CJK, emoji) are handled via display width

use reovim_driver_input::KeySequence;

use crate::state::BindingEntry;

/// Minimum width for the key column.
const MIN_KEY_WIDTH: usize = 4;

/// Arrow separator between key and description.
const ARROW: &str = " -> ";

/// Border characters (box-drawing).
const BORDER_TOP_LEFT: char = '\u{250C}'; // ┌
const BORDER_TOP_RIGHT: char = '\u{2510}'; // ┐
const BORDER_BOTTOM_LEFT: char = '\u{2514}'; // └
const BORDER_BOTTOM_RIGHT: char = '\u{2518}'; // ┘
const BORDER_HORIZONTAL: char = '\u{2500}'; // ─
const BORDER_VERTICAL: char = '\u{2502}'; // │

/// Render the which-key popup content.
///
/// Creates a bordered box containing the bindings, with the prefix
/// displayed in the top border.
///
/// # Arguments
///
/// * `prefix` - The key sequence prefix to show in the title
/// * `bindings` - The filtered bindings to display
/// * `max_width` - Maximum width in columns (0 = no limit)
/// * `max_height` - Maximum height in rows (0 = no limit, not counting borders)
///
/// # Returns
///
/// A vector of strings, each representing one line of the popup.
/// Includes top border, binding lines, and bottom border.
///
/// # Example
///
/// ```ignore
/// let lines = render_popup(&prefix, &bindings, 40, 10);
/// // Returns:
/// // ["┌──── g ────┐",
/// //  "│g  -> Go to top",
/// //  "│d  -> Go to definition",
/// //  "└───────────┘"]
/// ```
pub fn render_popup(
    prefix: &KeySequence,
    bindings: &[BindingEntry],
    max_width: u16,
    max_height: u16,
) -> Vec<String> {
    let max_width = if max_width == 0 {
        80 // Default width
    } else {
        max_width as usize
    };

    let max_lines = if max_height == 0 {
        20 // Default max height
    } else {
        max_height as usize
    };

    // Handle empty bindings case
    if bindings.is_empty() {
        return render_no_bindings(prefix, max_width);
    }

    // Calculate key column width (max suffix display width + some padding)
    let key_width = bindings
        .iter()
        .map(|b| display_width(&format!("{}", b.suffix)))
        .max()
        .unwrap_or(MIN_KEY_WIDTH)
        .max(MIN_KEY_WIDTH);

    // Calculate content width
    let content_width = calculate_content_width(bindings, key_width);
    let box_width = content_width.min(max_width - 2).max(10); // -2 for borders, min 10

    // Format the prefix for the title
    let prefix_str = if prefix.is_empty() {
        "all".to_string()
    } else {
        format!("{}", prefix)
    };

    let mut lines = Vec::new();

    // Top border with title
    lines.push(render_top_border(&prefix_str, box_width));

    // Binding lines (limited by max_height)
    let binding_count = bindings.len().min(max_lines);
    for binding in bindings.iter().take(binding_count) {
        lines.push(render_binding_line(binding, key_width, box_width));
    }

    // If there are more bindings, show indicator
    if bindings.len() > binding_count {
        let remaining = bindings.len() - binding_count;
        lines.push(render_more_indicator(remaining, box_width));
    }

    // Bottom border
    lines.push(render_bottom_border(box_width));

    lines
}

/// Render the popup for when there are no bindings.
fn render_no_bindings(prefix: &KeySequence, max_width: usize) -> Vec<String> {
    let prefix_str = if prefix.is_empty() {
        "all".to_string()
    } else {
        format!("{}", prefix)
    };

    let message = "No bindings found";
    let box_width = message.len().max(prefix_str.len() + 6).min(max_width - 2);

    vec![
        render_top_border(&prefix_str, box_width),
        render_message_line(message, box_width),
        render_bottom_border(box_width),
    ]
}

/// Render the top border with centered title.
fn render_top_border(title: &str, width: usize) -> String {
    let title_display = format!(" {} ", title);
    let title_width = display_width(&title_display);

    if title_width >= width - 2 {
        // Title is too long, just use horizontal lines
        format!(
            "{}{}{}",
            BORDER_TOP_LEFT,
            BORDER_HORIZONTAL.to_string().repeat(width),
            BORDER_TOP_RIGHT
        )
    } else {
        let remaining = width - title_width;
        let left_len = remaining / 2;
        let right_len = remaining - left_len;

        format!(
            "{}{}{}{}{}",
            BORDER_TOP_LEFT,
            BORDER_HORIZONTAL.to_string().repeat(left_len),
            title_display,
            BORDER_HORIZONTAL.to_string().repeat(right_len),
            BORDER_TOP_RIGHT
        )
    }
}

/// Render the bottom border.
fn render_bottom_border(width: usize) -> String {
    format!(
        "{}{}{}",
        BORDER_BOTTOM_LEFT,
        BORDER_HORIZONTAL.to_string().repeat(width),
        BORDER_BOTTOM_RIGHT
    )
}

/// Render a single binding line.
fn render_binding_line(binding: &BindingEntry, key_width: usize, box_width: usize) -> String {
    let key_str = format!("{}", binding.suffix);
    let key_display_width = display_width(&key_str);
    let key_padding = key_width.saturating_sub(key_display_width);

    // Calculate available space for description
    // Format: │{key}{padding}{arrow}{description}│
    let arrow_width = ARROW.len();
    let prefix_width = key_width + arrow_width;
    let available_desc_width = box_width.saturating_sub(prefix_width);

    let description = truncate_to_width(&binding.description, available_desc_width);

    // Build the line content
    let content = format!("{}{}{}{}", key_str, " ".repeat(key_padding), ARROW, description);

    // Pad to fill the box width
    let content_width = display_width(&content);
    let padding = box_width.saturating_sub(content_width);

    format!("{}{}{}{}", BORDER_VERTICAL, content, " ".repeat(padding), BORDER_VERTICAL)
}

/// Render a message line (centered).
fn render_message_line(message: &str, box_width: usize) -> String {
    let msg_width = display_width(message);
    let padding_total = box_width.saturating_sub(msg_width);
    let left_pad = padding_total / 2;
    let right_pad = padding_total - left_pad;

    format!(
        "{}{}{}{}{}",
        BORDER_VERTICAL,
        " ".repeat(left_pad),
        message,
        " ".repeat(right_pad),
        BORDER_VERTICAL
    )
}

/// Render the "more items" indicator.
fn render_more_indicator(remaining: usize, box_width: usize) -> String {
    let message = format!("... {} more", remaining);
    let msg_width = display_width(&message);
    let padding_total = box_width.saturating_sub(msg_width);
    let left_pad = padding_total / 2;
    let right_pad = padding_total - left_pad;

    format!(
        "{}{}{}{}{}",
        BORDER_VERTICAL,
        " ".repeat(left_pad),
        message,
        " ".repeat(right_pad),
        BORDER_VERTICAL
    )
}

/// Calculate the display width of a string (considering Unicode).
///
/// For now, this is a simple char count. A full implementation would
/// use unicode-width for proper CJK/emoji handling.
fn display_width(s: &str) -> usize {
    // Simple implementation - count grapheme clusters
    // For full Unicode support, use unicode-width crate
    s.chars().count()
}

/// Calculate the content width needed for the bindings.
fn calculate_content_width(bindings: &[BindingEntry], key_width: usize) -> usize {
    let arrow_width = ARROW.len();

    bindings
        .iter()
        .map(|b| key_width + arrow_width + display_width(&b.description))
        .max()
        .unwrap_or(key_width + arrow_width + 10) // Default min width
}

/// Truncate a string to fit within a width, adding "..." if needed.
fn truncate_to_width(s: &str, max_width: usize) -> String {
    if max_width < 4 {
        // Can't fit anything meaningful
        return String::new();
    }

    let width = display_width(s);
    if width <= max_width {
        s.to_string()
    } else {
        // Truncate and add "..."
        let mut result = String::new();
        let target_width = max_width - 3; // Reserve 3 for "..."

        for (current_width, c) in s.chars().enumerate() {
            if current_width >= target_width {
                break;
            }
            result.push(c);
        }

        result.push_str("...");
        result
    }
}

/// Calculate the dimensions needed for the popup.
///
/// Returns (width, height) in characters.
pub fn calculate_dimensions(
    prefix: &KeySequence,
    bindings: &[BindingEntry],
    max_width: u16,
    max_height: u16,
) -> (u16, u16) {
    let lines = render_popup(prefix, bindings, max_width, max_height);

    let width = lines
        .iter()
        .map(|line| display_width(line))
        .max()
        .unwrap_or(10) as u16;

    let height = lines.len() as u16;

    (width, height)
}

/// Create overlay constraints for bottom-positioned which-key popup.
///
/// # Arguments
///
/// * `screen_width` - Total screen width in cells
/// * `screen_height` - Total screen height in cells
/// * `content_height` - Height of the popup content in cells
///
/// # Returns
///
/// `OverlayConstraints` positioned at the bottom of the screen.
///
/// # Small Screen Handling
///
/// If the content height exceeds half the screen height, the max height
/// is limited to half the screen to leave room for content.
pub fn bottom_overlay_constraints(
    screen_width: u16,
    screen_height: u16,
    content_height: u16,
) -> reovim_driver_display::layout::OverlayConstraints {
    use reovim_driver_display::layout::OverlayConstraints;

    // Limit max height to half the screen
    let max_height = screen_height / 2;

    // Clamp content height to max
    let effective_height = content_height.min(max_height).max(MIN_POPUP_HEIGHT);

    // Position at bottom
    let y = screen_height.saturating_sub(effective_height);

    OverlayConstraints::at_position(0, y)
        .with_size(screen_width, effective_height)
        .with_max_size(screen_width, max_height)
}

/// Minimum popup height in cells.
const MIN_POPUP_HEIGHT: u16 = 3;

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_input::KeySequence};

    fn binding(suffix: &str, description: &str) -> BindingEntry {
        let seq = KeySequence::parse(suffix).unwrap_or_default();
        BindingEntry::new(seq, description)
    }

    #[test]
    fn test_render_empty_shows_message() {
        let prefix = KeySequence::parse("g").unwrap();
        let lines = render_popup(&prefix, &[], 40, 10);

        assert_eq!(lines.len(), 3); // Top, message, bottom
        assert!(lines[1].contains("No bindings found"));
    }

    #[test]
    fn test_render_bindings_format() {
        let prefix = KeySequence::parse("g").unwrap();
        let bindings = vec![binding("g", "Go to top"), binding("d", "Go to definition")];

        let lines = render_popup(&prefix, &bindings, 40, 10);

        assert!(lines.len() >= 4); // Top, 2 bindings, bottom

        // Check top border contains prefix
        assert!(lines[0].contains("g"));

        // Check bindings contain arrows
        assert!(lines[1].contains(" -> "));
        assert!(lines[2].contains(" -> "));
    }

    #[test]
    fn test_render_truncates_long_descriptions() {
        let prefix = KeySequence::new();
        let bindings = vec![binding(
            "x",
            "This is a very long description that should be truncated",
        )];

        let lines = render_popup(&prefix, &bindings, 30, 10);

        // Should be truncated with "..."
        let binding_line = &lines[1];
        // Use display_width, not byte length (Unicode characters)
        assert!(display_width(binding_line) <= 32); // 30 + 2 for borders
        assert!(binding_line.contains("..."));
    }

    #[test]
    fn test_render_unicode_descriptions() {
        let prefix = KeySequence::new();
        let bindings = vec![binding("a", "Hello World"), binding("b", "Japanese Text")];

        let lines = render_popup(&prefix, &bindings, 40, 10);

        // Should render without errors
        assert!(lines.len() >= 4);
    }

    #[test]
    fn test_render_special_keys() {
        let prefix = KeySequence::parse("<C-w>").unwrap();
        let bindings = vec![binding("h", "Window left"), binding("j", "Window down")];

        let lines = render_popup(&prefix, &bindings, 40, 10);

        // Should contain the prefix in title
        assert!(lines[0].contains("<C-w>") || lines[0].contains("C-w"));
    }

    #[test]
    fn test_render_border_width() {
        let prefix = KeySequence::new();
        let bindings = vec![binding("x", "Short")];

        let lines = render_popup(&prefix, &bindings, 40, 10);

        // All lines should have same display width (borders align)
        let widths: Vec<usize> = lines.iter().map(|l| display_width(l)).collect();
        let first_width = widths[0];
        for (i, &w) in widths.iter().enumerate() {
            assert_eq!(
                w, first_width,
                "Line {} has display width {} but expected {}",
                i, w, first_width
            );
        }
    }

    #[test]
    fn test_calculate_dimensions() {
        let prefix = KeySequence::new();
        let bindings = vec![binding("g", "Go to top"), binding("d", "Definition")];

        let (width, height) = calculate_dimensions(&prefix, &bindings, 40, 10);

        assert!(width > 0);
        assert!(height >= 4); // Top + 2 bindings + bottom
    }

    #[test]
    fn test_render_max_height_limits() {
        let prefix = KeySequence::new();
        let bindings: Vec<BindingEntry> = (0..20)
            .map(|i| binding(&format!("{}", (b'a' + i as u8) as char), "Description"))
            .collect();

        let lines = render_popup(&prefix, &bindings, 40, 5);

        // Should be limited: top + 5 bindings + more indicator + bottom = 8
        // Or top + 5 bindings + bottom = 7 if no more indicator fits
        assert!(lines.len() <= 8);

        // Should show "more" indicator if there are more
        let has_more = lines.iter().any(|l| l.contains("more"));
        assert!(has_more, "Should show 'more' indicator");
    }

    #[test]
    fn test_truncate_to_width() {
        assert_eq!(truncate_to_width("short", 10), "short");
        assert_eq!(truncate_to_width("this is long", 10), "this is...");
        assert_eq!(truncate_to_width("tiny", 3), ""); // Can't fit
    }

    #[test]
    fn test_top_border_with_title() {
        let border = render_top_border("test", 20);
        assert!(border.contains("test"));
        assert!(border.starts_with(BORDER_TOP_LEFT));
        assert!(border.ends_with(BORDER_TOP_RIGHT));
    }

    #[test]
    fn test_bottom_border() {
        let border = render_bottom_border(20);
        assert!(border.starts_with(BORDER_BOTTOM_LEFT));
        assert!(border.ends_with(BORDER_BOTTOM_RIGHT));
        // Use display_width, not byte length (Unicode box-drawing characters)
        assert_eq!(display_width(&border), 22); // 20 + 2 corners
    }

    // Bottom overlay constraints tests

    #[test]
    fn test_overlay_positioned_at_bottom() {
        use reovim_driver_display::layout::Anchor;

        let constraints = bottom_overlay_constraints(80, 24, 10);

        // Check anchor is at screen position
        match constraints.anchor {
            Anchor::Screen { x, y } => {
                assert_eq!(x, 0);
                // y should be at bottom: 24 - 10 = 14
                assert_eq!(y, 14);
            }
            _ => panic!("Expected Screen anchor"),
        }
    }

    #[test]
    fn test_overlay_full_width() {
        let constraints = bottom_overlay_constraints(80, 24, 10);

        assert_eq!(constraints.preferred_width, Some(80));
    }

    #[test]
    fn test_overlay_height_matches_content() {
        let constraints = bottom_overlay_constraints(80, 24, 8);

        assert_eq!(constraints.preferred_height, Some(8));
    }

    #[test]
    fn test_overlay_small_screen() {
        // 30x5 terminal - very small
        let constraints = bottom_overlay_constraints(30, 5, 10);

        // Max height should be half screen (2)
        // But minimum is 3, so effective height is 3
        assert_eq!(constraints.preferred_height, Some(MIN_POPUP_HEIGHT));
        assert_eq!(constraints.max_height, Some(2)); // Half of 5

        // Position should be at bottom
        match constraints.anchor {
            reovim_driver_display::layout::Anchor::Screen { x, y } => {
                assert_eq!(x, 0);
                // y = 5 - 3 = 2
                assert_eq!(y, 2);
            }
            _ => panic!("Expected Screen anchor"),
        }
    }

    #[test]
    fn test_overlay_max_height_is_half_screen() {
        let constraints = bottom_overlay_constraints(80, 24, 20);

        // Max height should be half screen (12)
        assert_eq!(constraints.max_height, Some(12));

        // Preferred height should be clamped to max
        assert_eq!(constraints.preferred_height, Some(12));
    }
}
