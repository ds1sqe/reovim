#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Session subsystem contracts for reovim.
//!
//! Provides pure-contract types and traits for session management.
//! This crate has no domain-specific dependencies — it depends only on
//! the kernel and subsys-layout.
//!
//! # Contents
//!
//! - **Types**: [`ClientId`], [`Viewport`], [`CursorSnapshot`], [`KeySequence`]
//! - **Extension system**: [`SessionExtension`], [`ExtensionMap`], [`TextInputSink`]
//! - **Mode lifecycle**: [`SessionMode`], [`ModeError`]
//! - **Empty session handling**: [`EmptySessionHandler`], [`EmptySessionContext`], [`EmptySessionAction`]
//! - **Handler system**: [`SessionHandlerKey`], [`SessionHandlerRegistry`]
//! - **Initial mode provider**: [`InitialModeProvider`]
//! - **Leader key provider**: [`LeaderKeyProvider`], [`expand_leader`]
//! - **Notification queue**: [`PendingNotificationQueue`] and related types
//! - **Stale check**: [`StaleCheck`]
//! - **Tick scheduler**: [`TickScheduler`], [`TickSchedulerHandle`]
//! - **API traits**: [`ClipboardApi`], [`CompositorApi`], [`ExtensionApi`], [`FindCharState`]
//! - **Bridge system**: [`BridgeRegistry`], [`BridgeProvider`], [`ExtensionStateBridge`]

pub mod api;
pub mod bridges;
mod empty_handler;
mod extension;
mod handler_key;
mod handler_registry;
mod initial_mode;
mod leader_key;
mod mode;
mod notification_queue;
mod stale_check;
pub mod tick;
mod types;

// Types
pub use types::{ClientId, CursorSnapshot, KeySequence, Viewport};

// Extension system
pub use extension::{ExtensionMap, SessionExtension, SessionExtensionDyn, TextInputSink};

// Mode lifecycle
pub use mode::{ModeError, SessionMode};

// Empty session handling
pub use empty_handler::{EmptySessionAction, EmptySessionContext, EmptySessionHandler};

// Handler system
pub use {handler_key::SessionHandlerKey, handler_registry::SessionHandlerRegistry};

// Initial mode provider
pub use initial_mode::InitialModeProvider;

// Leader key provider
pub use leader_key::{LeaderKeyProvider, expand_leader};

// Notification queue
pub use notification_queue::{
    PendingEntry, PendingLevel, PendingNotification, PendingNotificationQueue, PendingOp,
};

// Stale check
pub use stale_check::StaleCheck;

// Tick scheduler
pub use tick::{TickScheduler, TickSchedulerHandle};

// API traits
pub use api::{
    ClipboardApi, CompositorApi, CompositorError, ExtensionApi, FindCharRecord, FindCharState,
};

// Bridge system
pub use bridges::{
    BridgeContext, BridgeProvider, BridgeRegistry, ExtensionScope, ExtensionStateBridge,
};
