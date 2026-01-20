//! Vim-style focus navigation policy.
//!
//! This module provides `VimFocusPolicy`, which implements the `FocusPolicy` trait
//! from the display driver. It provides vim-style directional navigation between windows.
//!
//! # Architecture
//!
//! Following the mechanism vs policy principle:
//! - **Mechanism** (display driver): `FocusPolicy` trait, `NavigateDirection`
//! - **Policy** (this module): `VimFocusPolicy` decides HOW focus moves
//!
//! # Navigation Algorithm
//!
//! When moving in a direction (e.g., left):
//! 1. Find windows that are actually to the left of current
//! 2. Among those, prefer windows with edge alignment (same y range)
//! 3. Among aligned windows, choose the closest one
//! 4. If no aligned windows, choose the closest window overall

use reovim_driver_display::{FocusPolicy, NavigateDirection, Rect, WindowId, WindowView};

/// Vim-style focus navigation policy.
///
/// Implements C-w h/j/k/l navigation semantics, preferring windows
/// that are visually aligned with the current window.
#[derive(Debug, Clone, Copy, Default)]
pub struct VimFocusPolicy;

impl VimFocusPolicy {
    /// Create a new vim focus policy.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Cycle through windows in order.
    ///
    /// Implements C-w w (forward) and C-w W (backward) behavior.
    #[must_use]
    pub fn cycle(
        &self,
        forward: bool,
        current: WindowId,
        views: &[WindowView],
    ) -> Option<WindowId> {
        if views.is_empty() {
            return None;
        }

        // Find current index
        let current_idx = views.iter().position(|v| v.window_id == current)?;

        // Calculate next index with wrapping
        let next_idx = if forward {
            (current_idx + 1) % views.len()
        } else {
            (current_idx + views.len() - 1) % views.len()
        };

        Some(views[next_idx].window_id)
    }
}

impl FocusPolicy for VimFocusPolicy {
    fn next(
        &self,
        direction: NavigateDirection,
        current: WindowId,
        views: &[WindowView],
    ) -> Option<WindowId> {
        // Find current window
        let current_view = views.iter().find(|v| v.window_id == current)?;
        let current_bounds = current_view.bounds;

        // Find candidates in the given direction
        let candidates: Vec<_> = views
            .iter()
            .filter(|v| {
                v.window_id != current && is_in_direction(v.bounds, current_bounds, direction)
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // First, try to find windows with edge alignment
        let aligned: Vec<_> = candidates
            .iter()
            .filter(|v| has_overlap(v.bounds, current_bounds, direction))
            .copied()
            .collect();

        // Choose from aligned candidates if any, otherwise from all candidates
        let pool: &[&WindowView] = if aligned.is_empty() {
            &candidates
        } else {
            &aligned
        };

        // Find the closest window
        pool.iter()
            .min_by_key(|v| distance(v.bounds, current_bounds, direction))
            .map(|v| v.window_id)
    }
}

/// Check if `target` is in the given direction from `from`.
fn is_in_direction(target: Rect, from: Rect, direction: NavigateDirection) -> bool {
    let (target_cx, target_cy) = center(target);
    let (from_cx, from_cy) = center(from);

    match direction {
        NavigateDirection::Left => target_cx < from_cx,
        NavigateDirection::Right => target_cx > from_cx,
        NavigateDirection::Up => target_cy < from_cy,
        NavigateDirection::Down => target_cy > from_cy,
    }
}

/// Check if `target` has edge overlap with `from` in the perpendicular axis.
///
/// For horizontal movement (left/right), checks if y ranges overlap.
/// For vertical movement (up/down), checks if x ranges overlap.
fn has_overlap(target: Rect, from: Rect, direction: NavigateDirection) -> bool {
    match direction {
        NavigateDirection::Left | NavigateDirection::Right => {
            // Check y overlap
            let from_top = from.y;
            let from_bottom = from.y.saturating_add(from.height);
            let target_top = target.y;
            let target_bottom = target.y.saturating_add(target.height);

            // Ranges overlap if neither is completely before the other
            from_top < target_bottom && target_top < from_bottom
        }
        NavigateDirection::Up | NavigateDirection::Down => {
            // Check x overlap
            let from_left = from.x;
            let from_right = from.x.saturating_add(from.width);
            let target_left = target.x;
            let target_right = target.x.saturating_add(target.width);

            from_left < target_right && target_left < from_right
        }
    }
}

/// Calculate distance between windows based on direction.
///
/// For left/right, measures horizontal distance between nearest edges.
/// For up/down, measures vertical distance between nearest edges.
fn distance(target: Rect, from: Rect, direction: NavigateDirection) -> i32 {
    match direction {
        NavigateDirection::Left => {
            // Distance from left edge of `from` to right edge of `target`
            let from_left = i32::from(from.x);
            let target_right = i32::from(target.x.saturating_add(target.width));
            (from_left - target_right).abs()
        }
        NavigateDirection::Right => {
            // Distance from right edge of `from` to left edge of `target`
            let from_right = i32::from(from.x.saturating_add(from.width));
            let target_left = i32::from(target.x);
            (target_left - from_right).abs()
        }
        NavigateDirection::Up => {
            // Distance from top edge of `from` to bottom edge of `target`
            let from_top = i32::from(from.y);
            let target_bottom = i32::from(target.y.saturating_add(target.height));
            (from_top - target_bottom).abs()
        }
        NavigateDirection::Down => {
            // Distance from bottom edge of `from` to top edge of `target`
            let from_bottom = i32::from(from.y.saturating_add(from.height));
            let target_top = i32::from(target.y);
            (target_top - from_bottom).abs()
        }
    }
}

/// Get the center point of a rectangle.
const fn center(rect: Rect) -> (u16, u16) {
    (rect.x + rect.width / 2, rect.y + rect.height / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_view(id: usize, x: u16, y: u16, w: u16, h: u16) -> WindowView {
        WindowView::new(WindowId::from_raw(id), Rect::new(x, y, w, h))
    }

    #[test]
    fn test_vim_focus_policy_new() {
        let policy = VimFocusPolicy::new();
        // Should be copyable
        let _copy = policy;
    }

    #[test]
    fn test_focus_left() {
        let policy = VimFocusPolicy::new();
        let views = vec![
            make_view(1, 0, 0, 40, 24),  // Left
            make_view(2, 40, 0, 40, 24), // Right
        ];

        // From right, go left
        let next = policy.next(NavigateDirection::Left, WindowId::from_raw(2), &views);
        assert_eq!(next, Some(WindowId::from_raw(1)));

        // From left, go left (nothing there)
        let next = policy.next(NavigateDirection::Left, WindowId::from_raw(1), &views);
        assert_eq!(next, None);
    }

    #[test]
    fn test_focus_right() {
        let policy = VimFocusPolicy::new();
        let views = vec![
            make_view(1, 0, 0, 40, 24),  // Left
            make_view(2, 40, 0, 40, 24), // Right
        ];

        // From left, go right
        let next = policy.next(NavigateDirection::Right, WindowId::from_raw(1), &views);
        assert_eq!(next, Some(WindowId::from_raw(2)));

        // From right, go right (nothing there)
        let next = policy.next(NavigateDirection::Right, WindowId::from_raw(2), &views);
        assert_eq!(next, None);
    }

    #[test]
    fn test_focus_up() {
        let policy = VimFocusPolicy::new();
        let views = vec![
            make_view(1, 0, 0, 80, 12),  // Top
            make_view(2, 0, 12, 80, 12), // Bottom
        ];

        // From bottom, go up
        let next = policy.next(NavigateDirection::Up, WindowId::from_raw(2), &views);
        assert_eq!(next, Some(WindowId::from_raw(1)));

        // From top, go up (nothing there)
        let next = policy.next(NavigateDirection::Up, WindowId::from_raw(1), &views);
        assert_eq!(next, None);
    }

    #[test]
    fn test_focus_down() {
        let policy = VimFocusPolicy::new();
        let views = vec![
            make_view(1, 0, 0, 80, 12),  // Top
            make_view(2, 0, 12, 80, 12), // Bottom
        ];

        // From top, go down
        let next = policy.next(NavigateDirection::Down, WindowId::from_raw(1), &views);
        assert_eq!(next, Some(WindowId::from_raw(2)));

        // From bottom, go down (nothing there)
        let next = policy.next(NavigateDirection::Down, WindowId::from_raw(2), &views);
        assert_eq!(next, None);
    }

    #[test]
    fn test_focus_prefers_aligned() {
        let policy = VimFocusPolicy::new();
        // Layout:
        // +-------+-------+
        // |   1   |   3   |
        // +-------+-------+
        // |   2   |
        // +-------+
        let views = vec![
            make_view(1, 0, 0, 40, 12),  // Top-left
            make_view(2, 0, 12, 40, 12), // Bottom-left
            make_view(3, 40, 0, 40, 12), // Top-right
        ];

        // From window 1, going right should prefer 3 (aligned) over anything else
        let next = policy.next(NavigateDirection::Right, WindowId::from_raw(1), &views);
        assert_eq!(next, Some(WindowId::from_raw(3)));

        // From window 3, going left should prefer 1 (aligned)
        let next = policy.next(NavigateDirection::Left, WindowId::from_raw(3), &views);
        assert_eq!(next, Some(WindowId::from_raw(1)));
    }

    #[test]
    fn test_focus_closest_when_multiple() {
        let policy = VimFocusPolicy::new();
        // Layout with three windows in a row:
        // +---+---+---+
        // | 1 | 2 | 3 |
        // +---+---+---+
        let views = vec![
            make_view(1, 0, 0, 26, 24),
            make_view(2, 26, 0, 26, 24),
            make_view(3, 52, 0, 28, 24),
        ];

        // From 1, going right should pick 2 (closest)
        let next = policy.next(NavigateDirection::Right, WindowId::from_raw(1), &views);
        assert_eq!(next, Some(WindowId::from_raw(2)));

        // From 3, going left should pick 2 (closest)
        let next = policy.next(NavigateDirection::Left, WindowId::from_raw(3), &views);
        assert_eq!(next, Some(WindowId::from_raw(2)));
    }

    #[test]
    fn test_focus_unknown_current() {
        let policy = VimFocusPolicy::new();
        let views = vec![make_view(1, 0, 0, 80, 24)];

        // Current window not in views
        let next = policy.next(NavigateDirection::Left, WindowId::from_raw(999), &views);
        assert_eq!(next, None);
    }

    #[test]
    fn test_focus_empty_views() {
        let policy = VimFocusPolicy::new();
        let views: Vec<WindowView> = vec![];

        let next = policy.next(NavigateDirection::Left, WindowId::from_raw(1), &views);
        assert_eq!(next, None);
    }

    #[test]
    fn test_cycle_forward() {
        let policy = VimFocusPolicy::new();
        let views = vec![
            make_view(1, 0, 0, 40, 24),
            make_view(2, 40, 0, 40, 24),
            make_view(3, 0, 24, 80, 12),
        ];

        // Cycle forward from 1
        assert_eq!(policy.cycle(true, WindowId::from_raw(1), &views), Some(WindowId::from_raw(2)));
        // Cycle forward from 2
        assert_eq!(policy.cycle(true, WindowId::from_raw(2), &views), Some(WindowId::from_raw(3)));
        // Cycle forward from 3 (wraps to 1)
        assert_eq!(policy.cycle(true, WindowId::from_raw(3), &views), Some(WindowId::from_raw(1)));
    }

    #[test]
    fn test_cycle_backward() {
        let policy = VimFocusPolicy::new();
        let views = vec![
            make_view(1, 0, 0, 40, 24),
            make_view(2, 40, 0, 40, 24),
            make_view(3, 0, 24, 80, 12),
        ];

        // Cycle backward from 3
        assert_eq!(policy.cycle(false, WindowId::from_raw(3), &views), Some(WindowId::from_raw(2)));
        // Cycle backward from 2
        assert_eq!(policy.cycle(false, WindowId::from_raw(2), &views), Some(WindowId::from_raw(1)));
        // Cycle backward from 1 (wraps to 3)
        assert_eq!(policy.cycle(false, WindowId::from_raw(1), &views), Some(WindowId::from_raw(3)));
    }

    #[test]
    fn test_cycle_unknown_current() {
        let policy = VimFocusPolicy::new();
        let views = vec![make_view(1, 0, 0, 80, 24)];

        assert_eq!(policy.cycle(true, WindowId::from_raw(999), &views), None);
    }

    #[test]
    fn test_cycle_empty_views() {
        let policy = VimFocusPolicy::new();
        let views: Vec<WindowView> = vec![];

        assert_eq!(policy.cycle(true, WindowId::from_raw(1), &views), None);
    }

    #[test]
    fn test_is_in_direction() {
        let from = Rect::new(40, 40, 20, 20);

        // Left
        assert!(is_in_direction(Rect::new(0, 40, 20, 20), from, NavigateDirection::Left));
        assert!(!is_in_direction(Rect::new(60, 40, 20, 20), from, NavigateDirection::Left));

        // Right
        assert!(is_in_direction(Rect::new(60, 40, 20, 20), from, NavigateDirection::Right));
        assert!(!is_in_direction(Rect::new(0, 40, 20, 20), from, NavigateDirection::Right));

        // Up
        assert!(is_in_direction(Rect::new(40, 0, 20, 20), from, NavigateDirection::Up));
        assert!(!is_in_direction(Rect::new(40, 60, 20, 20), from, NavigateDirection::Up));

        // Down
        assert!(is_in_direction(Rect::new(40, 60, 20, 20), from, NavigateDirection::Down));
        assert!(!is_in_direction(Rect::new(40, 0, 20, 20), from, NavigateDirection::Down));
    }

    #[test]
    fn test_has_overlap_horizontal() {
        let from = Rect::new(0, 10, 40, 20);

        // Overlapping y range
        assert!(has_overlap(Rect::new(40, 10, 40, 20), from, NavigateDirection::Right));
        assert!(has_overlap(Rect::new(40, 0, 40, 20), from, NavigateDirection::Right));
        assert!(has_overlap(Rect::new(40, 20, 40, 20), from, NavigateDirection::Right));

        // Non-overlapping y range
        assert!(!has_overlap(Rect::new(40, 40, 40, 20), from, NavigateDirection::Right));
    }

    #[test]
    fn test_has_overlap_vertical() {
        let from = Rect::new(10, 0, 20, 40);

        // Overlapping x range
        assert!(has_overlap(Rect::new(10, 40, 20, 40), from, NavigateDirection::Down));
        assert!(has_overlap(Rect::new(0, 40, 20, 40), from, NavigateDirection::Down));
        assert!(has_overlap(Rect::new(20, 40, 20, 40), from, NavigateDirection::Down));

        // Non-overlapping x range
        assert!(!has_overlap(Rect::new(40, 40, 20, 40), from, NavigateDirection::Down));
    }

    #[test]
    fn test_distance() {
        let from = Rect::new(40, 40, 20, 20);

        // Distance to left
        let left = Rect::new(0, 40, 20, 20);
        assert_eq!(distance(left, from, NavigateDirection::Left), 20); // 40 - 20 = 20

        // Distance to right
        let right = Rect::new(80, 40, 20, 20);
        assert_eq!(distance(right, from, NavigateDirection::Right), 20); // 80 - 60 = 20

        // Distance up
        let up = Rect::new(40, 0, 20, 20);
        assert_eq!(distance(up, from, NavigateDirection::Up), 20); // 40 - 20 = 20

        // Distance down
        let down = Rect::new(40, 80, 20, 20);
        assert_eq!(distance(down, from, NavigateDirection::Down), 20); // 80 - 60 = 20
    }
}
