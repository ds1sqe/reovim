//! `CommandService` gRPC implementation (#453).
//!
//! Provides command discovery and argument completion for cmdline UI.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_driver_command::{CommandQueryService, ExCommandQueryService, ExCommandRegistry},
    reovim_protocol::v2::{
        CommandSource, CompleteArgsRequest, CompleteArgsResponse, ExCommandEntry,
        KeybindingCommandEntry, SearchCommandsRequest, SearchCommandsResponse,
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
                ex_commands: Vec::new(),
                keybinding_commands: Vec::new(),
            };

            // Search ex-commands
            if matches!(source, CommandSource::All | CommandSource::Ex)
                && let Some(registry) = services.get::<ExCommandRegistry>()
            {
                resp.ex_commands = registry
                    .search_by_prefix(&req.prefix)
                    .into_iter()
                    .map(|info| ExCommandEntry {
                        id: info.id,
                        names: info.names,
                        help: info.help,
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
                .get::<ExCommandRegistry>()
                .map_or_else(Vec::new, |registry| {
                    registry.complete_args(&req.command, &req.partial)
                })
        });

        Ok(Response::new(CompleteArgsResponse { completions }))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::session::SessionState,
        reovim_driver_command::{ExCommandContext, ExCommandError, ExCommandHandler},
    };

    // === Helper: create a session with services registered ===

    fn make_session_with_ex_commands() -> Arc<Session> {
        struct WriteHandler;
        impl ExCommandHandler for WriteHandler {
            fn id(&self) -> &'static str {
                "write"
            }
            fn names(&self) -> &[&'static str] {
                &["w", "write"]
            }
            fn execute(
                &self,
                _ctx: &mut ExCommandContext<'_>,
                _args: &[&str],
            ) -> Result<(), ExCommandError> {
                Ok(())
            }
            fn help(&self) -> &'static str {
                "Write buffer"
            }
        }

        struct QuitHandler;
        impl ExCommandHandler for QuitHandler {
            fn id(&self) -> &'static str {
                "quit"
            }
            fn names(&self) -> &[&'static str] {
                &["q", "quit"]
            }
            fn execute(
                &self,
                _ctx: &mut ExCommandContext<'_>,
                _args: &[&str],
            ) -> Result<(), ExCommandError> {
                Ok(())
            }
            fn help(&self) -> &'static str {
                "Quit editor"
            }
        }

        let handlers: Vec<Arc<dyn ExCommandHandler>> =
            vec![Arc::new(WriteHandler), Arc::new(QuitHandler)];
        let registry = ExCommandRegistry::from_handlers(handlers);

        let state = SessionState::default();
        state.app.kernel.services.register(Arc::new(registry));

        Arc::new(Session::from_state(SessionId::new("test"), state))
    }

    fn make_sessions_with(session: &Arc<Session>) -> Arc<SessionRegistry> {
        let sessions = Arc::new(SessionRegistry::new());
        sessions.insert(session);
        sessions
    }

    // === Construction and error tests ===

    #[test]
    fn test_command_service_impl_new() {
        let sessions = Arc::new(SessionRegistry::new());
        let session_id = SessionId::new("test");
        let service = CommandServiceImpl::new(sessions, session_id);
        assert!(service.get_session().is_err());
    }

    #[test]
    fn test_command_service_impl_no_session() {
        let sessions = Arc::new(SessionRegistry::new());
        let session_id = SessionId::new("nonexistent");
        let service = CommandServiceImpl::new(sessions, session_id);
        let result = service.get_session();
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().code(), tonic::Code::NotFound);
    }

    // === SearchCommands tests ===

    #[tokio::test]
    async fn test_search_commands_ex_only() {
        let session = make_session_with_ex_commands();
        let sessions = make_sessions_with(&session);
        let service = CommandServiceImpl::new(sessions, SessionId::new("test"));

        let request = Request::new(SearchCommandsRequest {
            prefix: "w".to_string(),
            source: CommandSource::Ex.into(),
        });
        let response = service.search_commands(request).await.unwrap().into_inner();

        assert_eq!(response.ex_commands.len(), 1);
        assert_eq!(response.ex_commands[0].id, "write");
        assert!(response.keybinding_commands.is_empty());
    }

    #[tokio::test]
    async fn test_search_commands_keybinding_only() {
        let session = make_session_with_ex_commands();
        let sessions = make_sessions_with(&session);
        let service = CommandServiceImpl::new(sessions, SessionId::new("test"));

        let request = Request::new(SearchCommandsRequest {
            prefix: "w".to_string(),
            source: CommandSource::Keybinding.into(),
        });
        let response = service.search_commands(request).await.unwrap().into_inner();

        // No keybinding commands registered in our test session
        assert!(response.ex_commands.is_empty());
        assert!(response.keybinding_commands.is_empty());
    }

    #[tokio::test]
    async fn test_search_commands_all() {
        let session = make_session_with_ex_commands();
        let sessions = make_sessions_with(&session);
        let service = CommandServiceImpl::new(sessions, SessionId::new("test"));

        let request = Request::new(SearchCommandsRequest {
            prefix: String::new(),
            source: CommandSource::All.into(),
        });
        let response = service.search_commands(request).await.unwrap().into_inner();

        // Empty prefix → all ex-commands (2 handlers)
        assert_eq!(response.ex_commands.len(), 2);
    }

    #[tokio::test]
    async fn test_search_commands_no_session() {
        let sessions = Arc::new(SessionRegistry::new());
        let service = CommandServiceImpl::new(sessions, SessionId::new("test"));

        let request = Request::new(SearchCommandsRequest {
            prefix: "w".to_string(),
            source: CommandSource::All.into(),
        });
        let result = service.search_commands(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
    }

    // === CompleteArgs tests ===

    #[tokio::test]
    async fn test_complete_args_found() {
        struct ColorschemeHandler;
        impl ExCommandHandler for ColorschemeHandler {
            fn id(&self) -> &'static str {
                "colorscheme"
            }
            fn names(&self) -> &[&'static str] {
                &["colorscheme"]
            }
            fn execute(
                &self,
                _ctx: &mut ExCommandContext<'_>,
                _args: &[&str],
            ) -> Result<(), ExCommandError> {
                Ok(())
            }
            fn complete(&self, partial: &str) -> Vec<String> {
                vec![format!("{partial}-dark"), format!("{partial}-light")]
            }
        }

        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(ColorschemeHandler)];
        let registry = ExCommandRegistry::from_handlers(handlers);

        let state = SessionState::default();
        state.app.kernel.services.register(Arc::new(registry));

        let session = Arc::new(Session::from_state(SessionId::new("test"), state));
        let sessions = make_sessions_with(&session);
        let service = CommandServiceImpl::new(sessions, SessionId::new("test"));

        let request = Request::new(CompleteArgsRequest {
            command: "colorscheme".to_string(),
            partial: "gru".to_string(),
        });
        let response = service.complete_args(request).await.unwrap().into_inner();
        assert_eq!(response.completions.len(), 2);
        assert_eq!(response.completions[0], "gru-dark");
        assert_eq!(response.completions[1], "gru-light");
    }

    #[tokio::test]
    async fn test_complete_args_not_found() {
        let session = make_session_with_ex_commands();
        let sessions = make_sessions_with(&session);
        let service = CommandServiceImpl::new(sessions, SessionId::new("test"));

        let request = Request::new(CompleteArgsRequest {
            command: "nonexistent".to_string(),
            partial: String::new(),
        });
        let response = service.complete_args(request).await.unwrap().into_inner();
        assert!(response.completions.is_empty());
    }

    #[tokio::test]
    async fn test_complete_args_no_session() {
        let sessions = Arc::new(SessionRegistry::new());
        let service = CommandServiceImpl::new(sessions, SessionId::new("test"));

        let request = Request::new(CompleteArgsRequest {
            command: "w".to_string(),
            partial: String::new(),
        });
        let result = service.complete_args(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
    }
}
