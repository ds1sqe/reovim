//! `[lazy]` table entries.
//!
//! Each entry names a dependency and specifies exactly one load
//! trigger: `on-domain`, `on-event`, `on-capability`, or
//! `eager = true`. The trigger gates `dlopen` of the dependency's
//! cdylib.

use serde::{Deserialize, Serialize};

use crate::ManifestError;

/// A lazy-load trigger for a dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LazyTrigger {
    /// Load when a domain with this name is opened.
    OnDomain(String),
    /// Load when an event with this name fires.
    OnEvent(String),
    /// Load when a capability with this name is requested.
    OnCapability(String),
    /// Load eagerly at startup. Serialized as `eager = true`.
    Eager,
}

/// Intermediate TOML representation: zero or more trigger fields
/// set, exactly one of which must be populated for a valid entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RawLazy {
    #[serde(default, rename = "on-domain", skip_serializing_if = "Option::is_none")]
    pub on_domain: Option<String>,
    #[serde(default, rename = "on-event", skip_serializing_if = "Option::is_none")]
    pub on_event: Option<String>,
    #[serde(
        default,
        rename = "on-capability",
        skip_serializing_if = "Option::is_none"
    )]
    pub on_capability: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eager: Option<bool>,
}

impl RawLazy {
    /// Convert a raw entry into a [`LazyTrigger`].
    ///
    /// Fails with [`ManifestError::InvalidLazyTrigger`] if zero or
    /// more than one trigger field is set, or if `eager` is set to
    /// `false`.
    pub fn into_trigger(self, dep: &str) -> Result<LazyTrigger, ManifestError> {
        match (self.on_domain, self.on_event, self.on_capability, self.eager) {
            (Some(d), None, None, None) => Ok(LazyTrigger::OnDomain(d)),
            (None, Some(e), None, None) => Ok(LazyTrigger::OnEvent(e)),
            (None, None, Some(c), None) => Ok(LazyTrigger::OnCapability(c)),
            (None, None, None, Some(true)) => Ok(LazyTrigger::Eager),
            _ => Err(ManifestError::InvalidLazyTrigger {
                dep: dep.to_string(),
            }),
        }
    }
}

impl From<&LazyTrigger> for RawLazy {
    fn from(trigger: &LazyTrigger) -> Self {
        let mut raw = Self::default();
        match trigger {
            LazyTrigger::OnDomain(s) => raw.on_domain = Some(s.clone()),
            LazyTrigger::OnEvent(s) => raw.on_event = Some(s.clone()),
            LazyTrigger::OnCapability(s) => raw.on_capability = Some(s.clone()),
            LazyTrigger::Eager => raw.eager = Some(true),
        }
        raw
    }
}
