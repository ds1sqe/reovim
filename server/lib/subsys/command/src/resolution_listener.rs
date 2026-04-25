//! Listener hook fired by [`crate::CommandNameIndex`] before every
//! resolution attempt.
//!
//! The lazy-load runtime registers a concrete listener that observes
//! command-name resolution and dlopens packages whose `pkg.lock`
//! `on-event = "<name>"` trigger matches. Listeners may not branch
//! resolution and may not feed back into the same call's lookup —
//! commands registered by a listener become visible on the *next*
//! resolution cycle.
//!
//! The trait surface stays mechanism-only — no `LazyRegistry` types
//! and no result type — so the subsys tier never imports the
//! lazy-load layer.

/// Side-effect hook invoked before [`crate::CommandNameIndex`]
/// performs its `HashMap` lookup.
///
/// Implementations must be quick (this is the command-dispatch hot
/// path) and Send + Sync (listeners are stored behind an `Arc`).
pub trait CommandResolutionListener: Send + Sync {
    /// Called immediately before the index attempts to resolve
    /// `name`. The argument is the user-typed string for `resolve` /
    /// `resolve_entry`, or the prefix being searched for
    /// `resolve_prefix`. Listeners may inspect or react but cannot
    /// influence the lookup result on this call.
    fn on_resolve(&self, name: &str);
}
