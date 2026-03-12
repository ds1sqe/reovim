//! gRPC authentication interceptor (#483).
//!
//! Reads `x-reovim-token` from request metadata, resolves it to a
//! `ClientId` via the [`TokenRegistry`], and injects it into request
//! extensions. Handlers then extract `ClientId` from extensions
//! instead of trusting the self-reported field in the request body.
//!
//! # Phase 5: Token-Only Authentication
//!
//! Token authentication is now **mandatory** for all RPCs except `Join()`.
//! Two resolution functions are provided:
//!
//! - [`require_client_id`] — for caller-identity RPCs (`send_keys`, leave, etc.)
//! - [`resolve_target_client_id`] — for state queries where the body field
//!   selects which client's state to return (0 = self via token)

use std::sync::Arc;

use tonic::{Request, Status, service::Interceptor};

use crate::session::{ClientId, SessionToken, TokenRegistry};

/// gRPC interceptor that resolves session tokens to client IDs.
///
/// Inserted into every gRPC service via `with_interceptor`. On each
/// request:
///
/// 1. Reads `x-reovim-token` from metadata
/// 2. Looks up token in [`TokenRegistry`] (read lock, O(1))
/// 3. If found, inserts `ClientId` into request extensions
/// 4. If not found, passes through (Phase 1 backward compat)
#[derive(Clone)]
pub struct AuthInterceptor {
    tokens: Arc<TokenRegistry>,
}

impl AuthInterceptor {
    /// Create a new interceptor backed by the given token registry.
    #[must_use]
    pub const fn new(tokens: Arc<TokenRegistry>) -> Self {
        Self { tokens }
    }
}

impl Interceptor for AuthInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        if let Some(token_str) = request
            .metadata()
            .get("x-reovim-token")
            .and_then(|v| v.to_str().ok())
        {
            let token = SessionToken::from(token_str);
            if let Some(client_id) = self.tokens.resolve(&token) {
                request.extensions_mut().insert(client_id);
            }
        }
        Ok(request)
    }
}

/// Require token-authenticated `ClientId` (#483 Phase 5).
///
/// Used for **caller-identity** RPCs where the token is the sole source
/// of identity (`send_keys`, leave, `update_presence`, `set_sync_mode`, etc.).
///
/// # Errors
///
/// Returns `Unauthenticated` if no token was provided in the request.
// `Status` is tonic's standard error type - size is inherent to the library
#[allow(clippy::result_large_err)]
pub fn require_client_id(token_client_id: Option<ClientId>) -> Result<ClientId, Status> {
    token_client_id
        .ok_or_else(|| Status::unauthenticated("session token required — call Join() first"))
}

/// Resolve target client ID for state queries (#483 Phase 5).
///
/// Used for **targeting** RPCs where the caller wants another client's
/// state (`get_mode`, `get_cursor`, `get_layout`, etc.).
///
/// The token authenticates the **caller**. The body `target_client_id`
/// selects **whose** state to return:
/// - `0` → return the caller's own state (uses token identity)
/// - `>0` → return that specific client's state
///
/// # Errors
///
/// Returns `Unauthenticated` if no token was provided in the request.
// `Status` is tonic's standard error type - size is inherent to the library
#[allow(clippy::result_large_err)]
#[allow(clippy::cast_possible_truncation)]
pub fn resolve_target_client_id(
    token_client_id: Option<ClientId>,
    target_client_id: u64,
) -> Result<ClientId, Status> {
    let caller = require_client_id(token_client_id)?;
    // target=0 means "self" — return caller's own state
    if target_client_id == 0 {
        return Ok(caller);
    }
    // target>0 means a specific client's state
    Ok(ClientId::new(target_client_id as usize))
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
