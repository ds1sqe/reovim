#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Notification module - POLICY layer.
//!
//! Provides toast notification state and bridge for server-to-client
//! notification streaming. Other modules push notifications via
//! `ext_mut::<NotificationState>().push(level, title)`.
//!
//! # Architecture (#443)
//!
//! This module reads [`NotificationState`] from the client's `ExtensionMap`
//! -- a generic session extension populated by any module that wants to
//! emit user-facing notifications. It has **zero dependency on vim** and
//! works with any editor module that uses the `ExtensionApi`.
//!
//! The bridge is registered via [`BridgeProvider`] during `init()`.

mod bridge;
mod drain;
mod state;

pub use {
    bridge::NotificationBridge,
    state::{Notification, NotificationLevel, NotificationState, Progress},
};

use {
    reovim_driver_session::{NotificationDrainRegistry, bridges::BridgeProvider},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    std::sync::Arc,
};

const KIND: &str = "notification";
const MODULE_ID: ModuleId = ModuleId::new("notification");

/// Notification module.
///
/// Registers [`NotificationBridge`] via [`BridgeProvider`] during `init()`.
pub struct NotificationModule;

impl NotificationModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for NotificationModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for NotificationModule {
    fn id(&self) -> ModuleId {
        MODULE_ID
    }

    fn name(&self) -> &'static str {
        "notification"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(NotificationBridge);

        // Register NotificationDrain implementation (#542: decouple completion from this module).
        let drain_registry = ctx.services.get_or_create::<NotificationDrainRegistry>();
        drain_registry.register(Arc::new(drain::NotificationDrainImpl));

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[KIND]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(NotificationModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
