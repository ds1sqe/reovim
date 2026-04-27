//! Layer compositor implementation.

use std::collections::BTreeMap;

use super::{Composable, ComposableId, FrameBuffer, Style, ZGroup, ZOrder};

/// Entry in the compositor registry.
///
/// Stores both the composable and a cached z-order value for efficient sorting.
pub struct CompositorEntry {
    /// The composable element
    pub composable: Box<dyn Composable>,
    /// Cached z-order for sorting
    pub z_order: ZOrder,
}

impl CompositorEntry {
    /// Create a new entry from a composable.
    ///
    /// The z-order is initialized from the composable's current z-order.
    #[must_use]
    pub fn new(composable: Box<dyn Composable>) -> Self {
        let z_order = composable.z_order();
        Self {
            composable,
            z_order,
        }
    }
}

/// Layer compositor for z-ordered rendering.
///
/// Manages a collection of composable elements and renders them in z-order.
/// Elements with higher z-order are rendered on top of lower ones.
pub struct LayerCompositor {
    /// Registered composables by ID
    entries: BTreeMap<ComposableId, CompositorEntry>,
    /// Cached render order (sorted by z-order)
    render_order: Vec<ComposableId>,
    /// Whether `render_order` needs rebuilding
    order_dirty: bool,
    /// Currently focused element
    focused_id: Option<ComposableId>,
}

impl Default for LayerCompositor {
    fn default() -> Self {
        Self::new()
    }
}

impl LayerCompositor {
    /// Create a new empty layer compositor.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            render_order: Vec::new(),
            order_dirty: true,
            focused_id: None,
        }
    }

    // ========================================================================
    // Registration
    // ========================================================================

    /// Register a composable element.
    ///
    /// If an element with the same ID already exists, it is replaced.
    pub fn register(&mut self, composable: Box<dyn Composable>) {
        let id = composable.id();
        self.entries.insert(id, CompositorEntry::new(composable));
        self.order_dirty = true;
    }

    /// Unregister a composable by ID.
    ///
    /// If the removed element was focused, focus is cleared.
    pub fn unregister(&mut self, id: ComposableId) {
        self.entries.remove(&id);
        self.order_dirty = true;
        if self.focused_id == Some(id) {
            self.focused_id = None;
        }
    }

    // ========================================================================
    // Entry Access
    // ========================================================================

    /// Get a composable entry by ID.
    #[must_use]
    pub fn get_entry(&self, id: ComposableId) -> Option<&CompositorEntry> {
        self.entries.get(&id)
    }

    /// Get a mutable composable entry by ID.
    pub fn get_entry_mut(&mut self, id: ComposableId) -> Option<&mut CompositorEntry> {
        self.entries.get_mut(&id)
    }

    /// Get a composable by ID (trait object).
    #[must_use]
    pub fn get(&self, id: ComposableId) -> Option<&dyn Composable> {
        self.get_entry(id).map(|e| e.composable.as_ref())
    }

    /// Get a mutable composable by ID (trait object).
    pub fn get_mut(&mut self, id: ComposableId) -> Option<&mut (dyn Composable + '_)> {
        Some(self.get_entry_mut(id)?.composable.as_mut())
    }

    // ========================================================================
    // Z-Order Management
    // ========================================================================

    /// Set the z-order for a composable.
    ///
    /// Updates both the cached z-order and the composable's z-order.
    pub fn set_z_order(&mut self, id: ComposableId, z: ZOrder) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.z_order = z;
            entry.composable.set_z_order(z);
            self.order_dirty = true;
        }
    }

    /// Set the z-group for a composable, preserving `sub_order`.
    ///
    /// Creates a new z-order with the same `sub_order` but different group.
    pub fn set_z_group(&mut self, id: ComposableId, group: ZGroup) {
        if let Some(entry) = self.entries.get_mut(&id) {
            let new_z = ZOrder::new(group, entry.z_order.sub_order);
            entry.z_order = new_z;
            entry.composable.set_z_order(new_z);
            self.order_dirty = true;
        }
    }

    /// Bring a composable to the front within its z-group.
    ///
    /// Updates the sequence number to make this the topmost element
    /// among those with the same group and `sub_order`.
    pub fn bring_to_front(&mut self, id: ComposableId) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.z_order.bring_to_front();
            entry.composable.set_z_order(entry.z_order);
            self.order_dirty = true;
        }
    }

    /// Send a composable to the back within its z-group.
    ///
    /// Resets the sequence to 0, making this the bottommost element
    /// among those with the same group and `sub_order`.
    pub fn send_to_back(&mut self, id: ComposableId) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.z_order.send_to_back();
            entry.composable.set_z_order(entry.z_order);
            self.order_dirty = true;
        }
    }

    // ========================================================================
    // Focus Management
    // ========================================================================

    /// Set the focused element.
    pub const fn set_focus(&mut self, id: Option<ComposableId>) {
        self.focused_id = id;
    }

    /// Get the currently focused element ID.
    #[must_use]
    pub const fn focused(&self) -> Option<ComposableId> {
        self.focused_id
    }

    /// Find the keyboard target (topmost that captures keyboard).
    ///
    /// Returns the ID of the topmost visible composable that captures
    /// keyboard input, or None if no such element exists.
    pub fn keyboard_target(&mut self) -> Option<ComposableId> {
        self.ensure_render_order();

        // Search from back (highest z-order) to front
        // render_order is built from entries.keys(), so indexing is always valid
        for id in self.render_order.iter().rev() {
            let entry = &self.entries[id];
            if entry.composable.is_visible() && entry.composable.captures_keyboard() {
                return Some(*id);
            }
        }
        None
    }

    // ========================================================================
    // Hit Testing
    // ========================================================================

    /// Hit test for the topmost element at a position.
    ///
    /// Returns the ID of the topmost visible composable that contains
    /// the given point, or None if no element is at that position.
    pub fn hit_test(&mut self, x: u16, y: u16, buffer: &FrameBuffer) -> Option<ComposableId> {
        self.ensure_render_order();

        let screen_width = buffer.width();
        let screen_height = buffer.height();

        // Search from back (highest z-order) to front
        // render_order is built from entries.keys(), so indexing is always valid
        for id in self.render_order.iter().rev() {
            let entry = &self.entries[id];
            if entry.composable.is_visible() {
                let bounds = entry.composable.bounds(screen_width, screen_height);
                if bounds.contains(x, y) {
                    return Some(*id);
                }
            }
        }
        None
    }

    // ========================================================================
    // Rendering
    // ========================================================================

    /// Render all visible composables in z-order.
    ///
    /// Returns the cursor position from the topmost composable that provides one.
    pub fn render(
        &mut self,
        buffer: &mut FrameBuffer,
        default_style: &Style,
    ) -> Option<(u16, u16)> {
        self.ensure_render_order();

        let screen_width = buffer.width();
        let screen_height = buffer.height();

        // Clone render_order to avoid borrow conflicts
        // (need immutable iteration while mutably accessing entries for rendering)
        let ids: Vec<ComposableId> = self.render_order.clone();

        let mut cursor_pos = None;

        for entry in ids.iter().filter_map(|id| self.entries.get(id)) {
            if !entry.composable.is_visible() {
                continue;
            }

            let bounds = entry.composable.bounds(screen_width, screen_height);

            // Skip composables with zero-size bounds
            if bounds.is_empty() {
                continue;
            }

            entry.composable.render(buffer, default_style);

            // Cursor position from topmost wins (last in iteration)
            if let Some(pos) = entry.composable.cursor_position() {
                cursor_pos = Some(pos);
            }
        }

        cursor_pos
    }

    /// Get the render order (sorted by z-order).
    ///
    /// Returns a clone of the internal render order.
    #[must_use]
    pub fn render_order(&mut self) -> Vec<ComposableId> {
        self.ensure_render_order();
        self.render_order.clone()
    }

    // ========================================================================
    // Utilities
    // ========================================================================

    /// Get the number of registered composables.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the compositor is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all registered composable IDs.
    pub fn ids(&self) -> impl Iterator<Item = &ComposableId> {
        self.entries.keys()
    }

    // ========================================================================
    // Internal
    // ========================================================================

    /// Ensure `render_order` is up to date.
    fn ensure_render_order(&mut self) {
        if self.order_dirty {
            self.rebuild_render_order();
            self.order_dirty = false;
        }
    }

    /// Rebuild the render order by sorting entries by z-order.
    fn rebuild_render_order(&mut self) {
        self.render_order.clear();
        self.render_order.extend(self.entries.keys().copied());

        // Sort by z-order (lower first, rendered first = behind)
        self.render_order.sort_by(|a, b| {
            let z_a = self.entries.get(a).map(|e| e.z_order);
            let z_b = self.entries.get(b).map(|e| e.z_order);
            z_a.cmp(&z_b)
        });
    }
}

#[cfg(test)]
#[path = "layer_tests.rs"]
mod tests;
