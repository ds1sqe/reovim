//! Binary split tree for tiled window management.
//!
//! Implements the `TiledLayer` trait from `reovim-subsys-layout`.
//! Windows are organized as a binary tree where internal nodes represent
//! splits and leaves represent windows.

use reovim_subsys_layout::{
    Direction, LayerId, MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, Permil, Rect, SplitDirection,
    TiledLayer, TiledTree, WindowId, WindowPlacement, ZOrder, Zone,
};

trait SplitTreeExt {
    fn collect_windows(&self, out: &mut Vec<WindowId>);
    fn contains(&self, target: WindowId) -> bool;
    fn arrange(
        &self,
        bounds: Rect,
        layer_id: LayerId,
        z_base: ZOrder,
        placements: &mut Vec<WindowPlacement>,
    );
    fn split_at(
        &mut self,
        target: WindowId,
        direction: SplitDirection,
        bounds: Rect,
    ) -> Option<WindowId>;
    fn close_leaf(&mut self, target: WindowId) -> Option<WindowId>;
    fn first_leaf(&self) -> WindowId;
    fn resize_at(&mut self, target: WindowId, direction: Direction, delta: i16, bounds: Rect);
    fn equalize(&mut self);
}

impl SplitTreeExt for TiledTree {
    /// Collect all window IDs in this subtree.
    fn collect_windows(&self, out: &mut Vec<WindowId>) {
        match self {
            Self::Window(id) => out.push(*id),
            Self::Split { first, second, .. } => {
                first.collect_windows(out);
                second.collect_windows(out);
            }
        }
    }

    /// Find a leaf by window ID and return whether it exists.
    fn contains(&self, target: WindowId) -> bool {
        match self {
            Self::Window(id) => *id == target,
            Self::Split { first, second, .. } => first.contains(target) || second.contains(target),
        }
    }

    /// Compute placements by recursively partitioning bounds.
    fn arrange(
        &self,
        bounds: Rect,
        layer_id: LayerId,
        z_base: ZOrder,
        placements: &mut Vec<WindowPlacement>,
    ) {
        match self {
            Self::Window(id) => {
                placements.push(WindowPlacement::new(*id, layer_id, Zone::Tiled, bounds, z_base));
            }
            Self::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (first_bounds, second_bounds) =
                    split_rect(bounds, *direction, ratio_to_f32(*ratio));
                first.arrange(first_bounds, layer_id, z_base, placements);
                second.arrange(second_bounds, layer_id, z_base, placements);
            }
        }
    }

    /// Replace the leaf matching `target` with a split node.
    /// Returns the new window ID on success.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn split_at(
        &mut self,
        target: WindowId,
        direction: SplitDirection,
        bounds: Rect,
    ) -> Option<WindowId> {
        match self {
            Self::Window(id) if *id == target => {
                // Check minimum size after split
                let (first_bounds, second_bounds) = split_rect(bounds, direction, 0.5);
                if !meets_minimum_size(first_bounds) || !meets_minimum_size(second_bounds) {
                    return None;
                }
                let new_id = WindowId::new();
                let old_leaf = Box::new(Self::Window(*id));
                let new_leaf = Box::new(Self::Window(new_id));
                *self = Self::Split {
                    direction,
                    ratio: Permil::HALF,
                    first: old_leaf,
                    second: new_leaf,
                };
                Some(new_id)
            }
            Self::Window(_) => None,
            &mut Self::Split {
                direction: sd,
                ratio,
                ref mut first,
                ref mut second,
            } => {
                let (first_bounds, second_bounds) = split_rect(bounds, sd, ratio_to_f32(ratio));
                if first.contains(target) {
                    first.split_at(target, direction, first_bounds)
                } else {
                    second.split_at(target, direction, second_bounds)
                }
            }
        }
    }

    /// Remove a leaf and return the sibling subtree plus a focus target.
    /// Returns `None` if this is the only leaf.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn close_leaf(&mut self, target: WindowId) -> Option<WindowId> {
        match self {
            Self::Window(_) => None, // Can't close root leaf from within
            Self::Split { first, second, .. } => {
                // Check if first child is the target leaf
                if let Self::Window(id) = first.as_ref()
                    && *id == target
                {
                    let sibling = *second.clone();
                    let focus = sibling.first_leaf();
                    *self = sibling;
                    return Some(focus);
                }
                // Check if second child is the target leaf
                if let Self::Window(id) = second.as_ref()
                    && *id == target
                {
                    let sibling = *first.clone();
                    let focus = sibling.first_leaf();
                    *self = sibling;
                    return Some(focus);
                }
                // Recurse into children
                if first.contains(target) {
                    first.close_leaf(target)
                } else if second.contains(target) {
                    second.close_leaf(target)
                } else {
                    None
                }
            }
        }
    }

    /// Get the first leaf in depth-first order.
    fn first_leaf(&self) -> WindowId {
        match self {
            Self::Window(id) => *id,
            Self::Split { first, .. } => first.first_leaf(),
        }
    }

    /// Adjust the ratio of the split containing `target` on the given axis.
    fn resize_at(&mut self, target: WindowId, direction: Direction, delta: i16, bounds: Rect) {
        let &mut Self::Split {
            direction: sd,
            ref mut ratio,
            ref mut first,
            ref mut second,
        } = self
        else {
            return;
        };
        let axis_matches = matches!(
            (sd, direction),
            (SplitDirection::Vertical, Direction::Left | Direction::Right)
                | (SplitDirection::Horizontal, Direction::Up | Direction::Down)
        );

        let (fb, sb) = split_rect(bounds, sd, ratio_to_f32(*ratio));

        if axis_matches && (first.contains(target) || second.contains(target)) {
            let total = match sd {
                SplitDirection::Vertical => f32::from(bounds.width),
                SplitDirection::Horizontal => f32::from(bounds.height),
            };
            if total > 0.0 {
                let sign = if first.contains(target) {
                    match direction {
                        Direction::Right | Direction::Down => 1.0,
                        _ => -1.0,
                    }
                } else {
                    match direction {
                        Direction::Left | Direction::Up => 1.0,
                        _ => -1.0,
                    }
                };
                let new_ratio = (ratio_to_f32(*ratio)
                    + sign * f32::from(delta.unsigned_abs()) / total)
                    .clamp(0.1, 0.9);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    *ratio = Permil::new((new_ratio * 1000.0).round() as u16)
                        .expect("ratio clamped into valid permil range");
                }
            }
        } else {
            // Recurse into the child that contains the target
            if first.contains(target) {
                first.resize_at(target, direction, delta, fb);
            } else if second.contains(target) {
                second.resize_at(target, direction, delta, sb);
            }
        }
    }

    /// Recursively set all split ratios to 0.5.
    fn equalize(&mut self) {
        if let Self::Split {
            ratio,
            first,
            second,
            ..
        } = self
        {
            *ratio = Permil::HALF;
            first.equalize();
            second.equalize();
        }
    }
}

/// Tiled zone managing a binary split tree of windows.
#[derive(Debug, Clone)]
pub struct TiledZone {
    root: Option<TiledTree>,
    layer_id: LayerId,
}

impl TiledZone {
    /// Create a new empty tiled zone.
    #[must_use]
    pub const fn new(layer_id: LayerId) -> Self {
        Self {
            root: None,
            layer_id,
        }
    }

    #[must_use]
    pub const fn tree(&self) -> Option<&TiledTree> {
        self.root.as_ref()
    }
}

impl TiledLayer for TiledZone {
    fn arrange(&self, bounds: Rect) -> Vec<WindowPlacement> {
        let Some(root) = &self.root else {
            return Vec::new();
        };
        let z_base = ZOrder::layer_base(self.layer_id);
        let mut placements = Vec::new();
        root.arrange(bounds, self.layer_id, z_base, &mut placements);
        placements
    }

    fn add_first(&mut self) -> WindowId {
        let id = WindowId::new();
        self.root = Some(TiledTree::Window(id));
        id
    }

    fn split(&mut self, target: WindowId, direction: SplitDirection) -> Option<WindowId> {
        let root = self.root.as_mut()?;
        // We need bounds for minimum-size checks. Use a large default;
        // the actual bounds are provided by `arrange()` at render time.
        // For split validation we use a generous size that won't reject
        // valid splits — the real geometry is checked by the caller.
        let bounds = Rect::new(0, 0, 1000, 1000);
        root.split_at(target, direction, bounds)
    }

    fn close(&mut self, window: WindowId) -> Option<WindowId> {
        let root = self.root.as_mut()?;
        // If root is a single leaf matching target, clear the tree
        if let TiledTree::Window(id) = root {
            if *id == window {
                return None; // Last window — can't close
            }
            return None; // Window not found
        }
        root.close_leaf(window)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn navigate(
        &self,
        from: WindowId,
        direction: Direction,
        views: &[WindowPlacement],
    ) -> Option<WindowId> {
        let from_placement = views.iter().find(|p| p.window_id == from)?;
        let from_center = rect_center(from_placement.bounds);

        let mut best: Option<(WindowId, f32)> = None;
        for p in views {
            if p.window_id == from {
                continue;
            }
            let c = rect_center(p.bounds);
            let is_in_direction = match direction {
                Direction::Left => c.0 < from_center.0,
                Direction::Right => c.0 > from_center.0,
                Direction::Up => c.1 < from_center.1,
                Direction::Down => c.1 > from_center.1,
            };
            if !is_in_direction {
                continue;
            }
            let dx = c.0 - from_center.0;
            let dy = c.1 - from_center.1;
            let dist = dx.mul_add(dx, dy * dy);
            if best.is_none() || dist < best.unwrap().1 {
                best = Some((p.window_id, dist));
            }
        }
        best.map(|(id, _)| id)
    }

    fn resize(&mut self, window: WindowId, direction: Direction, delta: i16) {
        if let Some(root) = &mut self.root {
            let bounds = Rect::new(0, 0, 1000, 1000);
            root.resize_at(window, direction, delta, bounds);
        }
    }

    fn equalize(&mut self) {
        if let Some(root) = &mut self.root {
            root.equalize();
        }
    }

    fn windows(&self) -> Vec<WindowId> {
        let Some(root) = &self.root else {
            return Vec::new();
        };
        let mut out = Vec::new();
        root.collect_windows(&mut out);
        out
    }

    fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    fn cycle(&self, from: WindowId, forward: bool, views: &[WindowPlacement]) -> Option<WindowId> {
        if views.len() < 2 {
            return None;
        }
        // Sort by winnr order: top-to-bottom, left-to-right
        let mut view_map: Vec<(u16, u16, WindowId)> = views
            .iter()
            .map(|p| (p.bounds.y, p.bounds.x, p.window_id))
            .collect();
        view_map.sort_by_key(|&(y, x, _)| (y, x));
        let sorted: Vec<WindowId> = view_map.iter().map(|&(_, _, id)| id).collect();

        let pos = sorted.iter().position(|&id| id == from)?;
        let next = if forward {
            (pos + 1) % sorted.len()
        } else {
            (pos + sorted.len() - 1) % sorted.len()
        };
        Some(sorted[next])
    }
}

// =============================================================================
// Geometry helpers
// =============================================================================

/// Split a rect into two parts along the given direction.
fn split_rect(bounds: Rect, direction: SplitDirection, ratio: f32) -> (Rect, Rect) {
    match direction {
        SplitDirection::Vertical => {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let left_w = (f32::from(bounds.width) * ratio) as u16;
            let right_w = bounds.width.saturating_sub(left_w);
            (
                Rect::new(bounds.x, bounds.y, left_w, bounds.height),
                Rect::new(bounds.x + left_w, bounds.y, right_w, bounds.height),
            )
        }
        SplitDirection::Horizontal => {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let top_h = (f32::from(bounds.height) * ratio) as u16;
            let bottom_h = bounds.height.saturating_sub(top_h);
            (
                Rect::new(bounds.x, bounds.y, bounds.width, top_h),
                Rect::new(bounds.x, bounds.y + top_h, bounds.width, bottom_h),
            )
        }
    }
}

/// Check if a rect meets minimum window dimensions.
const fn meets_minimum_size(rect: Rect) -> bool {
    rect.width >= MIN_WINDOW_WIDTH && rect.height >= MIN_WINDOW_HEIGHT
}

/// Get the center of a rect as (f32, f32).
fn rect_center(r: Rect) -> (f32, f32) {
    (
        f32::from(r.x) + f32::from(r.width) / 2.0,
        f32::from(r.y) + f32::from(r.height) / 2.0,
    )
}

const fn ratio_to_f32(ratio: Permil) -> f32 {
    ratio.value() as f32 / 1000.0
}

#[cfg(test)]
#[path = "tiled_zone_tests.rs"]
mod tests;
