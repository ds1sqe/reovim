//! Why a Client call did not happen or did not finish.

use core::fmt;

use reovim_kernel_interface::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Fault {
    /// The Kernel or a Driver refused.
    Kernel(Error),
    /// The link is gone.
    Carrier,
    /// Bytes that are not a message, or a reply of the wrong shape.
    Wire,
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kernel(error) => write!(f, "{}", word(*error)),
            Self::Carrier => write!(f, "link gone"),
            Self::Wire => write!(f, "not a message"),
        }
    }
}

/// The Kernel's error as a word, not its `Debug` name; whoever prints
/// it puts the context around it.
const fn word(error: Error) -> &'static str {
    match error {
        Error::Denied => "denied",
        Error::Unsupported => "unsupported",
        Error::Invalid => "invalid",
        Error::Stale => "stale",
        Error::Busy => "busy",
        Error::NoSpace => "no space",
        Error::Closed => "closed",
        Error::Gone => "gone",
        Error::Cancelled => "cancelled",
    }
}

impl From<Error> for Fault {
    fn from(error: Error) -> Self {
        Self::Kernel(error)
    }
}
