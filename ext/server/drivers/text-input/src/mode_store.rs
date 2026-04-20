//! Mode info store for `ServiceRegistry`.

use std::sync::RwLock;

use {
    crate::ModeInfo,
    reovim_kernel::api::v1::{Mode, ModeId, Service},
};

/// Store for mode information registered by modules.
pub struct ModeInfoStore {
    modes: RwLock<Vec<ModeInfo>>,
}

impl ModeInfoStore {
    /// Create a new empty mode store.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new() -> Self {
        Self {
            modes: RwLock::new(Vec::new()),
        }
    }

    /// Add a mode to the store.
    pub fn add(&self, info: ModeInfo) {
        self.modes
            .write()
            .expect("ModeInfoStore lock poisoned")
            .push(info);
    }

    /// Add a mode from a Mode implementation.
    pub fn add_mode<M: Mode>(&self, mode: M) {
        self.add(ModeInfo::from_mode(mode));
    }

    /// Take all modes, clearing the store.
    pub fn take_modes(&self) -> Vec<ModeInfo> {
        std::mem::take(&mut *self.modes.write().expect("ModeInfoStore lock poisoned"))
    }

    /// Get the number of registered modes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.modes
            .read()
            .expect("ModeInfoStore lock poisoned")
            .len()
    }

    /// Check if the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modes
            .read()
            .expect("ModeInfoStore lock poisoned")
            .is_empty()
    }

    /// Find a registered mode by module name and mode name.
    #[must_use]
    pub fn find_by_name(&self, module: &str, name: &str) -> Option<ModeId> {
        self.modes
            .read()
            .expect("ModeInfoStore lock poisoned")
            .iter()
            .find(|m| m.id.module().as_str() == module && m.id.name() == name)
            .map(|m| m.id.clone())
    }
}

impl Default for ModeInfoStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for ModeInfoStore {}

impl std::fmt::Debug for ModeInfoStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModeInfoStore")
            .field("count", &self.len())
            .finish()
    }
}
