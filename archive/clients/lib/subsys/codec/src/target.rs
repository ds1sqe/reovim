//! `FrameTarget`: typeid-keyed bag of capability slots.
//!
//! A `FrameTarget` carries zero or more capabilities that render handlers
//! populate and that surface encoders consume. Handlers look up a slot
//! by its concrete Rust type via [`FrameTarget::get_mut`].
//!
//! Core is shape-blind: it never knows whether a target is a cell grid,
//! a pixel buffer, a DOM tree, or a mesh scene. Those types live in ext
//! crates (`ext/client/<platform>/capabilities/*`) and are inserted into
//! the target by the ext handler that produces them.
//!
//! `FrameTarget` moves through async tasks between the network decoder
//! and the render driver, so slot values must be `Send`. `Sync` is not
//! required because mutation goes through `&mut self`.

use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

/// Typeid-keyed bag of capability slots.
///
/// Insert a slot with [`insert`](Self::insert), look it up with
/// [`get_mut`](Self::get_mut) or [`get`](Self::get), remove it with
/// [`remove`](Self::remove). All operations are type-erased through
/// `Box<dyn Any + Send>` and return concrete typed references on lookup.
#[derive(Default)]
pub struct FrameTarget {
    slots: HashMap<TypeId, Box<dyn Any + Send>>,
}

impl std::fmt::Debug for FrameTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameTarget")
            .field("slot_count", &self.slots.len())
            .finish()
    }
}

impl FrameTarget {
    /// Create an empty `FrameTarget`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a capability slot. Replaces any existing slot of the same type.
    pub fn insert<T: Any + Send + 'static>(&mut self, value: T) {
        self.slots.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Look up a mutable handle to a capability slot by type.
    /// Returns `None` if the slot is not present.
    pub fn get_mut<T: Any + 'static>(&mut self) -> Option<&mut T> {
        self.slots
            .get_mut(&TypeId::of::<T>())
            .and_then(|b| b.downcast_mut::<T>())
    }

    /// Read-only variant of [`get_mut`](Self::get_mut).
    #[must_use]
    pub fn get<T: Any + 'static>(&self) -> Option<&T> {
        self.slots
            .get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref::<T>())
    }

    /// Whether a slot of type `T` is present.
    #[must_use]
    pub fn has<T: Any + 'static>(&self) -> bool {
        self.slots.contains_key(&TypeId::of::<T>())
    }

    /// Remove a capability slot, returning the owned value if present.
    pub fn remove<T: Any + Send + 'static>(&mut self) -> Option<T> {
        self.slots
            .remove(&TypeId::of::<T>())
            .and_then(|b| b.downcast::<T>().ok().map(|b| *b))
    }

    /// Number of capability slots currently held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether no slots are held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Drop all slots.
    pub fn clear(&mut self) {
        self.slots.clear();
    }
}
