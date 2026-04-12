//! Event type definitions for kernel and driver layers.
//!
//! Linux equivalent: Similar to `include/linux/notifier.h` event types
//!
//! This module organizes events by layer:
//!
//! - **Kernel events**: Domain-free substrate notifications (buffer/window lifecycle,
//!   mode changes, byte-level edits)
//! - **Driver events**: Hardware abstraction layer events (display/input)
//! - **Domain events**: Per-domain semantic events (in separate crates, e.g.,
//!   `reovim-domain-text-events`)
//!
//! # Design Philosophy
//!
//! Events are categorized by the three-layer event model:
//!
//! | Layer | Event Type | Examples |
//! |-------|-----------|----------|
//! | Kernel | Substrate | `BufferCreated`, `BufferBytesEdited`, `ModeChanged` |
//! | Domain | Semantic | `TextBufferModified`, `CursorMoved` (text-domain) |
//! | Driver | Hardware | `DisplayResized`, `KeyInput`, `MouseInput` |
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::events::{
//!     kernel::BufferCreated,
//!     driver::{DisplayResized, KeyInput, KeyCode, Modifiers},
//! };
//!
//! // Kernel event
//! let _ = BufferCreated { buffer_id: 1 };
//!
//! // Driver event
//! let _ = KeyInput {
//!     key: KeyCode::Char('a'),
//!     modifiers: Modifiers::NONE,
//! };
//! ```

pub mod driver;
pub mod kernel;
pub mod key;

// Re-export common types for convenience
pub use {
    driver::{
        DisplayResized, FrameRendered, KeyCode, KeyInput, Modifiers, MouseButton, MouseEvent,
        MouseInput,
    },
    kernel::{
        BufferBytesEdited, BufferClosed, BufferCreated, BufferSaved, BufferSwitched,
        BufferWillSave, ChangeSource, FileOpened, LayoutChangeKind, LayoutChanged, ModeChanged,
        OptionChanged, OptionReset, Shutdown, SplitDirection, WindowClosed, WindowCreated,
        WindowFocused, priority,
    },
    key::{ClientId, KeyPressEvent, SessionId},
};
