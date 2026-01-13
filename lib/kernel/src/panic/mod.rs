//! Panic handling subsystem.
//!
//! Linux equivalent: `kernel/panic.c`
//!
//! Provides custom panic handlers, crash recovery, and state dumping.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     Panic Subsystem                         │
//! │                                                             │
//! │  ┌─────────────────┐  ┌─────────────────┐                   │
//! │  │    Handler      │  │    Recovery     │                   │
//! │  │ (panic hook,    │  │ (buffer save,   │                   │
//! │  │  recovery cb)   │  │  file mgmt)     │                   │
//! │  └─────────────────┘  └─────────────────┘                   │
//! │                                                             │
//! │  ┌─────────────────┐                                        │
//! │  │    Report       │                                        │
//! │  │ (crash report   │                                        │
//! │  │  generation)    │                                        │
//! │  └─────────────────┘                                        │
//! │                                                             │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Components
//!
//! - [`handler`] - Custom panic handler installation
//! - [`recovery`] - Buffer recovery utilities
//! - [`report`] - Crash report generation
//!
//! # Example
//!
//! ```ignore
//! use reovim_kernel::panic::{
//!     install_panic_handler,
//!     set_recovery_callback,
//!     save_buffer_for_recovery,
//! };
//!
//! // Set up recovery callback
//! set_recovery_callback(Box::new(|_info| {
//!     // Save unsaved buffers
//!     for buffer in get_unsaved_buffers() {
//!         save_buffer_for_recovery(
//!             buffer.id,
//!             buffer.path.as_deref(),
//!             &buffer.content,
//!         ).ok();
//!     }
//! }));
//!
//! // Install the panic handler
//! install_panic_handler();
//! ```

mod handler;
mod recovery;
mod report;

// Re-export handler types
#[allow(unused_imports)]
pub use handler::{install_panic_handler, is_handler_installed, set_recovery_callback};

// Re-export recovery types
#[allow(unused_imports)]
pub use recovery::{
    RecoverySnapshot, UnsavedBuffer, cleanup_old_recovery_files, list_recovery_files, recovery_dir,
    save_buffer_for_recovery,
};

// Re-export report types
#[allow(unused_imports)]
pub use report::{CrashReport, generate_crash_report};
