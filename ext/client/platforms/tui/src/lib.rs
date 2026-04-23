#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Reovim TUI platform runtime.
//!
//! Hosts the TUI platform-core machinery: event loop, gRPC notification
//! fan-in, render/input/state machinery. Exposes `run(args)` as the
//! single entry point dispatched from `apps/bin/reovim`.
//!
//! This crate is currently a transitional re-export shim over
//! `reovim-client-tui`. Source files migrate in one cluster at a time
//! so the workspace stays green at every commit boundary.

pub use reovim_client_tui::{
    CliHistoryEntry, CliPanelState, CliResult, ClientRole, CursorPosition, CursorStyleHint,
    HeadlessOutput, InputHandler, LevelColor, LineNumberMode, LogPanelState, RemoteClient,
    RenderBackend, RenderConfig, RenderState, SelectionState, TerminalOutput, TuiArgs,
    TuiCoreState, TuiDebugConfig, TuiHandle, TuiHandleError, TuiLogBuffer, TuiLogEntry, TuiOutput,
    build_frame_content, format_entry, format_frame_buffer, render_frame, render_panel,
};

#[cfg(feature = "grpc")]
pub use reovim_client_tui::{
    TuiApp, TuiAppError, TuiGrpcClient, TuiGrpcError, connect_headless, connect_interactive,
};
