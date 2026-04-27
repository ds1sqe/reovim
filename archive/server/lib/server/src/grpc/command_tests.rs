use {
    super::*,
    crate::session::SessionState,
    reovim_kernel::api::v1::{CommandId, ModuleId},
    reovim_subsys_command::Command,
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

// test_search_commands_keybinding_with_snapshot: DELETED — uses CommandHandler/SessionRuntime
// which are driver-specific types unavailable in server dev-dependencies.

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
