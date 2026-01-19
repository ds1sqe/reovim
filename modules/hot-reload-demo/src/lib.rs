//! Hot Reload Demo Module
//!
//! Demonstrates the module hot reload functionality with state preservation.
//! This module maintains a counter and reload statistics that persist across
//! hot reloads.
//!
//! # State Preservation
//!
//! On hot reload:
//! 1. `save_state()` serializes counter, `reload_count`, and timestamps
//! 2. Module code is unloaded and reloaded from disk
//! 3. `restore_state()` deserializes and restores the state
//! 4. Counter value is preserved, `reload_count` is incremented
//!
//! # Binary State Format
//!
//! ```text
//! Bytes 0-3:   version (u32, little-endian) - state format version
//! Bytes 4-11:  counter (i64, little-endian) - current counter value
//! Bytes 12-15: reload_count (u32, little-endian) - number of hot reloads
//! Bytes 16-23: initial_load_time (u64, little-endian) - Unix timestamp
//! ```
//!
//! # Commands
//!
//! - `demo:increment` - Increment counter by count (default 1)
//! - `demo:decrement` - Decrement counter by count (default 1)
//! - `demo:show` - Display counter, reload count, timestamps
//! - `demo:reset` - Reset counter to zero
//!
//! # Example
//!
//! ```ignore
//! // Load the module
//! :module-load ~/.local/share/reovim/modules/libreovim_module_hot_reload_demo.so
//!
//! // Increment counter
//! :demo:increment 5
//!
//! // Show state
//! :demo:show
//! // Counter: 5, Reloads: 0, Since: 2025-01-14 12:00:00
//!
//! // Hot reload (after recompiling)
//! :module-reload hot-reload-demo
//!
//! // Counter is preserved, reload count incremented
//! :demo:show
//! // Counter: 5, Reloads: 1, Since: 2025-01-14 12:00:00
//! ```

use std::time::{SystemTime, UNIX_EPOCH};

use reovim_kernel::api::v1::{
    CommandRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
};

/// Current state format version.
///
/// Increment when changing the binary format of `save_state()`.
const STATE_VERSION: u32 = 1;

/// Hot reload demo module.
///
/// Maintains a counter and tracks hot reload statistics.
pub struct HotReloadDemoModule {
    /// Current counter value.
    counter: i64,

    /// Number of hot reloads since initial load.
    reload_count: u32,

    /// Unix timestamp of initial load (preserved across reloads).
    initial_load_time: Option<u64>,

    /// Unix timestamp of last hot reload.
    last_reload_time: Option<u64>,

    /// Whether module has been initialized.
    initialized: bool,
}

impl HotReloadDemoModule {
    /// Create a new demo module instance.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            counter: 0,
            reload_count: 0,
            initial_load_time: None,
            last_reload_time: None,
            initialized: false,
        }
    }

    /// Get current Unix timestamp.
    fn now_unix() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Get the current counter value.
    #[must_use]
    pub const fn counter(&self) -> i64 {
        self.counter
    }

    /// Get the reload count.
    #[must_use]
    pub const fn reload_count(&self) -> u32 {
        self.reload_count
    }

    /// Get the initial load time.
    #[must_use]
    pub const fn initial_load_time(&self) -> Option<u64> {
        self.initial_load_time
    }

    /// Get the last reload time.
    #[must_use]
    pub const fn last_reload_time(&self) -> Option<u64> {
        self.last_reload_time
    }

    /// Increment the counter by the given amount.
    pub const fn increment(&mut self, amount: i64) {
        self.counter = self.counter.saturating_add(amount);
    }

    /// Decrement the counter by the given amount.
    pub const fn decrement(&mut self, amount: i64) {
        self.counter = self.counter.saturating_sub(amount);
    }

    /// Reset the counter to zero.
    pub const fn reset(&mut self) {
        self.counter = 0;
    }
}

impl Default for HotReloadDemoModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for HotReloadDemoModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("hot-reload-demo")
    }

    fn name(&self) -> &'static str {
        "Hot Reload Demo"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        if self.initial_load_time.is_none() {
            // Fresh load - set initial timestamp
            self.initial_load_time = Some(Self::now_unix());
        }
        self.initialized = true;

        tracing::info!(
            "HotReloadDemoModule initialized: counter={}, reloads={}",
            self.counter,
            self.reload_count
        );

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!(
            "HotReloadDemoModule exiting: counter={}, reloads={}",
            self.counter,
            self.reload_count
        );
        self.initialized = false;
        Ok(())
    }

    fn commands(&self) -> Vec<CommandRegistration> {
        vec![
            CommandRegistration::new("demo:increment")
                .with_name("Demo Increment")
                .with_description("Increment the demo counter")
                .with_category("demo")
                .with_count(),
            CommandRegistration::new("demo:decrement")
                .with_name("Demo Decrement")
                .with_description("Decrement the demo counter")
                .with_category("demo")
                .with_count(),
            CommandRegistration::new("demo:show")
                .with_name("Demo Show")
                .with_description("Show demo counter and statistics")
                .with_category("demo"),
            CommandRegistration::new("demo:reset")
                .with_name("Demo Reset")
                .with_description("Reset the demo counter to zero")
                .with_category("demo"),
        ]
    }

    // ========================================================================
    // Hot Reload Support
    // ========================================================================

    fn supports_hot_reload(&self) -> bool {
        true
    }

    fn save_state(&self) -> Option<Box<[u8]>> {
        // Binary format (24 bytes total):
        // - version: u32 (4 bytes)
        // - counter: i64 (8 bytes)
        // - reload_count: u32 (4 bytes)
        // - initial_load_time: u64 (8 bytes)

        let initial_time = self.initial_load_time.unwrap_or(0);

        let mut state = Vec::with_capacity(24);
        state.extend_from_slice(&STATE_VERSION.to_le_bytes());
        state.extend_from_slice(&self.counter.to_le_bytes());
        state.extend_from_slice(&self.reload_count.to_le_bytes());
        state.extend_from_slice(&initial_time.to_le_bytes());

        Some(state.into_boxed_slice())
    }

    fn restore_state(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        // Minimum size check
        if state.len() < 24 {
            return Err(ModuleError::InitFailed(format!(
                "state too short: {} bytes, expected 24",
                state.len()
            )));
        }

        // Parse version
        let version = u32::from_le_bytes(
            state[0..4]
                .try_into()
                .map_err(|_| ModuleError::InitFailed("failed to parse version".into()))?,
        );

        // Version compatibility check
        if version > STATE_VERSION {
            return Err(ModuleError::InitFailed(format!(
                "state version {version} is newer than current version {STATE_VERSION}",
            )));
        }

        // Parse fields
        let counter = i64::from_le_bytes(
            state[4..12]
                .try_into()
                .map_err(|_| ModuleError::InitFailed("failed to parse counter".into()))?,
        );

        let reload_count = u32::from_le_bytes(
            state[12..16]
                .try_into()
                .map_err(|_| ModuleError::InitFailed("failed to parse reload_count".into()))?,
        );

        let initial_load_time =
            u64::from_le_bytes(state[16..24].try_into().map_err(|_| {
                ModuleError::InitFailed("failed to parse initial_load_time".into())
            })?);

        // Restore state
        self.counter = counter;
        self.reload_count = reload_count.saturating_add(1); // Increment on reload
        self.initial_load_time = if initial_load_time > 0 {
            Some(initial_load_time)
        } else {
            None
        };
        self.last_reload_time = Some(Self::now_unix());

        tracing::info!(
            "HotReloadDemoModule state restored: counter={}, reloads={}",
            self.counter,
            self.reload_count
        );

        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(HotReloadDemoModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_module() {
        let module = HotReloadDemoModule::new();
        assert_eq!(module.counter(), 0);
        assert_eq!(module.reload_count(), 0);
        assert!(module.initial_load_time().is_none());
    }

    #[test]
    fn test_increment_decrement() {
        let mut module = HotReloadDemoModule::new();

        module.increment(5);
        assert_eq!(module.counter(), 5);

        module.increment(3);
        assert_eq!(module.counter(), 8);

        module.decrement(2);
        assert_eq!(module.counter(), 6);

        module.reset();
        assert_eq!(module.counter(), 0);
    }

    #[test]
    fn test_saturating_operations() {
        let mut module = HotReloadDemoModule::new();

        // Test overflow protection
        module.counter = i64::MAX;
        module.increment(1);
        assert_eq!(module.counter(), i64::MAX);

        module.counter = i64::MIN;
        module.decrement(1);
        assert_eq!(module.counter(), i64::MIN);
    }

    #[test]
    fn test_module_trait_impl() {
        let module = HotReloadDemoModule::new();
        assert_eq!(module.id().as_str(), "hot-reload-demo");
        assert_eq!(module.name(), "Hot Reload Demo");
        assert_eq!(module.version(), Version::new(0, 9, 0));
        assert!(module.supports_hot_reload());
    }

    #[test]
    fn test_save_state() {
        let mut module = HotReloadDemoModule::new();
        module.counter = 42;
        module.reload_count = 3;
        module.initial_load_time = Some(1_234_567_890);

        let state = module.save_state();
        assert!(state.is_some());

        let state = state.unwrap();
        assert_eq!(state.len(), 24);

        // Verify version
        let version = u32::from_le_bytes(state[0..4].try_into().unwrap());
        assert_eq!(version, STATE_VERSION);

        // Verify counter
        let counter = i64::from_le_bytes(state[4..12].try_into().unwrap());
        assert_eq!(counter, 42);
    }

    #[test]
    fn test_restore_state() {
        let mut original = HotReloadDemoModule::new();
        original.counter = 42;
        original.reload_count = 3;
        original.initial_load_time = Some(1_234_567_890);

        let state = original.save_state().unwrap();

        let mut restored = HotReloadDemoModule::new();
        let result = restored.restore_state(&state);

        assert!(result.is_ok());
        assert_eq!(restored.counter(), 42);
        assert_eq!(restored.reload_count(), 4); // Incremented on restore
        assert_eq!(restored.initial_load_time(), Some(1_234_567_890));
        assert!(restored.last_reload_time().is_some());
    }

    #[test]
    fn test_restore_state_version_too_new() {
        let mut module = HotReloadDemoModule::new();

        // Create state with version 99 (future)
        let mut state = vec![0u8; 24];
        state[0..4].copy_from_slice(&99u32.to_le_bytes());

        let result = module.restore_state(&state);
        assert!(result.is_err());

        if let Err(ModuleError::InitFailed(msg)) = result {
            assert!(msg.contains("newer"));
        }
    }

    #[test]
    fn test_restore_state_too_short() {
        let mut module = HotReloadDemoModule::new();

        let state = vec![0u8; 10]; // Too short
        let result = module.restore_state(&state);

        assert!(result.is_err());
        if let Err(ModuleError::InitFailed(msg)) = result {
            assert!(msg.contains("too short"));
        }
    }

    #[test]
    fn test_commands() {
        let module = HotReloadDemoModule::new();
        let commands = module.commands();

        assert_eq!(commands.len(), 4);

        let ids: Vec<&str> = commands.iter().map(|c| c.id).collect();
        assert!(ids.contains(&"demo:increment"));
        assert!(ids.contains(&"demo:decrement"));
        assert!(ids.contains(&"demo:show"));
        assert!(ids.contains(&"demo:reset"));
    }

    #[test]
    fn test_init_sets_initial_time() {
        let mut module = HotReloadDemoModule::new();
        assert!(module.initial_load_time().is_none());

        let ctx = ModuleContext::default();
        let result = module.init(&ctx);

        assert!(matches!(result, ProbeResult::Success));
        assert!(module.initial_load_time().is_some());
    }

    #[test]
    fn test_hot_reload_cycle() {
        // Simulate a full hot reload cycle

        // 1. Fresh load
        let mut module1 = HotReloadDemoModule::new();
        let ctx = ModuleContext::default();
        module1.init(&ctx);

        // 2. User modifies counter
        module1.increment(42);
        assert_eq!(module1.counter(), 42);

        // 3. Save state before unload
        let state = module1.save_state().unwrap();
        module1.exit().unwrap();

        // 4. Create new module instance (simulates reload)
        let mut module2 = HotReloadDemoModule::new();

        // 5. Restore state
        module2.restore_state(&state).unwrap();

        // 6. Init the restored module
        module2.init(&ctx);

        // 7. Verify state preserved
        assert_eq!(module2.counter(), 42);
        assert_eq!(module2.reload_count(), 1); // Was 0, now 1
        assert!(module2.initial_load_time().is_some());
        assert!(module2.last_reload_time().is_some());
    }
}
