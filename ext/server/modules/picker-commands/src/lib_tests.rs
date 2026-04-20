use std::path::PathBuf;

use reovim_driver_picker::CommandInfo;

use super::*;

fn services() -> reovim_kernel::api::v1::ServiceRegistry {
    reovim_kernel::api::v1::ServiceRegistry::new()
}

fn empty_ctx() -> PickerContext {
    PickerContext {
        cwd: PathBuf::from("."),
        query: String::new(),
        buffers: vec![],
        commands: vec![],
        options: vec![],
    }
}

#[test]
fn name_and_title() {
    let picker = CommandsPicker::new();
    assert_eq!(picker.name(), "commands");
    assert_eq!(picker.title(), "Commands");
}

#[test]
fn is_static() {
    let picker = CommandsPicker::new();
    assert!(picker.is_static());
}

#[test]
fn default_prompt() {
    let picker = CommandsPicker::new();
    assert_eq!(picker.prompt(), "> ");
}

#[test]
fn items_empty_context() {
    let picker = CommandsPicker::new();
    let items = picker.items(&empty_ctx(), &services());
    assert!(items.is_empty());
}

#[test]
fn items_from_commands() {
    let picker = CommandsPicker::new();
    let ctx = PickerContext {
        cwd: PathBuf::from("."),
        query: String::new(),
        buffers: vec![],
        commands: vec![
            CommandInfo {
                qualified_name: "editor:save".to_owned(),
                description: "Save the current file".to_owned(),
            },
            CommandInfo {
                qualified_name: "editor:quit".to_owned(),
                description: "Quit the editor".to_owned(),
            },
        ],
        options: vec![],
    };

    let items = picker.items(&ctx, &services());
    assert_eq!(items.len(), 2);

    assert_eq!(items[0].display, "editor:save");
    assert_eq!(items[0].detail.as_deref(), Some("Save the current file"));
    assert!(items[0].icon.is_none());

    assert_eq!(items[1].display, "editor:quit");
    assert_eq!(items[1].detail.as_deref(), Some("Quit the editor"));
}

#[test]
fn on_select_command() {
    let picker = CommandsPicker::new();
    let item = PickerItem {
        display: "editor:save".to_owned(),
        detail: None,
        data: PickerData::Command("editor:save".to_owned()),
        icon: None,
    };
    let action = picker.on_select(&item);
    assert!(matches!(action, PickerAction::ExecuteCommand(ref name) if name == "editor:save"));
}

#[test]
fn on_select_wrong_data_closes() {
    let picker = CommandsPicker::new();
    let item = PickerItem {
        display: "test".to_owned(),
        detail: None,
        data: PickerData::Text("wrong".to_owned()),
        icon: None,
    };
    let action = picker.on_select(&item);
    assert!(matches!(action, PickerAction::Close));
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn default_impl() {
    let picker = CommandsPicker::default();
    assert_eq!(picker.name(), "commands");
}

#[test]
fn no_preview() {
    let picker = CommandsPicker::new();
    let item = PickerItem {
        display: "x".to_owned(),
        detail: None,
        data: PickerData::Command("x".to_owned()),
        icon: None,
    };
    assert!(picker.preview(&item, &services()).is_none());
}

// -- Module tests --

#[test]
fn module_id() {
    let module = PickerCommandsModule::new();
    assert_eq!(module.id().as_str(), "picker-commands");
}

#[test]
fn module_name() {
    let module = PickerCommandsModule::new();
    assert_eq!(module.name(), "Command Picker");
}

#[test]
fn module_version() {
    let module = PickerCommandsModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn module_default() {
    let module = PickerCommandsModule::default();
    assert_eq!(module.id().as_str(), "picker-commands");
}

#[test]
fn module_exit() {
    let mut module = PickerCommandsModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn module_init_registers_picker() {
    let services = Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
    let ctx = ModuleContext::new(
        reovim_kernel::api::v1::KernelContext::default(),
        services.clone(),
        PathBuf::from("/tmp"),
        PathBuf::from("/tmp"),
    );

    let mut module = PickerCommandsModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    let registry = services.get::<PickerRegistry>();
    assert!(registry.is_some());
    let reg = registry.unwrap();
    assert!(reg.get("commands").is_some());
}
