//! Proto boundary transpilation — the single crossing point.
//!
//! `from_proto()` converts a `ProjectionUpdatedPayload` into a
//! `DomainProjection`. After this function, no proto types exist in the
//! caller's scope. This is the D21 design: fail loud on invalid input.

use {
    reovim_protocol::v3::ProjectionUpdatedPayload,
    reovim_subsys_coordination::{DomainId, ProjectionTag},
};

use super::{DomainProjection, TranspileError};

/// Convert a proto `ProjectionUpdatedPayload` to a `DomainProjection`.
///
/// This is the **single proto boundary crossing** for projection data.
/// After this call, the caller works exclusively with newtypes.
///
/// # Panics
///
/// Panics if `payload.datum` is `None`. Per D21, a missing datum is a
/// server contract violation — fail loud, not `unwrap_or_default`.
#[must_use]
pub fn from_proto(payload: ProjectionUpdatedPayload) -> DomainProjection {
    match try_from_proto(payload) {
        Ok(proj) => proj,
        Err(TranspileError::MissingDatum) => {
            panic!(
                "BUG: ProjectionUpdatedPayload missing required datum — server contract violation (D21)"
            )
        }
    }
}

/// Fallible conversion for contexts where panicking is inappropriate.
///
/// Returns `Err(TranspileError::MissingDatum)` instead of panicking.
pub fn try_from_proto(
    payload: ProjectionUpdatedPayload,
) -> Result<DomainProjection, TranspileError> {
    let datum = payload.datum.ok_or(TranspileError::MissingDatum)?;

    Ok(DomainProjection {
        tag: ProjectionTag::new(&payload.tag),
        domain_id: DomainId(payload.domain_id),
        window_id: payload.window_id.map(|id| id as usize),
        content: datum.content,
        display: datum.display.unwrap_or_default(),
        transient: payload.transient,
        version: payload.version,
        client_id: payload.client_id,
    })
}
