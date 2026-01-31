//! Event type definitions for kernel and driver layers.
//!
//! Linux equivalent: Similar to `include/linux/notifier.h` event types
//!
//! This module organizes events by layer:
//!
//! - **Kernel events**: Pure mechanism-level notifications (buffer/window/cursor changes)
//! - **Driver events**: Hardware abstraction layer events (display/input)
//!
//! # Design Philosophy
//!
//! Events are categorized by the "mechanism vs policy" principle:
//!
//! | Layer | Event Type | Examples |
//! |-------|-----------|----------|
//! | Kernel | Notification | `BufferCreated`, `CursorMoved`, `ModeChanged` |
//! | Driver | Hardware | `DisplayResized`, `KeyInput`, `MouseInput` |
//! | Core/Plugin | Request/Policy | `RequestOpenFile`, `RequestSetTheme` |
//!
//! Request events (policy) stay in `lib/core` or plugins, not in the kernel.
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::events::{
//!     kernel::{BufferCreated, BufferModified, Modification},
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
        BufferClosed, BufferCreated, BufferModified, BufferSaved, BufferSwitched, ChangeSource,
        CursorMoved, FileOpened, FileTypeChanged, LayoutChangeKind, LayoutChanged, ModeChanged,
        Modification, OptionChanged, OptionReset, Shutdown, SplitDirection, ViewportScrolled,
        WindowClosed, WindowCreated, WindowFocused, priority,
    },
    key::{ClientId, KeyPressEvent, SessionId},
};
