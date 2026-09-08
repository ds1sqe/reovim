use core::fmt;

use reovim_kernel_interface::resource::state::ResourceStateError;

/// Rejection before a Buffer transition becomes authoritative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferError {
    /// Exact native preparation, state, model or installation fence failed.
    State(ResourceStateError),
    /// Node/native-state reservation or allocation capacity was exhausted.
    Capacity,
    /// A fresh Buffer version or opaque node identity cannot be allocated.
    IdentityExhausted,
    /// Assembly supplied an unusable Buffer reservation or source schema.
    InvalidConfiguration,
}

impl fmt::Display for BufferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Buffer acceptance: {self:?}")
    }
}

impl core::error::Error for BufferError {}

/// Rejection retains all input ownership for retirement outside authority.
pub struct Rejected<Value> {
    /// Why no state or History transition was accepted.
    pub cause: BufferError,
    /// The unchanged owned candidate or staged edit.
    pub value: Value,
}

impl<Value> fmt::Debug for Rejected<Value> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Rejected")
            .field("cause", &self.cause)
            .finish_non_exhaustive()
    }
}
