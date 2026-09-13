//! Pending semantic operation, independent of transport and persistence.

use reovim_kernel_interface::ResourceId;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Operation {
    #[default]
    Idle,
    Attach(ResourceId),
    Retire {
        origin: u64,
    },
    Observe {
        origin: u64,
    },
    Context {
        origin: u64,
        reading: Option<(u64, u64)>,
    },
    Change(Change),
    /// Retire owned Resources before ending the Client's view lifetime.
    Closing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Change {
    Replace { origin: u64 },
    History { redo: bool },
}

impl Change {
    #[must_use]
    pub const fn origin(self) -> u64 {
        match self {
            Self::Replace { origin } => origin,
            Self::History { .. } => 0,
        }
    }
}
