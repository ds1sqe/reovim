#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Git provider driver for reovim.
//!
//! This driver defines the interface for git data access.
//! Following the mechanism/policy separation:
//!
//! - **Mechanism** (this driver): [`GitProvider`] trait, typed data structs,
//!   [`GitProviderStore`] for service discovery
//! - **Policy** (modules): Implementations such as subprocess-based git
//!   access or library-based (libgit2)
//!
//! # Architecture (#530)
//!
//! Cross-cutting git data (branches, status, log, diff hunks) is provided
//! once at the driver layer and consumed by multiple modules (pickers,
//! statusline, gutter signs, blame) through `ServiceRegistry`.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │  Consumers (pickers, statusline, gutter, blame)         │
//! │  → services.get::<GitProviderStore>()?.get()?           │
//! ├─────────────────────────────────────────────────────────┤
//! │  ServiceRegistry                                        │
//! │  → GitProviderStore holds Arc<dyn GitProvider>          │
//! ├─────────────────────────────────────────────────────────┤
//! │  Driver (this crate)                                    │
//! │  → GitProvider trait, typed data structs                │
//! ├─────────────────────────────────────────────────────────┤
//! │  Module (reovim-module-git)                             │
//! │  → SubprocessGitProvider implementation                 │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_git::{GitProviderStore, GitProvider};
//!
//! // In module init():
//! let store = ctx.services.get_or_create::<GitProviderStore>();
//! store.register(Arc::new(SubprocessGitProvider));
//!
//! // In any consumer:
//! let store = ctx.services.get::<GitProviderStore>()?;
//! let git = store.get()?;
//! let branches = git.branches(&cwd);
//! ```

mod provider;
mod store;
pub mod types;

pub use {provider::GitProvider, store::GitProviderStore};

#[cfg(test)]
#[path = "provider_tests.rs"]
mod provider_tests;

#[cfg(test)]
#[path = "store_tests.rs"]
mod store_tests;

#[cfg(test)]
#[path = "types_tests.rs"]
mod types_tests;
