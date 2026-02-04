//! Layout interpreter trait.
//!
//! The layout interpreter converts logical layouts from the server
//! into platform-specific rendered structures.

use crate::{
    Rect, Size, SplitDirection,
    rendered::{Window, WindowTree},
    wire::LogicalLayout,
};

/// Trait for interpreting logical layouts into rendered structures.
///
/// Each platform implements this trait to convert the server's
/// logical layout into its specific window/view representation.
///
/// # Type Parameter
///
/// `Output` is the platform-specific structure. For example:
/// - TUI: `WindowTree` with terminal coordinates
/// - Web: DOM element structure
/// - Android: Compose layout description
pub trait LayoutInterpreter {
    /// The platform-specific output type.
    type Output;

    /// Interpret a logical layout into platform-specific structure.
    ///
    /// `screen` is the available screen/container size.
    fn interpret(&self, logical: &LogicalLayout, screen: Size) -> Self::Output;
}

/// Default interpreter that creates a `WindowTree` from `LogicalLayout`.
///
/// This is used by both the web client (via WASM) and as a reference
/// implementation for other platforms.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultLayoutInterpreter;

impl LayoutInterpreter for DefaultLayoutInterpreter {
    type Output = WindowTree;

    fn interpret(&self, logical: &LogicalLayout, screen: Size) -> Self::Output {
        self.interpret_with_bounds(logical, Rect::new(0, 0, screen.width, screen.height))
    }
}

impl DefaultLayoutInterpreter {
    /// Create a new default interpreter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn interpret_with_bounds(self, logical: &LogicalLayout, bounds: Rect) -> WindowTree {
        match logical {
            LogicalLayout::Single {
                buffer_id,
                viewport_id,
            } => WindowTree::leaf(Window::new(*viewport_id, *viewport_id, *buffer_id, bounds)),
            LogicalLayout::Split {
                direction,
                children,
                ratios,
            } => {
                let child_bounds = self.split_bounds(bounds, *direction, ratios);
                let child_trees: Vec<_> = children
                    .iter()
                    .zip(child_bounds)
                    .map(|(child, bounds)| self.interpret_with_bounds(child, bounds))
                    .collect();
                WindowTree::split(*direction, child_trees, bounds)
            }
            LogicalLayout::Tabs { tabs, active, .. } => {
                let tab_trees: Vec<_> = tabs
                    .iter()
                    .map(|tab| self.interpret_with_bounds(tab, bounds))
                    .collect();
                WindowTree::tabs(tab_trees, *active, bounds)
            }
        }
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_lossless,
        clippy::unused_self
    )]
    fn split_bounds(self, bounds: Rect, direction: SplitDirection, ratios: &[f32]) -> Vec<Rect> {
        let mut result = Vec::with_capacity(ratios.len());
        let mut offset = 0u16;

        for (i, ratio) in ratios.iter().enumerate() {
            let (x, y, width, height) = match direction {
                SplitDirection::Horizontal => {
                    let h = (bounds.height as f32 * ratio) as u16;
                    let actual_h = if i == ratios.len() - 1 {
                        bounds.height - offset
                    } else {
                        h
                    };
                    let rect = (bounds.x, bounds.y + offset, bounds.width, actual_h);
                    offset += actual_h;
                    rect
                }
                SplitDirection::Vertical => {
                    let w = (bounds.width as f32 * ratio) as u16;
                    let actual_w = if i == ratios.len() - 1 {
                        bounds.width - offset
                    } else {
                        w
                    };
                    let rect = (bounds.x + offset, bounds.y, actual_w, bounds.height);
                    offset += actual_w;
                    rect
                }
            };
            result.push(Rect::new(x, y, width, height));
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpreter_single() {
        let interpreter = DefaultLayoutInterpreter::new();
        let logical = LogicalLayout::single(1, 100);
        let screen = Size::new(80, 24);

        let tree = interpreter.interpret(&logical, screen);
        assert!(tree.is_leaf());
        assert_eq!(tree.bounds(), Rect::new(0, 0, 80, 24));
    }

    #[test]
    fn test_interpreter_vsplit() {
        let interpreter = DefaultLayoutInterpreter::new();
        let logical = LogicalLayout::vsplit(vec![
            LogicalLayout::single(1, 100),
            LogicalLayout::single(2, 101),
        ]);
        let screen = Size::new(80, 24);

        let tree = interpreter.interpret(&logical, screen);
        assert_eq!(tree.window_count(), 2);

        let windows = tree.all_windows();
        assert_eq!(windows[0].bounds.width, 40);
        assert_eq!(windows[1].bounds.width, 40);
        assert_eq!(windows[0].bounds.x, 0);
        assert_eq!(windows[1].bounds.x, 40);
    }

    #[test]
    fn test_interpreter_hsplit() {
        let interpreter = DefaultLayoutInterpreter::new();
        let logical = LogicalLayout::hsplit(vec![
            LogicalLayout::single(1, 100),
            LogicalLayout::single(2, 101),
        ]);
        let screen = Size::new(80, 24);

        let tree = interpreter.interpret(&logical, screen);
        assert_eq!(tree.window_count(), 2);

        let windows = tree.all_windows();
        assert_eq!(windows[0].bounds.height, 12);
        assert_eq!(windows[1].bounds.height, 12);
        assert_eq!(windows[0].bounds.y, 0);
        assert_eq!(windows[1].bounds.y, 12);
    }

    #[test]
    fn test_interpreter_nested() {
        let interpreter = DefaultLayoutInterpreter::new();
        let logical = LogicalLayout::vsplit(vec![
            LogicalLayout::hsplit(vec![
                LogicalLayout::single(1, 100),
                LogicalLayout::single(2, 101),
            ]),
            LogicalLayout::single(3, 102),
        ]);
        let screen = Size::new(80, 24);

        let tree = interpreter.interpret(&logical, screen);
        assert_eq!(tree.window_count(), 3);
    }

    #[test]
    fn test_interpreter_tabs() {
        let interpreter = DefaultLayoutInterpreter::new();
        let logical = LogicalLayout::tabs(
            vec![LogicalLayout::single(1, 100), LogicalLayout::single(2, 101)],
            0,
        );
        let screen = Size::new(80, 24);

        let tree = interpreter.interpret(&logical, screen);
        assert_eq!(tree.window_count(), 2);
        // All tabs share the same bounds
        let windows = tree.all_windows();
        assert_eq!(windows[0].bounds, windows[1].bounds);
    }
}
