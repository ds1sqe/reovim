//! Mode provider registry.

use reovim_kernel::api::v1::MultiServiceRegistry;

use crate::{DefaultModeProvider, mode_key::ModeProviderKey};

/// Registry for mode providers, keyed by purpose.
pub type ModeProviderRegistry = MultiServiceRegistry<ModeProviderKey, dyn DefaultModeProvider>;
