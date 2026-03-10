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
mod tests {
    use {
        super::*,
        crate::session::SessionState,
        reovim_driver_command::Command,
        reovim_kernel::api::v1::{CommandId, ModuleId},
    };

    // === Helper: create a session with CommandNameIndex registered ===

    fn make_session_with_name_index() -> Arc<Session> {
        struct WriteCmd;
        #[cfg_attr(coverage_nightly, coverage(off))]
        impl Command for WriteCmd {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("commands"), "write")
            }
            fn description(&self) -> &'static str {
                "Write buffer"
            }
            fn names(&self) -> &[&'static str] {
                &["w", "write"]
            }
        }

        struct QuitCmd;
        #[cfg_attr(coverage_nightly, coverage(off))]
        impl Command for QuitCmd {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("commands"), "quit")
            }
            fn description(&self) -> &'static str {
                "Quit editor"
            }
            fn names(&self) -> &[&'static str] {
                &["q", "quit"]
            }
        }

        let mut index = CommandNameIndex::new();
        let write: Arc<dyn Command> = Arc::new(WriteCmd);
        let quit: Arc<dyn Command> = Arc::new(QuitCmd);

        index.insert("w".to_string(), write.id(), Arc::clone(&write));
        index.insert("write".to_string(), write.id(), write);
        index.insert("q".to_string(), quit.id(), Arc::clone(&quit));
        index.insert("quit".to_string(), quit.id(), quit);

        let state = SessionState::default();
        state.app.kernel.services.register(Arc::new(index));

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
    async fn test_search_commands_user_only() {
        let session = make_session_with_name_index();
        let sessions = make_sessions_with(&session);
        let service = CommandServiceImpl::new(sessions, SessionId::new("test"));

        let request = Request::new(SearchCommandsRequest {
            prefix: "w".to_string(),
            source: CommandSource::User.into(),
        });
        let response = service.search_commands(request).await.unwrap().into_inner();

        assert_eq!(response.user_commands.len(), 1);
        assert_eq!(response.user_commands[0].id, "write");
        assert!(response.keybinding_commands.is_empty());
    }

    #[tokio::test]
    async fn test_search_commands_keybinding_only() {
        let session = make_session_with_name_index();
        let sessions = make_sessions_with(&session);
        let service = CommandServiceImpl::new(sessions, SessionId::new("test"));

        let request = Request::new(SearchCommandsRequest {
            prefix: "w".to_string(),
            source: CommandSource::Keybinding.into(),
        });
        let response = service.search_commands(request).await.unwrap().into_inner();

        // No keybinding commands registered in our test session
        assert!(response.user_commands.is_empty());
        assert!(response.keybinding_commands.is_empty());
    }

    #[tokio::test]
    async fn test_search_commands_all() {
        let session = make_session_with_name_index();
        let sessions = make_sessions_with(&session);
        let service = CommandServiceImpl::new(sessions, SessionId::new("test"));

        let request = Request::new(SearchCommandsRequest {
            prefix: String::new(),
            source: CommandSource::All.into(),
        });
        let response = service.search_commands(request).await.unwrap().into_inner();

        // Empty prefix → all user commands (2 unique commands)
        assert_eq!(response.user_commands.len(), 2);
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

    #[tokio::test]
    async fn test_search_commands_keybinding_with_snapshot() {
        use {
            crate::CommandQuerySnapshot,
            reovim_driver_command::{ArgSpec, CommandHandler, CommandResult},
            reovim_driver_session::SessionRuntime,
        };

        struct MoveDown;
        #[cfg_attr(coverage_nightly, coverage(off))]
        impl Command for MoveDown {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "move-down")
            }
            fn description(&self) -> &'static str {
                "Move cursor down"
            }
            fn args(&self) -> Vec<ArgSpec> {
                vec![]
            }
            fn names(&self) -> &[&'static str] {
                &["move-down"]
            }
        }
        #[cfg_attr(coverage_nightly, coverage(off))]
        impl CommandHandler for MoveDown {
            fn execute(
                &self,
                _runtime: &mut SessionRuntime<'_>,
                _args: &reovim_driver_command::CommandContext,
            ) -> CommandResult {
                CommandResult::Success
            }
        }

        let mut cmd_registry = crate::CommandRegistry::new();
        cmd_registry.register(Arc::new(MoveDown));
        let snapshot = CommandQuerySnapshot::from_registry(&cmd_registry);

        let state = SessionState::default();
        state.app.kernel.services.register(Arc::new(snapshot));
        let session = Arc::new(Session::from_state(SessionId::new("test"), state));
        let sessions = make_sessions_with(&session);
        let service = CommandServiceImpl::new(sessions, SessionId::new("test"));

        let request = Request::new(SearchCommandsRequest {
            prefix: "move".to_string(),
            source: CommandSource::Keybinding.into(),
        });
        let response = service.search_commands(request).await.unwrap().into_inner();

        assert_eq!(response.keybinding_commands.len(), 1);
        assert_eq!(response.keybinding_commands[0].id, "test:move-down");
        assert_eq!(response.keybinding_commands[0].names, vec!["move-down"]);
        assert_eq!(response.keybinding_commands[0].description, "Move cursor down");
    }

    // === CompleteArgs tests ===

    #[tokio::test]
    async fn test_complete_args_found() {
        struct ColorschemeCmd;
        #[cfg_attr(coverage_nightly, coverage(off))]
        impl Command for ColorschemeCmd {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("commands"), "colorscheme")
            }
            fn description(&self) -> &'static str {
                "Set colorscheme"
            }
            fn names(&self) -> &[&'static str] {
                &["colorscheme"]
            }
            fn complete(&self, partial: &str) -> Vec<String> {
                vec![format!("{partial}-dark"), format!("{partial}-light")]
            }
        }

        let mut index = CommandNameIndex::new();
        let cmd: Arc<dyn Command> = Arc::new(ColorschemeCmd);
        index.insert("colorscheme".to_string(), cmd.id(), cmd);

        let state = SessionState::default();
        state.app.kernel.services.register(Arc::new(index));

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
        let session = make_session_with_name_index();
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
