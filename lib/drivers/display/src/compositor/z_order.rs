//! Z-order types for layer management.
//!
//! Z-order determines the rendering order of composable elements.
//! Elements with higher z-order are rendered on top.

use std::sync::atomic::{AtomicU32, Ordering};

/// Global sequence counter for z-order tie-breaking.
///
/// This counter is GLOBAL (not per-compositor) and ensures that
/// elements created/moved later have higher sequence numbers.
static SEQUENCE_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Get the next sequence number.
fn next_sequence() -> u32 {
    SEQUENCE_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Z-order group categories.
///
/// Each group represents a major layer category with a fixed priority.
/// Elements in higher groups always render on top of lower groups.
///
/// # Values
///
/// These values are part of the API contract and MUST NOT change:
/// - `Base` = 0 (tab line, status line)
/// - `Sidebar` = 100 (explorer, file tree)
/// - `Editor` = 200 (editor windows)
/// - `Floating` = 300 (floating windows)
/// - `Overlay` = 400 (leap labels, inline hints)
/// - `Popup` = 500 (completion menu)
/// - `Panel` = 600 (which-key hints)
/// - `Modal` = 700 (telescope, settings)
/// - `Alert` = 800 (alert dialogs)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u16)]
pub enum ZGroup {
    /// Base layer: tab line, status line (always at bottom)
    #[default]
    Base = 0,
    /// Sidebar: explorer, file tree
    Sidebar = 100,
    /// Editor: split windows, text content
    Editor = 200,
    /// Floating: non-modal floating windows
    Floating = 300,
    /// Overlay: leap labels, inline hints
    Overlay = 400,
    /// Popup: completion menu
    Popup = 500,
    /// Panel: which-key hints
    Panel = 600,
    /// Modal: telescope, settings menu
    Modal = 700,
    /// Alert: alert dialogs (highest priority)
    Alert = 800,
}

impl ZGroup {
    /// Get the numeric value for ordering.
    #[must_use]
    pub const fn value(self) -> u16 {
        self as u16
    }
}

/// Fine-grained z-order for composable elements.
///
/// Z-order comparison uses the following priority (highest to lowest):
/// 1. `group` - Major category (`ZGroup`)
/// 2. `sub_order` - Within-group priority (0-255)
/// 3. `sequence` - Tie-breaker for same `group`+`sub_order`
///
/// The sequence is automatically assigned from a global counter,
/// so elements created later have higher sequence numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZOrder {
    /// Major z-order category
    pub group: ZGroup,
    /// Within-group priority (0-255)
    pub sub_order: u8,
    /// Sequence number for tie-breaking (auto-assigned)
    pub sequence: u32,
}

impl ZOrder {
    /// Create a new z-order with the given group and sub-order.
    ///
    /// The sequence is automatically assigned from the global counter.
    #[must_use]
    pub fn new(group: ZGroup, sub_order: u8) -> Self {
        Self {
            group,
            sub_order,
            sequence: next_sequence(),
        }
    }

    /// Create base layer z-order.
    #[must_use]
    pub fn base() -> Self {
        Self::new(ZGroup::Base, 0)
    }

    /// Create sidebar z-order with sub-priority.
    #[must_use]
    pub fn sidebar(sub_order: u8) -> Self {
        Self::new(ZGroup::Sidebar, sub_order)
    }

    /// Create editor z-order with sub-priority.
    #[must_use]
    pub fn editor(sub_order: u8) -> Self {
        Self::new(ZGroup::Editor, sub_order)
    }

    /// Create floating window z-order with sub-priority.
    #[must_use]
    pub fn floating(sub_order: u8) -> Self {
        Self::new(ZGroup::Floating, sub_order)
    }

    /// Create overlay z-order with sub-priority.
    #[must_use]
    pub fn overlay(sub_order: u8) -> Self {
        Self::new(ZGroup::Overlay, sub_order)
    }

    /// Create popup z-order with sub-priority.
    #[must_use]
    pub fn popup(sub_order: u8) -> Self {
        Self::new(ZGroup::Popup, sub_order)
    }

    /// Create panel z-order with sub-priority.
    #[must_use]
    pub fn panel(sub_order: u8) -> Self {
        Self::new(ZGroup::Panel, sub_order)
    }

    /// Create modal z-order with sub-priority.
    #[must_use]
    pub fn modal(sub_order: u8) -> Self {
        Self::new(ZGroup::Modal, sub_order)
    }

    /// Create alert z-order with sub-priority.
    #[must_use]
    pub fn alert(sub_order: u8) -> Self {
        Self::new(ZGroup::Alert, sub_order)
    }

    /// Bring this element to the front within its group.
    ///
    /// Updates the sequence number to the current global value,
    /// making this the topmost element among those with the same
    /// group and `sub_order`.
    pub fn bring_to_front(&mut self) {
        self.sequence = next_sequence();
    }

    /// Send this element to the back within its group.
    ///
    /// Resets the sequence to 0, making this the bottommost element
    /// among those with the same group and `sub_order`.
    pub const fn send_to_back(&mut self) {
        self.sequence = 0;
    }
}

impl Ord for ZOrder {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.group
            .cmp(&other.group)
            .then_with(|| self.sub_order.cmp(&other.sub_order))
            .then_with(|| self.sequence.cmp(&other.sequence))
    }
}

impl PartialOrd for ZOrder {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_z_group_ordering() {
        assert!(ZGroup::Base < ZGroup::Sidebar);
        assert!(ZGroup::Sidebar < ZGroup::Editor);
        assert!(ZGroup::Editor < ZGroup::Floating);
        assert!(ZGroup::Floating < ZGroup::Overlay);
        assert!(ZGroup::Overlay < ZGroup::Popup);
        assert!(ZGroup::Popup < ZGroup::Panel);
        assert!(ZGroup::Panel < ZGroup::Modal);
        assert!(ZGroup::Modal < ZGroup::Alert);
    }

    #[test]
    fn test_zgroup_values_match_spec() {
        // CRITICAL: These values are part of the API contract!
        assert_eq!(ZGroup::Base as u16, 0);
        assert_eq!(ZGroup::Sidebar as u16, 100);
        assert_eq!(ZGroup::Editor as u16, 200);
        assert_eq!(ZGroup::Floating as u16, 300);
        assert_eq!(ZGroup::Overlay as u16, 400);
        assert_eq!(ZGroup::Popup as u16, 500);
        assert_eq!(ZGroup::Panel as u16, 600);
        assert_eq!(ZGroup::Modal as u16, 700);
        assert_eq!(ZGroup::Alert as u16, 800);
    }

    #[test]
    fn test_z_group_value_method() {
        assert_eq!(ZGroup::Base.value(), 0);
        assert_eq!(ZGroup::Alert.value(), 800);
    }

    #[test]
    fn test_z_order_ordering_across_groups() {
        let base = ZOrder::base();
        let editor = ZOrder::editor(0);
        let modal = ZOrder::modal(0);
        assert!(base < editor);
        assert!(editor < modal);
    }

    #[test]
    fn test_z_order_sub_order_within_group() {
        let low = ZOrder::editor(0);
        let high = ZOrder::editor(10);
        assert!(low < high);
    }

    #[test]
    fn test_z_order_sequence_tie_breaking() {
        let first = ZOrder::editor(0);
        let second = ZOrder::editor(0);
        // Same group and sub_order, but second has higher sequence (global counter)
        assert!(first < second);
    }

    #[test]
    fn test_z_order_bring_to_front() {
        let mut z = ZOrder::editor(5);
        let old_seq = z.sequence;
        z.bring_to_front();
        assert!(z.sequence > old_seq);
        assert_eq!(z.group, ZGroup::Editor); // Group unchanged
        assert_eq!(z.sub_order, 5); // Sub-order unchanged
    }

    #[test]
    fn test_z_order_send_to_back() {
        let mut z = ZOrder::editor(5);
        z.bring_to_front(); // Ensure non-zero sequence
        z.send_to_back();
        assert_eq!(z.sequence, 0);
        assert_eq!(z.group, ZGroup::Editor); // Group unchanged
        assert_eq!(z.sub_order, 5); // Sub-order unchanged
    }

    #[test]
    fn test_z_order_all_constructors() {
        // Verify all 9 constructors exist and produce correct groups
        assert_eq!(ZOrder::base().group, ZGroup::Base);
        assert_eq!(ZOrder::sidebar(0).group, ZGroup::Sidebar);
        assert_eq!(ZOrder::editor(0).group, ZGroup::Editor);
        assert_eq!(ZOrder::floating(0).group, ZGroup::Floating);
        assert_eq!(ZOrder::overlay(0).group, ZGroup::Overlay);
        assert_eq!(ZOrder::popup(0).group, ZGroup::Popup);
        assert_eq!(ZOrder::panel(0).group, ZGroup::Panel);
        assert_eq!(ZOrder::modal(0).group, ZGroup::Modal);
        assert_eq!(ZOrder::alert(0).group, ZGroup::Alert);
    }
}
