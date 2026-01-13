//! Inter-process communication subsystem.
//!
//! Linux equivalent: `ipc/`
//!
//! This module provides the event system for kernel-driver-module communication.
//! It includes:
//!
//! - **Event Bus**: Type-erased pub/sub event dispatch
//! - **`EventScope`**: GC-like synchronization for event lifecycle tracking
//! - **Channels**: MPSC and oneshot channels for direct communication
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                        EventBus                              │
//! │                                                              │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐   │
//! │  │ Subscription │    │ Subscription │    │ Subscription │   │
//! │  │   Handler A  │    │   Handler B  │    │   Handler C  │   │
//! │  └──────────────┘    └──────────────┘    └──────────────┘   │
//! │         │                   │                   │           │
//! │         └───────────────────┼───────────────────┘           │
//! │                             │                                │
//! │                    ┌────────┴────────┐                       │
//! │                    │    DynEvent     │                       │
//! │                    │  (type-erased)  │                       │
//! │                    └────────┬────────┘                       │
//! │                             │                                │
//! │                    ┌────────┴────────┐                       │
//! │                    │   EventScope    │                       │
//! │                    │ (lifecycle sync)│                       │
//! │                    └─────────────────┘                       │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Design Philosophy
//!
//! Following Linux kernel "mechanism, not policy":
//! - Provides communication primitives
//! - No opinion on message routing strategy
//! - Runtime integration is left to higher layers
//!
//! # Performance Characteristics
//!
//! - **Lock-free dispatch**: Handler lookup uses `ArcSwap` for O(1) reads
//! - **RCU subscriptions**: Copy-on-write prevents blocking dispatch
//! - **Fire-and-forget**: No error propagation from handlers
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//!
//! // Define an event
//! #[derive(Debug)]
//! struct BufferChanged { buffer_id: u64 }
//! impl Event for BufferChanged {}
//!
//! // Create event bus
//! let bus = EventBus::new();
//!
//! // Subscribe handler
//! let _sub = bus.subscribe::<BufferChanged, _>(100, |event| {
//!     println!("Buffer {} changed", event.buffer_id);
//!     EventResult::Handled
//! });
//!
//! // Emit event
//! bus.emit(BufferChanged { buffer_id: 1 });
//!
//! // With scope tracking
//! let scope = EventScope::new();
//! scope.increment();
//! bus.emit_scoped(BufferChanged { buffer_id: 2 }, &scope);
//! assert!(scope.is_complete());
//! ```

mod channel;
mod event;
mod event_bus;
mod scope;
mod subscription;

// Re-export channel types
pub use channel::{
    BoundedReceiver, BoundedSender, OneshotReceiver, OneshotSender, Receiver, RecvError, SendError,
    Sender, TryRecvError, TrySendError, bounded, channel, oneshot,
};

// Re-export event types
pub use event::{CacheKind, CacheUpdated, DynEvent, Event, EventResult};

// Re-export event bus
pub use event_bus::EventBus;

// Re-export scope types
pub use scope::{DEFAULT_TIMEOUT, EventScope, ScopeId};

// Re-export subscription types
pub use subscription::{Subscription, SubscriptionId};
