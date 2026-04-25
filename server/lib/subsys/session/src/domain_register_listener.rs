//! Listener hook fired by the runtime each time a [`DomainDriver`] is
//! installed on a session.
//!
//! The lazy-load runtime registers a concrete listener that observes
//! domain registrations and dlopens packages whose `pkg.lock`
//! `on-domain = "<name>"` trigger matches. Listeners may not branch
//! the registration itself.
//!
//! [`DomainDriver`]: crate::DomainDriver

/// Side-effect hook invoked when a domain driver is installed.
///
/// Implementations must be `Send + Sync` (listeners are stored behind
/// an `Arc`) and quick — they run on the writer-lock release path of
/// every `set_domain_driver` call.
pub trait DomainRegisterListener: Send + Sync {
    /// Called every time a domain driver is registered for a session.
    /// `domain_name` is the canonical name returned by the driver
    /// (e.g. `"text"`, `"lsp"`).
    fn on_register(&self, domain_name: &str);
}
