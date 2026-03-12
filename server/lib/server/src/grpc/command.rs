//! `CommandService` gRPC implementation (#453).
//!
//! Provides command discovery and argument completion for cmdline UI.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_driver_command::{CommandNameIndex, CommandQueryService},
    reovim_protocol::v2::{
        CommandSource, CompleteArgsRequest, CompleteArgsResponse, KeybindingCommandEntry,
        SearchCommandsRequest, SearchCommandsResponse, UserCommandEntry,
        command_service_server::CommandService,
    },
    tonic::{Request, Response, Status},
};

use crate::{
    CommandQuerySnapshot,
    session::{Session, SessionId, SessionRegistry},
};

/// gRPC `CommandService` implementation.
///
/// Bridges command query requests to the driver-layer query services.
pub struct CommandServiceImpl {
    sessions: Arc<SessionRegistry>,
    default_session_id: SessionId,
}

impl CommandServiceImpl {
    /// Create a new `CommandService`.
    #[must_use]
    pub const fn new(sessions: Arc<SessionRegistry>, default_session_id: SessionId) -> Self {
        Self {
            sessions,
            default_session_id,
        }
    }

    /// Get the default session.
    fn get_session(&self) -> Result<Arc<Session>, Status> {
        self.sessions
            .get(&self.default_session_id)
            .ok_or_else(|| Status::not_found("No active session"))
    }
}

#[tonic::async_trait]
impl CommandService for CommandServiceImpl {
    async fn search_commands(
        &self,
        request: Request<SearchCommandsRequest>,
    ) -> Result<Response<SearchCommandsResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;
        let source = CommandSource::try_from(req.source).unwrap_or(CommandSource::All);

        let response = session.with_state_sync(|state| {
            let services = &state.app.kernel.services;

            let mut resp = SearchCommandsResponse {
                user_commands: Vec::new(),
                keybinding_commands: Vec::new(),
            };

            // Search user commands via CommandNameIndex (#547)
            if matches!(source, CommandSource::All | CommandSource::User)
                && let Some(index) = services.get::<CommandNameIndex>()
            {
                resp.user_commands = index
                    .search_by_prefix(&req.prefix)
                    .into_iter()
                    .map(|(id, cmd)| UserCommandEntry {
                        id: id.name().to_string(),
                        names: cmd.names().iter().map(|s| (*s).to_string()).collect(),
                        help: cmd.description().to_string(),
                    })
                    .collect();
            }

            // Search keybinding commands
            if matches!(source, CommandSource::All | CommandSource::Keybinding)
                && let Some(snapshot) = services.get::<CommandQuerySnapshot>()
            {
                resp.keybinding_commands = snapshot
                    .search_by_prefix(&req.prefix)
                    .into_iter()
                    .map(|info| KeybindingCommandEntry {
                        id: info.id.to_string(),
                        names: info.names,
                        description: info.description,
                    })
                    .collect();
            }

            resp
        });

        Ok(Response::new(response))
    }

    async fn complete_args(
        &self,
        request: Request<CompleteArgsRequest>,
    ) -> Result<Response<CompleteArgsResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        let completions = session.with_state_sync(|state| {
            state
                .app
                .kernel
                .services
                .get::<CommandNameIndex>()
                .map_or_else(Vec::new, |index| index.complete_args(&req.command, &req.partial))
        });

        Ok(Response::new(CompleteArgsResponse { completions }))
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
