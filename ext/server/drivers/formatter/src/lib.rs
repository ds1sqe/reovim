#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Formatter provider driver for reovim.
//!
//! Defines the [`FormatterProvider`] trait contract and [`ExternalFormatter`]
//! implementation for running CLI formatters. This is pure mechanism — policy
//! (format-on-save, formatter resolution) belongs in `reovim-module-format`.

mod config;
mod error;
mod external;
mod provider;
mod registry;

pub use {
    config::FormatterConfig, error::FormatError, external::ExternalFormatter,
    provider::FormatterProvider, registry::FormatterRegistry,
};
