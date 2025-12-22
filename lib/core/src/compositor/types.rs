//! Core types for the compositor system

use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::{
    frame::FrameBuffer,
    highlight::{ColorMode, Theme},
};

/// Global sequence counter for z-order tie-breaking
static SEQUENCE_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Generate a unique sequence number for z-order
fn next_sequence() -> u32 {
    SEQUENCE_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Z-order groups for categorizing composable elements
///
/// Elements within the same group are ordered by `sub_order` and `sequence`.
/// Groups are ordered from bottom (rendered first) to top (rendered last).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u16)]
pub enum ZGroup {
    /// Base layer: tab line, status line (always at bottom)
    #[default]
    Base = 0,
    /// Sidebar elements: explorer, file tree
    Sidebar = 100,
    /// Editor windows (split windows, text content)
    Editor = 200,
    /// Floating windows (non-modal popups)
    Floating = 300,
    /// Overlays: leap labels, inline hints
    Overlay = 400,
    /// Popups: completion menu
    Popup = 500,
    /// Panels: which-key hints
    Panel = 600,
    /// Modal overlays: telescope, settings menu
    Modal = 700,
    /// Alert dialogs (highest priority)
    Alert = 800,
}

impl ZGroup {
    /// Get the numeric value for ordering
    #[must_use]
    pub const fn value(self) -> u16 {
        self as u16
    }
}

/// Fine-grained z-order specification
///
/// Ordering priority: `group` > `sub_order` > `sequence`
/// - `group`: Major category (Base, Editor, Modal, etc.)
/// - `sub_order`: Within-group priority (0-255)
/// - `sequence`: Tie-breaker for same group+`sub_order` (auto-generated)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZOrder {
    pub group: ZGroup,
    pub sub_order: u8,
    pub sequence: u32,
}

impl ZOrder {
    /// Create a new z-order with the given group and sub-order
    #[must_use]
    pub fn new(group: ZGroup, sub_order: u8) -> Self {
        Self {
            group,
            sub_order,
            sequence: next_sequence(),
        }
    }

    /// Create a z-order for the base layer
    #[must_use]
    pub fn base() -> Self {
        Self::new(ZGroup::Base, 0)
    }

    /// Create a z-order for editor windows
    #[must_use]
    pub fn editor(sub_order: u8) -> Self {
        Self::new(ZGroup::Editor, sub_order)
    }

    /// Create a z-order for floating windows
    #[must_use]
    pub fn floating(sub_order: u8) -> Self {
        Self::new(ZGroup::Floating, sub_order)
    }

    /// Create a z-order for overlays
    #[must_use]
    pub fn overlay(sub_order: u8) -> Self {
        Self::new(ZGroup::Overlay, sub_order)
    }

    /// Create a z-order for popups
    #[must_use]
    pub fn popup(sub_order: u8) -> Self {
        Self::new(ZGroup::Popup, sub_order)
    }

    /// Create a z-order for panels
    #[must_use]
    pub fn panel(sub_order: u8) -> Self {
        Self::new(ZGroup::Panel, sub_order)
    }

    /// Create a z-order for modal overlays
    #[must_use]
    pub fn modal(sub_order: u8) -> Self {
        Self::new(ZGroup::Modal, sub_order)
    }

    /// Create a z-order for alerts
    #[must_use]
    pub fn alert(sub_order: u8) -> Self {
        Self::new(ZGroup::Alert, sub_order)
    }

    /// Bring to front within the same group (updates sequence)
    pub fn bring_to_front(&mut self) {
        self.sequence = next_sequence();
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

/// Unique identifier for composable elements
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComposableId {
    /// Base layer (tab line + status line)
    Base,
    /// Tab line
    TabLine,
    /// Status line
    StatusLine,
    /// Editor window by ID
    Window(usize),
    /// Floating window by ID
    FloatingWindow(usize),
    /// Which-key panel
    WhichKey,
    // SettingsMenu is now handled by the settings-menu plugin
    /// Custom identifier
    Custom(&'static str),
}

impl ComposableId {
    /// Get the default z-group for this composable type
    #[must_use]
    pub const fn default_group(&self) -> ZGroup {
        match self {
            Self::Base | Self::TabLine | Self::StatusLine => ZGroup::Base,
            Self::Window(_) | Self::Custom(_) => ZGroup::Editor,
            Self::FloatingWindow(_) => ZGroup::Floating,
            Self::WhichKey => ZGroup::Panel,
        }
    }
}

/// Bounds of a composable element
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Bounds {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Bounds {
    /// Create new bounds
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create full-screen bounds
    #[must_use]
    pub const fn full_screen(width: u16, height: u16) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    /// Check if a point is inside these bounds
    #[must_use]
    pub const fn contains(&self, px: u16, py: u16) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    /// Check if two bounds overlap
    #[must_use]
    pub const fn overlaps(&self, other: &Self) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
}

/// Trait for composable elements that can be rendered by the compositor
///
/// This unifies the `Layer` and `Overlay` traits under a single interface.
pub trait Composable: std::fmt::Debug + Send + Sync {
    /// Unique identifier for this composable
    fn id(&self) -> ComposableId;

    /// Current z-order
    fn z_order(&self) -> ZOrder;

    /// Set the z-order (for bring-to-front/send-to-back operations)
    fn set_z_order(&mut self, z: ZOrder);

    /// Whether this composable is currently visible
    fn is_visible(&self) -> bool;

    /// Compute bounds based on screen dimensions
    fn bounds(&self, screen_width: u16, screen_height: u16) -> Bounds;

    /// Render to the frame buffer
    fn render(&self, buffer: &mut FrameBuffer, theme: &Theme, color_mode: ColorMode);

    /// Whether this composable captures keyboard input when visible
    ///
    /// When true, input events are consumed by this composable.
    fn captures_keyboard(&self) -> bool {
        true
    }

    /// Cursor position if this composable owns the cursor
    ///
    /// Returns `Some((x, y))` if the cursor should be positioned here.
    fn cursor_position(&self) -> Option<(u16, u16)> {
        None
    }
}

/// Entry in the compositor registry
pub struct CompositorEntry {
    /// The composable element
    pub composable: Box<dyn Composable>,
    /// Cached z-order for sorting
    pub z_order: ZOrder,
}

impl CompositorEntry {
    /// Create a new entry
    #[must_use]
    pub fn new(composable: Box<dyn Composable>) -> Self {
        let z_order = composable.z_order();
        Self {
            composable,
            z_order,
        }
    }
}

/// Unified compositor for z-ordered rendering
///
/// Manages registration, z-ordering, and rendering of all composable elements.
pub struct Compositor {
    /// Registered composables by ID
    entries: BTreeMap<ComposableId, CompositorEntry>,
    /// Render order cache (sorted by z-order)
    render_order: Vec<ComposableId>,
    /// Whether render order needs recalculation
    order_dirty: bool,
    /// Currently focused composable (receives keyboard input)
    focused_id: Option<ComposableId>,
}

impl Default for Compositor {
    fn default() -> Self {
        Self::new()
    }
}

impl Compositor {
    /// Create a new empty compositor
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            render_order: Vec::new(),
            order_dirty: true,
            focused_id: None,
        }
    }

    /// Register a composable element
    pub fn register(&mut self, composable: Box<dyn Composable>) {
        let id = composable.id();
        self.entries.insert(id, CompositorEntry::new(composable));
        self.order_dirty = true;
    }

    /// Unregister a composable element
    pub fn unregister(&mut self, id: ComposableId) {
        self.entries.remove(&id);
        self.order_dirty = true;
        if self.focused_id == Some(id) {
            self.focused_id = None;
        }
    }

    /// Get a reference to a composable entry by ID
    #[must_use]
    pub fn get(&self, id: ComposableId) -> Option<&CompositorEntry> {
        self.entries.get(&id)
    }

    /// Get a mutable reference to a composable entry by ID
    pub fn get_mut(&mut self, id: ComposableId) -> Option<&mut CompositorEntry> {
        self.entries.get_mut(&id)
    }

    /// Bring a composable to the front of its z-group
    pub fn bring_to_front(&mut self, id: ComposableId) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.z_order.bring_to_front();
            entry.composable.set_z_order(entry.z_order);
            self.order_dirty = true;
        }
    }

    /// Set the z-group for a composable
    pub fn set_z_group(&mut self, id: ComposableId, group: ZGroup) {
        if let Some(entry) = self.entries.get_mut(&id) {
            let new_z = ZOrder::new(group, entry.z_order.sub_order);
            entry.z_order = new_z;
            entry.composable.set_z_order(new_z);
            self.order_dirty = true;
        }
    }

    /// Set focus to a specific composable
    pub const fn set_focus(&mut self, id: Option<ComposableId>) {
        self.focused_id = id;
    }

    /// Get the currently focused composable
    #[must_use]
    pub const fn focused(&self) -> Option<ComposableId> {
        self.focused_id
    }

    /// Find the topmost visible composable that captures keyboard input
    #[must_use]
    pub fn keyboard_target(&mut self) -> Option<ComposableId> {
        self.ensure_render_order();
        self.render_order
            .iter()
            .rev()
            .find(|id| {
                self.entries
                    .get(id)
                    .is_some_and(|e| e.composable.is_visible() && e.composable.captures_keyboard())
            })
            .copied()
    }

    /// Hit test: find the topmost visible composable at a point
    #[must_use]
    pub fn hit_test(
        &mut self,
        x: u16,
        y: u16,
        screen_width: u16,
        screen_height: u16,
    ) -> Option<ComposableId> {
        self.ensure_render_order();
        self.render_order
            .iter()
            .rev()
            .find(|id| {
                self.entries.get(id).is_some_and(|e| {
                    e.composable.is_visible()
                        && e.composable
                            .bounds(screen_width, screen_height)
                            .contains(x, y)
                })
            })
            .copied()
    }

    /// Render all visible composables in z-order
    ///
    /// Returns the cursor position from the topmost composable that provides one.
    pub fn render(
        &mut self,
        buffer: &mut FrameBuffer,
        theme: &Theme,
        color_mode: ColorMode,
    ) -> Option<(u16, u16)> {
        self.ensure_render_order();

        let mut cursor_pos = None;
        let screen_width = buffer.width();
        let screen_height = buffer.height();

        // Collect IDs to render (avoids borrow issues)
        let ids: Vec<ComposableId> = self.render_order.clone();

        for id in ids {
            if let Some(entry) = self.entries.get(&id)
                && entry.composable.is_visible()
            {
                let bounds = entry.composable.bounds(screen_width, screen_height);
                // Only render if bounds are valid
                if bounds.width > 0 && bounds.height > 0 {
                    entry.composable.render(buffer, theme, color_mode);
                }
                // Topmost cursor wins
                if let Some(pos) = entry.composable.cursor_position() {
                    cursor_pos = Some(pos);
                }
            }
        }

        cursor_pos
    }

    /// Ensure render order is up to date
    fn ensure_render_order(&mut self) {
        if self.order_dirty {
            self.rebuild_render_order();
        }
    }

    /// Rebuild the render order cache
    fn rebuild_render_order(&mut self) {
        self.render_order.clear();
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|a, b| a.1.z_order.cmp(&b.1.z_order));
        self.render_order = entries.into_iter().map(|(id, _)| *id).collect();
        self.order_dirty = false;
    }

    /// Get the number of registered composables
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the compositor is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all registered composable IDs
    pub fn ids(&self) -> impl Iterator<Item = &ComposableId> {
        self.entries.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_z_group_ordering() {
        assert!(ZGroup::Base < ZGroup::Editor);
        assert!(ZGroup::Editor < ZGroup::Floating);
        assert!(ZGroup::Floating < ZGroup::Modal);
        assert!(ZGroup::Modal < ZGroup::Alert);
    }

    #[test]
    fn test_z_order_ordering() {
        let base = ZOrder::base();
        let editor = ZOrder::editor(0);
        let modal = ZOrder::modal(0);

        assert!(base < editor);
        assert!(editor < modal);
    }

    #[test]
    fn test_z_order_sub_order() {
        let editor_low = ZOrder::editor(0);
        let editor_high = ZOrder::editor(10);

        assert!(editor_low < editor_high);
    }

    #[test]
    fn test_z_order_sequence() {
        let first = ZOrder::editor(0);
        let second = ZOrder::editor(0);

        // Same group and sub_order, but second has higher sequence
        assert!(first < second);
    }

    #[test]
    fn test_bounds_contains() {
        let bounds = Bounds::new(10, 20, 30, 40);

        assert!(bounds.contains(10, 20)); // top-left corner
        assert!(bounds.contains(25, 40)); // middle
        assert!(bounds.contains(39, 59)); // just inside bottom-right
        assert!(!bounds.contains(40, 60)); // just outside
        assert!(!bounds.contains(9, 20)); // just left
    }

    #[test]
    fn test_bounds_overlaps() {
        let a = Bounds::new(0, 0, 50, 50);
        let b = Bounds::new(25, 25, 50, 50);
        let c = Bounds::new(100, 100, 10, 10);

        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
        assert!(!a.overlaps(&c));
        assert!(!c.overlaps(&a));
    }

    #[test]
    fn test_composable_id_default_group() {
        assert_eq!(ComposableId::Base.default_group(), ZGroup::Base);
        assert_eq!(ComposableId::Window(0).default_group(), ZGroup::Editor);
        assert_eq!(ComposableId::FloatingWindow(0).default_group(), ZGroup::Floating);
        assert_eq!(ComposableId::WhichKey.default_group(), ZGroup::Panel);
    }

    #[test]
    fn test_compositor_empty() {
        let compositor = Compositor::new();
        assert!(compositor.is_empty());
        assert_eq!(compositor.len(), 0);
    }
}
