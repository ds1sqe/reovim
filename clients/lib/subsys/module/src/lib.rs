#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! `reovim-client-subsys-module` — Client module ABI contract crate.
//!
//! Provides the `ClientModule` trait family, `ClientServiceRegistry`,
//! `ClientModuleLoader`, and all FFI boundary types for dynamic client modules.
//! This crate owns the ABI; `reovim-client-driver` provides compat re-exports
//! that point here.
//!
//! # Feature flags
//!
//! * `testing` — enables `pub mod testing` with mock types for downstream
//!   test code (capsules surface, capabilities, server, theme, and registry
//!   mocks). Use `features = ["testing"]` in dev-dependencies.

pub mod ffi;
pub mod loader;
pub mod services;
pub mod traits;
pub mod types;

// Top-level re-exports for ergonomic use by downstream crates.
pub use types::{
    AnnotationContext, Attributes, BufferId, BufferUpdateEvent, CLIENT_MODULE_API_VERSION,
    ChromePosition, ClientModuleError, ClientModuleProbe, Color, ColorDepth, ColumnWidth,
    CursorInfo, DomainProjection, FocusEvent, GutterCell, InlineDecoration, Insets, KeyCode,
    KeyEvent, LineNumberMode, Modifiers, OptionKind, OptionMetadata, OptionValue, PlatformEvent,
    PointerButton, PointerEvent, PointerKind, ProbeResult, Rect, RemoteClientInfo, RenderBehavior,
    RenderingModel, SelectionInfo, SelectionMode, Style, SyntaxToken, TouchEvent, TouchKind,
    TransformedLine, Version, ViewportContext, VirtualLine, VirtualLinePosition, WindowId,
    WindowLayout, is_client_compatible,
};

pub use traits::{
    ChromeSurface, ClientModule, ClientModuleRegistry, LayoutPolicy, LocalInputResult,
    ModuleContext, PlatformCapabilities, ServerHandle, ThemeProvider, TokenProvider,
    ViewportRenderer,
};

pub use services::ClientServiceRegistry;

pub use loader::{
    ClientModuleFactory, ClientModuleLoader, ClientModuleLoaderError, ClientModuleState,
};

#[cfg(any(test, feature = "testing"))]
pub mod testing;
