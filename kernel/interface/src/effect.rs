//! Effect knowledge retained across external mutation boundaries.

/// Knowledge of whether one declared target effect occurred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectCertainty {
    /// The declared target effect is confirmed.
    Applied,
    /// The target is proved unchanged by the attempted operation.
    DefinitelyNotApplied,
    /// The effect may have occurred and requires reconciliation.
    OutcomeUnknown,
}
