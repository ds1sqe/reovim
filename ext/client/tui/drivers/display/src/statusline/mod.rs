//! Statusline mechanism layer.
//!
//! Defines traits and types for statusline rendering. Policy modules
//! implement these traits to provide actual content.
//!
//! # Architecture
//!
//! This module follows the kernel's "mechanism vs policy" principle:
//! - **Mechanism** (this module): Defines WHAT can be done
//!   - Section types and layout
//!   - Component trait and context
//!   - Rendering primitives
//! - **Policy** (ext/server/modules/statusline): Defines HOW things behave
//!   - Default configuration
//!   - Built-in components
//!   - Theme/color choices
//!
//! # Components
//!
//! - [`SectionId`] - Lualine-style section identifiers (A, B, C, X, Y, Z)
//! - [`Section`] - A section with rendered component outputs
//! - [`ComponentContext`] - Editor state snapshot for component rendering
//! - [`ComponentProvider`] - Trait for pluggable components
//! - [`StatuslineProvider`] - Trait for statusline content providers

mod component;
mod height;
mod layout;
mod provider;
mod renderer;
mod section;

pub use {
    component::{
        ComponentContext, ComponentData, ComponentDataContext, ComponentDataProvider,
        ComponentDataProviderKey, ComponentDataProviderRegistry, ComponentOutput,
        ComponentProvider, ComponentProviderKey, ComponentProviderRegistry, DataProviderAdapter,
        DiagnosticCounts,
    },
    height::{
        ContentMetrics, HeightConfig, HeightResult, OverflowStrategy, ScreenThreshold,
        calculate_height, calculate_height_from_sections, calculate_height_with_metrics,
    },
    layout::{LayoutCalculator, MultiRowLayout, RowContent},
    provider::{StatuslineProvider, StatuslineProviderKey, StatuslineProviderRegistry},
    renderer::{
        StatuslineRendererConfig, StatuslineSeparator, render_sections, render_statusline_simple,
        truncate_sections,
    },
    section::{Section, SectionId, SectionPosition},
};
