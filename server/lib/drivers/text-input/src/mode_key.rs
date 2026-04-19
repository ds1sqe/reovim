//! Mode provider key - typed key for mode provider lookup.

use reovim_kernel::api::v1::ServiceKey;

/// Typed key for mode provider lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModeProviderKey {
    /// Entry/startup mode provider.
    Entry,
}

impl ServiceKey for ModeProviderKey {
    fn service_name() -> &'static str {
        "Mode"
    }
}
