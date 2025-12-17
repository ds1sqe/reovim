//! Binary split tree for window layout management

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigateDirection {
    Left,
    Down,
    Up,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl WindowRect {
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self { x, y, width, height }
    }

    #[must_use]
    pub const fn overlaps_vertical(&self, other: &Self) -> bool {
        self.y < other.y + other.height && self.y + self.height > other.y
    }

    #[must_use]
    pub const fn overlaps_horizontal(&self, other: &Self) -> bool {
        self.x < other.x + other.width && self.x + self.width > other.x
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WindowLayout {
    pub window_id: usize,
    pub rect: WindowRect,
}

#[derive(Debug)]
pub enum SplitNode {
    Leaf { window_id: usize },
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<Self>,
        second: Box<Self>,
    },
}

impl SplitNode {
    #[must_use]
    pub const fn leaf(window_id: usize) -> Self {
        Self::Leaf { window_id }
    }

    #[must_use]
    pub fn split(direction: SplitDirection, first: Self, second: Self) -> Self {
        Self::Split {
            direction,
            ratio: 0.5,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    #[must_use]
    pub fn window_ids(&self) -> Vec<usize> {
        match self {
            Self::Leaf { window_id } => vec![*window_id],
            Self::Split { first, second, .. } => {
                let mut ids = first.window_ids();
                ids.extend(second.window_ids());
                ids
            }
        }
    }

    #[must_use]
    pub fn contains_window(&self, target_id: usize) -> bool {
        match self {
            Self::Leaf { window_id } => *window_id == target_id,
            Self::Split { first, second, .. } => {
                first.contains_window(target_id) || second.contains_window(target_id)
            }
        }
    }

    #[must_use]
    pub fn window_count(&self) -> usize {
        match self {
            Self::Leaf { .. } => 1,
            Self::Split { first, second, .. } => first.window_count() + second.window_count(),
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    pub fn calculate_layouts(&self, rect: WindowRect, layouts: &mut Vec<WindowLayout>) {
        match self {
            Self::Leaf { window_id } => {
                layouts.push(WindowLayout {
                    window_id: *window_id,
                    rect,
                });
            }
            Self::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (first_rect, second_rect) = match direction {
                    SplitDirection::Horizontal => {
                        let first_height = (f32::from(rect.height) * ratio) as u16;
                        let second_height = rect.height.saturating_sub(first_height);
                        (
                            WindowRect::new(rect.x, rect.y, rect.width, first_height),
                            WindowRect::new(
                                rect.x,
                                rect.y + first_height,
                                rect.width,
                                second_height,
                            ),
                        )
                    }
                    SplitDirection::Vertical => {
                        let first_width = (f32::from(rect.width) * ratio) as u16;
                        let second_width = rect.width.saturating_sub(first_width);
                        (
                            WindowRect::new(rect.x, rect.y, first_width, rect.height),
                            WindowRect::new(
                                rect.x + first_width,
                                rect.y,
                                second_width,
                                rect.height,
                            ),
                        )
                    }
                };
                first.calculate_layouts(first_rect, layouts);
                second.calculate_layouts(second_rect, layouts);
            }
        }
    }

    pub fn split_window(
        &mut self,
        target_window_id: usize,
        new_window_id: usize,
        direction: SplitDirection,
    ) -> bool {
        match self {
            Self::Leaf { window_id } if *window_id == target_window_id => {
                let original = Self::leaf(target_window_id);
                let new_window = Self::leaf(new_window_id);
                *self = Self::Split {
                    direction,
                    ratio: 0.5,
                    first: Box::new(original),
                    second: Box::new(new_window),
                };
                true
            }
            Self::Leaf { .. } => false,
            Self::Split { first, second, .. } => {
                first.split_window(target_window_id, new_window_id, direction)
                    || second.split_window(target_window_id, new_window_id, direction)
            }
        }
    }

    pub fn remove_window(&mut self, target_window_id: usize) -> bool {
        match self {
            Self::Leaf { .. } => false,
            Self::Split { first, second, .. } => {
                if let Self::Leaf { window_id } = **first
                    && window_id == target_window_id
                {
                    let second_node = std::mem::replace(second.as_mut(), Self::leaf(0));
                    *self = second_node;
                    return true;
                }

                if let Self::Leaf { window_id } = **second
                    && window_id == target_window_id
                {
                    let first_node = std::mem::replace(first.as_mut(), Self::leaf(0));
                    *self = first_node;
                    return true;
                }

                first.remove_window(target_window_id) || second.remove_window(target_window_id)
            }
        }
    }

    pub fn adjust_ratio(&mut self, window_id: usize, delta: f32) -> bool {
        match self {
            Self::Leaf { .. } => false,
            Self::Split {
                first,
                second,
                ratio,
                ..
            } => {
                let in_first = first.contains_window(window_id);
                let in_second = second.contains_window(window_id);

                if in_first || in_second {
                    let adjustment = if in_first { delta } else { -delta };
                    *ratio = (*ratio + adjustment).clamp(0.1, 0.9);
                    true
                } else {
                    first.adjust_ratio(window_id, delta) || second.adjust_ratio(window_id, delta)
                }
            }
        }
    }

    pub fn equalize(&mut self) {
        if let Self::Split {
            ratio,
            first,
            second,
            ..
        } = self
        {
            *ratio = 0.5;
            first.equalize();
            second.equalize();
        }
    }
}

#[must_use]
pub fn find_adjacent_window(
    from_window_id: usize,
    direction: NavigateDirection,
    layouts: &[WindowLayout],
) -> Option<usize> {
    let from_layout = layouts.iter().find(|l| l.window_id == from_window_id)?;
    let from_rect = &from_layout.rect;

    layouts
        .iter()
        .filter(|l| l.window_id != from_window_id)
        .filter(|l| match direction {
            NavigateDirection::Left => {
                l.rect.x + l.rect.width <= from_rect.x && from_rect.overlaps_vertical(&l.rect)
            }
            NavigateDirection::Right => {
                l.rect.x >= from_rect.x + from_rect.width && from_rect.overlaps_vertical(&l.rect)
            }
            NavigateDirection::Up => {
                l.rect.y + l.rect.height <= from_rect.y && from_rect.overlaps_horizontal(&l.rect)
            }
            NavigateDirection::Down => {
                l.rect.y >= from_rect.y + from_rect.height && from_rect.overlaps_horizontal(&l.rect)
            }
        })
        .min_by_key(|l| match direction {
            NavigateDirection::Left => from_rect.x.saturating_sub(l.rect.x + l.rect.width),
            NavigateDirection::Right => l.rect.x.saturating_sub(from_rect.x + from_rect.width),
            NavigateDirection::Up => from_rect.y.saturating_sub(l.rect.y + l.rect.height),
            NavigateDirection::Down => l.rect.y.saturating_sub(from_rect.y + from_rect.height),
        })
        .map(|l| l.window_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leaf_node() {
        let node = SplitNode::leaf(1);
        assert_eq!(node.window_ids(), vec![1]);
        assert!(node.contains_window(1));
        assert!(!node.contains_window(2));
        assert_eq!(node.window_count(), 1);
    }

    #[test]
    fn test_split_node() {
        let node = SplitNode::split(
            SplitDirection::Vertical,
            SplitNode::leaf(1),
            SplitNode::leaf(2),
        );
        assert_eq!(node.window_ids(), vec![1, 2]);
        assert!(node.contains_window(1));
        assert!(node.contains_window(2));
        assert_eq!(node.window_count(), 2);
    }

    #[test]
    fn test_calculate_layouts_single() {
        let node = SplitNode::leaf(1);
        let rect = WindowRect::new(0, 0, 100, 50);
        let mut layouts = Vec::new();
        node.calculate_layouts(rect, &mut layouts);

        assert_eq!(layouts.len(), 1);
        assert_eq!(layouts[0].window_id, 1);
        assert_eq!(layouts[0].rect, rect);
    }

    #[test]
    fn test_calculate_layouts_vertical_split() {
        let node = SplitNode::split(
            SplitDirection::Vertical,
            SplitNode::leaf(1),
            SplitNode::leaf(2),
        );
        let rect = WindowRect::new(0, 0, 100, 50);
        let mut layouts = Vec::new();
        node.calculate_layouts(rect, &mut layouts);

        assert_eq!(layouts.len(), 2);
        assert_eq!(layouts[0].window_id, 1);
        assert_eq!(layouts[0].rect.width, 50);
        assert_eq!(layouts[1].window_id, 2);
        assert_eq!(layouts[1].rect.x, 50);
    }

    #[test]
    fn test_split_window() {
        let mut node = SplitNode::leaf(1);
        assert!(node.split_window(1, 2, SplitDirection::Vertical));
        assert_eq!(node.window_ids(), vec![1, 2]);
    }

    #[test]
    fn test_remove_window() {
        let mut node = SplitNode::split(
            SplitDirection::Vertical,
            SplitNode::leaf(1),
            SplitNode::leaf(2),
        );
        assert!(node.remove_window(1));
        assert_eq!(node.window_ids(), vec![2]);
    }

    #[test]
    fn test_find_adjacent_window() {
        let layouts = vec![
            WindowLayout {
                window_id: 1,
                rect: WindowRect::new(0, 0, 50, 50),
            },
            WindowLayout {
                window_id: 2,
                rect: WindowRect::new(50, 0, 50, 50),
            },
        ];

        assert_eq!(
            find_adjacent_window(1, NavigateDirection::Right, &layouts),
            Some(2)
        );
        assert_eq!(
            find_adjacent_window(2, NavigateDirection::Left, &layouts),
            Some(1)
        );
        assert_eq!(
            find_adjacent_window(1, NavigateDirection::Left, &layouts),
            None
        );
    }
}
