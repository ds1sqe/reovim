#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Reovim TUI platform runtime.
//!
//! Hosts the TUI platform-core machinery: event loop, gRPC notification
//! fan-in, render/input/state machinery, and the `run(args)` entry
//! point wired by the standalone `reovim-tui` bin and any in-process
//! launcher.
//!
//! File re-exports sit over `reovim-client-tui`; the platform-owned
//! `run`/`logging` modules are the locked CLM v7 surface.

#[cfg(feature = "grpc")]
pub mod logging;

#[cfg(feature = "grpc")]
pub mod run;

#[cfg(feature = "grpc")]
pub use run::{TuiArgs, TuiRunError, run};

#[cfg(all(test, feature = "grpc"))]
#[path = "run_tests.rs"]
mod run_tests;

pub use reovim_client_tui::{
    CliHistoryEntry, CliPanelState, CliResult, ClientRole, CursorPosition, CursorStyleHint,
    HeadlessOutput, InputHandler, LevelColor, LineNumberMode, LogPanelState, RemoteClient,
    RenderBackend, RenderConfig, RenderState, SelectionState, TerminalOutput, TuiCoreState,
    TuiDebugConfig, TuiHandle, TuiHandleError, TuiLogBuffer, TuiLogEntry, TuiOutput,
    build_frame_content, format_entry, format_frame_buffer, render_frame, render_panel,
};

#[cfg(feature = "grpc")]
pub use reovim_client_tui::{
    TuiApp, TuiAppError, TuiGrpcClient, TuiGrpcError, connect_headless, connect_interactive,
};
