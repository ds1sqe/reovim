//! Why a Client call did not happen or did not finish.

use alloc::string::String;
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
    /// The input was refused before anything was sent.
    Usage(String),
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kernel(error) => write!(f, "{error:?}"),
            Self::Carrier => write!(f, "link gone"),
            Self::Wire => write!(f, "not a message"),
            Self::Usage(text) => write!(f, "{text}"),
        }
    }
}

impl From<Error> for Fault {
    fn from(error: Error) -> Self {
        Self::Kernel(error)
    }
}
