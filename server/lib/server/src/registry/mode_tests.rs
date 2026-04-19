use super::*;

// Test mode enum implementing the Mode trait
const TEST_MODULE: ModuleId = ModuleId::new("test");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
enum TestMode {
    Command = 0,
    Input = 1,
    Selection = 2,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Mode for TestMode {
    fn module() -> ModuleId {
        TEST_MODULE
    }

    fn discriminant(&self) -> u16 {
        *self as u16
    }

    fn id(&self) -> ModeId {
        ModeId::with_discriminant(TEST_MODULE, self.display_name(), self.discriminant())
    }

    fn display_name(&self) -> &'static str {
        match self {
            Self::Command => "COMMAND",
            Self::Input => "INPUT",
            Self::Selection => "SELECTION",
        }
    }

    fn cursor_style(&self) -> CursorStyle {
        match self {
            Self::Command | Self::Selection => CursorStyle::Block,
            Self::Input => CursorStyle::Bar,
        }
    }

    fn accepts_char_input(&self) -> bool {
        matches!(self, Self::Input)
    }

    fn has_selection(&self) -> bool {
        matches!(self, Self::Selection)
    }

    fn inherits_from(&self) -> Option<Self> {
        match self {
            Self::Selection => Some(Self::Command),
            _ => None,
        }
    }

    fn is_entry(&self) -> bool {
        matches!(self, Self::Command)
    }
}

#[test]
fn test_mode_registry_new() {
    let registry = ModeRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_mode_registry_register() {
    let mut registry = ModeRegistry::new();
    registry.register_mode(TestMode::Command);

    assert!(registry.contains(&TestMode::Command.id()));
    assert_eq!(registry.len(), 1);
}

#[test]
fn test_mode_registry_register_all() {
    let mut registry = ModeRegistry::new();
    registry.register_all([TestMode::Command, TestMode::Input, TestMode::Selection]);

    assert_eq!(registry.len(), 3);
    assert!(registry.contains(&TestMode::Command.id()));
    assert!(registry.contains(&TestMode::Input.id()));
    assert!(registry.contains(&TestMode::Selection.id()));
}

#[test]
fn test_mode_registry_accepts_char_input() {
    let mut registry = ModeRegistry::new();
    registry.register_all([TestMode::Command, TestMode::Input]);

    assert!(!registry.accepts_char_input(&TestMode::Command.id()));
    assert!(registry.accepts_char_input(&TestMode::Input.id()));
}

#[test]
fn test_mode_registry_cursor_style() {
    let mut registry = ModeRegistry::new();
    registry.register_all([TestMode::Command, TestMode::Input]);

    assert_eq!(registry.cursor_style(&TestMode::Command.id()), CursorStyle::Block);
    assert_eq!(registry.cursor_style(&TestMode::Input.id()), CursorStyle::Bar);
}

#[test]
fn test_mode_registry_entry_mode_auto_detect() {
    let mut registry = ModeRegistry::new();

    // Registry starts with no entry mode
    assert!(registry.entry_mode().is_none());

    // Register entry mode (Command)
    registry.register_mode(TestMode::Command);
    assert_eq!(registry.entry_mode(), Some(&TestMode::Command.id()));
}

#[test]
fn test_mode_entry_with_owner() {
    let entry = ModeEntry::from_mode(TestMode::Command);
    assert!(entry.owner().is_none());

    let owned = entry.with_owner(TEST_MODULE);
    assert_eq!(owned.owner(), Some(&TEST_MODULE));
}

#[test]
fn test_mode_entry_id() {
    let entry = ModeEntry::from_mode(TestMode::Input);
    assert_eq!(entry.id(), &TestMode::Input.id());
}

#[test]
fn test_mode_registry_get() {
    let mut registry = ModeRegistry::new();
    registry.register_mode(TestMode::Command);

    assert!(registry.get(&TestMode::Command.id()).is_some());
    assert!(registry.get(&TestMode::Input.id()).is_none());
}

#[test]
fn test_mode_registry_has_selection() {
    let mut registry = ModeRegistry::new();
    registry.register_all([TestMode::Command, TestMode::Selection]);

    assert!(!registry.has_selection(&TestMode::Command.id()));
    assert!(registry.has_selection(&TestMode::Selection.id()));
    // Unregistered mode
    assert!(!registry.has_selection(&TestMode::Input.id()));
}

#[test]
fn test_mode_registry_display_name() {
    let mut registry = ModeRegistry::new();
    registry.register_mode(TestMode::Command);

    assert_eq!(registry.display_name(&TestMode::Command.id()), "COMMAND");
    assert_eq!(registry.display_name(&TestMode::Input.id()), "UNKNOWN");
}

#[test]
fn test_mode_registry_inherits_from() {
    let mut registry = ModeRegistry::new();
    registry.register_all([TestMode::Command, TestMode::Selection]);

    assert!(registry.inherits_from(&TestMode::Command.id()).is_none());
    assert!(registry.inherits_from(&TestMode::Selection.id()).is_some());
    assert!(registry.inherits_from(&TestMode::Input.id()).is_none());
}

#[test]
fn test_mode_registry_cursor_style_unregistered() {
    let registry = ModeRegistry::new();
    // Unregistered mode should return Block
    assert_eq!(registry.cursor_style(&TestMode::Input.id()), CursorStyle::Block);
}

#[test]
fn test_mode_registry_ids() {
    let mut registry = ModeRegistry::new();
    registry.register_all([TestMode::Command, TestMode::Input]);

    assert_eq!(registry.ids().count(), 2);
}

#[test]
fn test_mode_registry_find_by_name() {
    let mut registry = ModeRegistry::new();
    registry.register_all([TestMode::Command, TestMode::Input]);

    let found = registry.find_by_name("test", "COMMAND");
    assert!(found.is_some());

    let not_found = registry.find_by_name("test", "nonexistent");
    assert!(not_found.is_none());

    let wrong_module = registry.find_by_name("wrong", "COMMAND");
    assert!(wrong_module.is_none());
}

#[test]
fn test_mode_registry_len_is_empty() {
    let mut registry = ModeRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);

    registry.register_mode(TestMode::Command);
    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 1);
}

#[test]
fn test_mode_registry_unregister_for_module() {
    let mut registry = ModeRegistry::new();
    let entry = ModeEntry::from_mode(TestMode::Command).with_owner(TEST_MODULE);
    registry.register(entry);

    let entry2 = ModeEntry::from_mode(TestMode::Input); // no owner
    registry.register(entry2);

    assert_eq!(registry.len(), 2);
    let removed = registry.unregister_for_module(&TEST_MODULE);
    assert_eq!(removed, 1);
    assert_eq!(registry.len(), 1);
}

#[test]
fn test_mode_entry_from_info() {
    let info = reovim_subsys_input_contracts::ModeInfo {
        id: TestMode::Command.id(),
        display_name: "TEST",
        cursor_style: CursorStyle::Underline,
        accepts_char_input: true,
        has_selection: false,
        inherits_from: None,
        is_entry: false,
    };
    let entry = ModeEntry::from_info(info);
    assert_eq!(entry.display_name, "TEST");
    assert_eq!(entry.cursor_style, CursorStyle::Underline);
    assert!(entry.accepts_char_input);
}

#[test]
fn test_mode_registry_default() {
    let registry = ModeRegistry::default();
    assert!(registry.is_empty());
}

#[test]
fn test_mode_registry_debug() {
    let registry = ModeRegistry::new();
    let debug = format!("{registry:?}");
    assert!(debug.contains("ModeRegistry"));
}

#[test]
fn test_mode_entry_first_entry_wins() {
    let mut registry = ModeRegistry::new();
    // Command is_entry = true, so it becomes the entry mode
    registry.register_mode(TestMode::Command);
    // Input is_entry = false, but Selection inherits from Command
    registry.register_mode(TestMode::Input);
    // Entry mode should still be Command (first registered entry mode)
    assert_eq!(registry.entry_mode(), Some(&TestMode::Command.id()));
}
