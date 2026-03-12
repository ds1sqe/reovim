use {super::*, crate::ArgKind, reovim_kernel::api::v1::ModuleId};

struct MockCommand {
    names: &'static [&'static str],
    module: &'static str,
    cmd_name: &'static str,
    desc: &'static str,
    cmd_args: Vec<ArgSpec>,
}

impl MockCommand {
    fn simple(names: &'static [&'static str]) -> Self {
        Self {
            names,
            module: "test",
            cmd_name: "mock-cmd",
            desc: "A mock command",
            cmd_args: vec![],
        }
    }
}

impl Command for MockCommand {
    fn id(&self) -> CommandId {
        CommandId::new(ModuleId::new(self.module), self.cmd_name)
    }

    fn description(&self) -> &'static str {
        self.desc
    }

    fn args(&self) -> Vec<ArgSpec> {
        self.cmd_args.clone()
    }

    fn names(&self) -> &[&'static str] {
        self.names
    }
}

#[test]
fn test_command_info_from_command() {
    let cmd = MockCommand::simple(&["write", "w"]);
    let info = CommandInfo::from_command(&cmd);

    assert_eq!(info.id.name(), "mock-cmd");
    assert_eq!(info.names, vec!["write", "w"]);
    assert_eq!(info.description, "A mock command");
    assert!(info.args.is_empty());
}

#[test]
fn test_command_info_from_command_with_args() {
    let cmd = MockCommand {
        names: &["test"],
        module: "test",
        cmd_name: "test-cmd",
        desc: "Test with args",
        cmd_args: vec![
            ArgSpec::required("count", ArgKind::Count, "Number of times"),
            ArgSpec::optional("register", ArgKind::Register, "Target register"),
        ],
    };
    let info = CommandInfo::from_command(&cmd);
    assert_eq!(info.args.len(), 2);
    assert!(info.args[0].required);
    assert!(!info.args[1].required);
}

#[test]
fn test_command_info_from_command_no_names() {
    let cmd = MockCommand::simple(&[]);
    let info = CommandInfo::from_command(&cmd);
    assert!(info.names.is_empty());
}

#[test]
fn test_command_info_has_user_names_true() {
    let cmd = MockCommand::simple(&["write", "w"]);
    let info = CommandInfo::from_command(&cmd);
    assert!(info.has_user_names());
}

#[test]
fn test_command_info_has_user_names_false() {
    let cmd = MockCommand::simple(&[]);
    let info = CommandInfo::from_command(&cmd);
    assert!(!info.has_user_names());
}

#[test]
fn test_command_info_has_user_names_single() {
    let cmd = MockCommand::simple(&["w"]);
    let info = CommandInfo::from_command(&cmd);
    assert!(info.has_user_names());
}

#[test]
fn test_command_info_clone() {
    let cmd = MockCommand::simple(&["write", "w"]);
    let info = CommandInfo::from_command(&cmd);
    let cloned = info.clone();

    assert_eq!(info.id.name(), cloned.id.name());
    assert_eq!(info.names, cloned.names);
    assert_eq!(info.description, cloned.description);
    assert_eq!(info.args.len(), cloned.args.len());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_info_debug() {
    let cmd = MockCommand::simple(&["write", "w"]);
    let info = CommandInfo::from_command(&cmd);
    let debug_str = format!("{info:?}");
    assert!(debug_str.contains("CommandInfo"));
    assert!(debug_str.contains("mock-cmd"));
}

#[test]
fn test_command_info_description_owned() {
    let cmd = MockCommand::simple(&[]);
    let info = CommandInfo::from_command(&cmd);
    // description is an owned String, verify it matches
    assert_eq!(info.description, "A mock command");
}

#[test]
fn test_command_info_names_owned() {
    let cmd = MockCommand::simple(&["w", "write"]);
    let info = CommandInfo::from_command(&cmd);
    // names should be owned Strings
    assert_eq!(info.names[0], "w");
    assert_eq!(info.names[1], "write");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_query_service_object_safe() {
    // Inner fn verifies compilation only, never called
    fn _accepts_dyn(_: &dyn CommandQueryService) {}
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_query_service_box_object_safe() {
    // Inner fn verifies compilation only, never called
    fn _accepts_box(_: Box<dyn CommandQueryService>) {}
}

// === CommandQueryProvider tests (#522) ===

#[test]
fn test_command_query_provider_empty() {
    let provider = CommandQueryProvider::new(vec![]);
    assert!(provider.list_all().is_empty());
    assert_eq!(provider.count(), 0);
}

#[test]
fn test_command_query_provider_with_commands() {
    let cmd = MockCommand::simple(&["w", "write"]);
    let info = CommandInfo::from_command(&cmd);
    let provider = CommandQueryProvider::new(vec![info]);
    assert_eq!(provider.count(), 1);
    assert_eq!(provider.list_all()[0].id.name(), "mock-cmd");
}

#[test]
fn test_command_query_provider_is_service() {
    use reovim_kernel::api::v1::ServiceRegistry;
    let provider = CommandQueryProvider::new(vec![]);
    let registry = ServiceRegistry::new();
    registry.register(std::sync::Arc::new(provider));
    assert!(registry.get::<CommandQueryProvider>().is_some());
}
