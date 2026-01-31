//! WASM bindings for web client.
//!
//! Uses `tsify-next` for automatic TypeScript type generation.
//! Types are exported directly via derives, functions are exported here.

// WASM-exported functions are called from JS, so these lints don't apply:
// - must_use: JS doesn't have must_use
// - missing_const_for_fn: wasm_bindgen doesn't support const fn
#![allow(
    clippy::must_use_candidate,
    clippy::missing_const_for_fn,
    clippy::needless_pass_by_value
)]

use wasm_bindgen::prelude::*;

use crate::{
    DefaultLayoutInterpreter, Direction, LogicalLayout, Rect, Size, SplitDirection,
    rendered::WindowTree, traits::LayoutInterpreter,
};

/// Initialize the WASM module.
///
/// Sets up panic hook for better error messages in the browser console.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Interpret a logical layout into a window tree.
///
/// This is a stateless function (not a class) for simplicity and tree-shaking.
///
/// # Arguments
///
/// * `logical` - The logical layout from the server
/// * `screen_width` - Screen width in columns
/// * `screen_height` - Screen height in rows
///
/// # Returns
///
/// A `WindowTree` with computed bounds for each window.
#[wasm_bindgen]
pub fn interpret_layout(
    logical: LogicalLayout,
    screen_width: u16,
    screen_height: u16,
) -> WindowTree {
    let screen = Size::new(screen_width, screen_height);
    DefaultLayoutInterpreter::new().interpret(&logical, screen)
}

/// Check if a point is inside a rectangle.
///
/// # Arguments
///
/// * `rect` - The rectangle to check
/// * `x` - X coordinate of the point
/// * `y` - Y coordinate of the point
///
/// # Returns
///
/// `true` if the point is inside the rectangle (exclusive of right/bottom edges).
#[wasm_bindgen]
pub fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
    rect.contains_xy(x, y)
}

/// Get the opposite direction.
///
/// # Arguments
///
/// * `dir` - The direction
///
/// # Returns
///
/// The opposite direction (Up <-> Down, Left <-> Right).
#[wasm_bindgen]
pub fn direction_opposite(dir: Direction) -> Direction {
    dir.opposite()
}

/// Check if a direction is horizontal (Left or Right).
#[wasm_bindgen]
pub fn direction_is_horizontal(dir: Direction) -> bool {
    dir.is_horizontal()
}

/// Check if a direction is vertical (Up or Down).
#[wasm_bindgen]
pub fn direction_is_vertical(dir: Direction) -> bool {
    dir.is_vertical()
}

/// Get the opposite split direction.
#[wasm_bindgen]
pub fn split_opposite(dir: SplitDirection) -> SplitDirection {
    dir.opposite()
}

/// Check if split direction is horizontal.
///
/// Horizontal splits stack windows vertically (one above the other).
#[wasm_bindgen]
pub fn split_is_horizontal(dir: SplitDirection) -> bool {
    matches!(dir, SplitDirection::Horizontal)
}

/// Check if split direction is vertical.
///
/// Vertical splits place windows side by side.
#[wasm_bindgen]
pub fn split_is_vertical(dir: SplitDirection) -> bool {
    matches!(dir, SplitDirection::Vertical)
}

/// Create a single window logical layout.
#[wasm_bindgen]
pub fn layout_single(buffer_id: u64, viewport_id: u64) -> LogicalLayout {
    LogicalLayout::single(buffer_id, viewport_id)
}

/// Create a horizontal split layout (windows stacked vertically).
#[wasm_bindgen]
pub fn layout_hsplit(children: Vec<LogicalLayout>) -> LogicalLayout {
    LogicalLayout::hsplit(children)
}

/// Create a vertical split layout (windows side by side).
#[wasm_bindgen]
pub fn layout_vsplit(children: Vec<LogicalLayout>) -> LogicalLayout {
    LogicalLayout::vsplit(children)
}

/// Get window count from a logical layout.
#[wasm_bindgen]
pub fn layout_window_count(layout: &LogicalLayout) -> usize {
    layout.window_count()
}

/// Check if a logical layout is a single window (leaf).
#[wasm_bindgen]
pub fn layout_is_leaf(layout: &LogicalLayout) -> bool {
    layout.is_leaf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpret_layout_single() {
        let logical = LogicalLayout::single(1, 100);
        let tree = interpret_layout(logical, 80, 24);
        assert!(tree.is_leaf());
        assert_eq!(tree.bounds(), Rect::new(0, 0, 80, 24));
    }

    #[test]
    fn test_interpret_layout_vsplit() {
        let logical = LogicalLayout::vsplit(vec![
            LogicalLayout::single(1, 100),
            LogicalLayout::single(2, 101),
        ]);
        let tree = interpret_layout(logical, 80, 24);
        assert_eq!(tree.window_count(), 2);
    }

    #[test]
    fn test_rect_contains() {
        let rect = Rect::new(10, 10, 20, 20);
        assert!(rect_contains(rect, 15, 15));
        assert!(!rect_contains(rect, 5, 5));
        assert!(!rect_contains(rect, 30, 30));
    }

    #[test]
    fn test_direction_opposite() {
        assert_eq!(direction_opposite(Direction::Up), Direction::Down);
        assert_eq!(direction_opposite(Direction::Left), Direction::Right);
    }

    #[test]
    fn test_direction_checks() {
        assert!(direction_is_horizontal(Direction::Left));
        assert!(direction_is_horizontal(Direction::Right));
        assert!(direction_is_vertical(Direction::Up));
        assert!(direction_is_vertical(Direction::Down));
    }

    #[test]
    fn test_split_direction() {
        assert!(split_is_horizontal(SplitDirection::Horizontal));
        assert!(split_is_vertical(SplitDirection::Vertical));
        assert_eq!(split_opposite(SplitDirection::Horizontal), SplitDirection::Vertical);
    }

    #[test]
    fn test_layout_helpers() {
        let single = layout_single(1, 100);
        assert!(layout_is_leaf(&single));
        assert_eq!(layout_window_count(&single), 1);

        let split = layout_hsplit(vec![single.clone(), single]);
        assert!(!layout_is_leaf(&split));
        assert_eq!(layout_window_count(&split), 2);
    }
}
