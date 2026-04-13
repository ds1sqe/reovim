use super::*;

// Test constants for keybinding tests
const TEST_MODULE: ModuleId = ModuleId::new("test");
const DELETE_LINE: CommandId = CommandId::new(TEST_MODULE, "delete-line");
const WINDOW_LEFT: CommandId = CommandId::new(TEST_MODULE, "window-left");
const GOTO_DEFINITION: CommandId = CommandId::new(TEST_MODULE, "goto-definition");

#[test]
fn test_command_registration_new() {
    let reg = CommandRegistration::new("delete");
    assert_eq!(reg.id, "delete");
    assert_eq!(reg.name, "");
    assert_eq!(reg.description, "");
    assert!(reg.category.is_none());
    assert!(!reg.accepts_count());
    assert!(!reg.accepts_operand());
    assert!(!reg.is_navigation());
    assert!(!reg.is_content_modifying());
    assert!(reg.depends_on.is_empty());
}

#[test]
fn test_command_registration_builder() {
    let reg = CommandRegistration::new("delete")
        .with_name("Delete")
        .with_description("Delete text")
        .with_category("operator")
        .with_count()
        .with_operand()
        .with_content_modifying()
        .with_depends_on(&["yank"])
        .with_flags(RegistrationFlags::required());

    assert_eq!(reg.id, "delete");
    assert_eq!(reg.name, "Delete");
    assert_eq!(reg.description, "Delete text");
    assert_eq!(reg.category, Some("operator"));
    assert!(reg.accepts_count());
    assert!(reg.accepts_operand());
    assert!(!reg.is_navigation());
    assert!(reg.is_content_modifying());
    assert_eq!(reg.depends_on, &["yank"]);
    assert!(reg.flags.is_required());
}

#[test]
fn test_command_registration_jump() {
    let reg = CommandRegistration::new("goto-definition").with_navigation();
    assert!(reg.is_navigation());
}

#[test]
fn test_keybinding_registration_new() {
    let reg = KeybindingRegistration::new("dd", DELETE_LINE);
    assert_eq!(reg.keys, "dd");
    assert_eq!(reg.command_id, DELETE_LINE);
    assert!(reg.modes.is_empty()); // All modes
    assert_eq!(reg.description, "");
    assert!(reg.category.is_none());
    assert!(reg.enabled);
    assert_eq!(reg.priority, 100); // Default plugin priority
    assert!(reg.depends_on.is_empty());
}

#[test]
fn test_keybinding_registration_builder() {
    let reg = KeybindingRegistration::new("<C-w>h", WINDOW_LEFT)
        .with_modes(&["normal"])
        .with_description("Move to left window")
        .with_category("window")
        .with_priority(50)
        .with_depends_on(&["window-split"])
        .with_flags(RegistrationFlags::deferrable());

    assert_eq!(reg.keys, "<C-w>h");
    assert_eq!(reg.command_id, WINDOW_LEFT);
    assert_eq!(reg.modes, &["normal"]);
    assert_eq!(reg.description, "Move to left window");
    assert_eq!(reg.category, Some("window"));
    assert!(reg.enabled);
    assert_eq!(reg.priority, 50);
    assert_eq!(reg.depends_on, &["window-split"]);
    assert!(reg.flags.is_deferrable());
}

#[test]
fn test_keybinding_registration_disabled() {
    let reg = KeybindingRegistration::new("gd", GOTO_DEFINITION).with_disabled();
    assert!(!reg.enabled);
}

#[test]
fn test_event_handler_registration_new() {
    let reg = EventHandlerRegistration::new("BufferChanged");
    assert_eq!(reg.event_type, "BufferChanged");
    assert_eq!(reg.priority, 100); // Default plugin priority
    assert_eq!(reg.description, "");
    assert!(!reg.once);
    assert!(reg.target_component.is_none());
    assert!(reg.depends_on.is_empty());
}

#[test]
fn test_event_handler_registration_builder() {
    let reg = EventHandlerRegistration::new("ContentModified")
        .with_priority(50)
        .with_description("Update content highlight")
        .with_target("treesitter")
        .with_depends_on(&["syntax-highlight"])
        .with_flags(RegistrationFlags::deferrable());

    assert_eq!(reg.event_type, "ContentModified");
    assert_eq!(reg.priority, 50);
    assert_eq!(reg.description, "Update content highlight");
    assert!(!reg.once);
    assert_eq!(reg.target_component, Some("treesitter"));
    assert_eq!(reg.depends_on, &["syntax-highlight"]);
    assert!(reg.flags.is_deferrable());
}

#[test]
fn test_event_handler_registration_once() {
    let reg = EventHandlerRegistration::new("ModuleLoaded").with_once();
    assert!(reg.once);
}

#[test]
fn test_event_handler_core_priority() {
    // Core priority clamped to 0-50
    let reg = EventHandlerRegistration::new("BufferChanged").core_priority(25);
    assert_eq!(reg.priority, 25);

    let reg = EventHandlerRegistration::new("BufferChanged").core_priority(100);
    assert_eq!(reg.priority, 50); // Clamped

    let reg = EventHandlerRegistration::new("BufferChanged").core_priority(0);
    assert_eq!(reg.priority, 0);
}
