//! Which-key related registry types (#457).
//!
//! Types for which-key integration that need to be accessible from both
//! bootstrap and input handler.

use {reovim_kernel::api::v1::Service, reovim_module_which_key::FilterRequest, tokio::sync::mpsc};

/// Wrapper for saturator sender to register in `ServiceRegistry`.
///
/// Provides access to the sender channel for sending filter requests
/// to the which-key saturator background task.
pub struct WhichKeySaturatorSender(mpsc::Sender<FilterRequest>);

impl WhichKeySaturatorSender {
    /// Create a new sender wrapper.
    #[must_use]
    pub const fn new(sender: mpsc::Sender<FilterRequest>) -> Self {
        Self(sender)
    }

    /// Get a clone of the sender for sending filter requests.
    #[must_use]
    pub fn clone_sender(&self) -> mpsc::Sender<FilterRequest> {
        self.0.clone()
    }
}

impl Service for WhichKeySaturatorSender {}
