use {super::*, reovim_driver_command::Command};

#[test]
fn open_metadata() {
    let cmd = Open;
    assert_eq!(cmd.id(), ids::OPEN);
    assert!(!cmd.description().is_empty());
}

#[test]
fn close_metadata() {
    let cmd = Close;
    assert_eq!(cmd.id(), ids::CLOSE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn next_metadata() {
    let cmd = Next;
    assert_eq!(cmd.id(), ids::NEXT);
    assert!(!cmd.description().is_empty());
}

#[test]
fn prev_metadata() {
    let cmd = Prev;
    assert_eq!(cmd.id(), ids::PREV);
    assert!(!cmd.description().is_empty());
}

#[test]
fn toggle_filter_metadata() {
    let cmd = ToggleFilter;
    assert_eq!(cmd.id(), ids::TOGGLE_FILTER);
    assert!(!cmd.description().is_empty());
}

#[test]
fn toggle_detail_metadata() {
    let cmd = ToggleDetail;
    assert_eq!(cmd.id(), ids::TOGGLE_DETAIL);
    assert!(!cmd.description().is_empty());
}

#[test]
fn command_handlers_count() {
    let handlers = command_handlers();
    assert_eq!(handlers.len(), 6);
}

#[test]
fn command_handlers_unique_ids() {
    let handlers = command_handlers();
    let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
    let mut deduped = ids.clone();
    deduped.sort_by_key(CommandId::name);
    deduped.dedup_by_key(|id| id.name());
    assert_eq!(ids.len(), deduped.len());
}

#[test]
fn collect_module_entries_empty_without_report() {
    let mut test = reovim_driver_session::testing::TestSessionRuntime::new();
    let entries = test.with_runtime(|rt| collect_module_entries(rt));
    assert!(entries.is_empty());
}

#[test]
fn collect_module_entries_with_report() {
    let mut test = reovim_driver_session::testing::TestSessionRuntime::new();

    {
        let mut report = ModuleLoadReport::new();
        report
            .loaded
            .push(reovim_kernel::api::v1::ModuleId::new("vim"));
        report
            .disabled
            .push(reovim_kernel::api::v1::ModuleId::new("treesitter-rust"));
        report
            .failed
            .push((reovim_kernel::api::v1::ModuleId::new("broken"), "crash".to_string()));
        test.kernel().services.register(std::sync::Arc::new(report));
    }

    let entries = test.with_runtime(|rt| collect_module_entries(rt));
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].id, "vim");
    assert!(matches!(entries[0].status, ModuleStatus::Loaded));
    assert_eq!(entries[1].id, "treesitter-rust");
    assert!(matches!(entries[1].status, ModuleStatus::Disabled));
    assert_eq!(entries[2].id, "broken");
    assert!(matches!(entries[2].status, ModuleStatus::Failed));
    assert_eq!(entries[2].reason.as_deref(), Some("crash"));
}
