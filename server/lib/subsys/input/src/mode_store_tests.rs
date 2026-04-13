use reovim_kernel::api::v1::{CursorStyle, Mode, ModeId, ModuleId, Service};

use crate::{ModeInfo, ModeInfoStore};

// Mock mode for testing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TestMode;

impl Mode for TestMode {
    fn module() -> ModuleId {
        ModuleId::new("test")
    }

    fn discriminant(&self) -> u16 {
        0
    }

    fn id(&self) -> ModeId {
        ModeId::new(ModuleId::new("test"), "test")
    }

    fn display_name(&self) -> &'static str {
        "TEST"
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::Block
    }

    fn accepts_char_input(&self) -> bool {
        false
    }

    fn has_selection(&self) -> bool {
        false
    }

    fn inherits_from(&self) -> Option<Self> {
        None
    }

    fn is_entry(&self) -> bool {
        false
    }
}

#[test]
fn test_store_new() {
    let store = ModeInfoStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn test_store_add_mode() {
    let store = ModeInfoStore::new();
    store.add_mode(TestMode);

    assert_eq!(store.len(), 1);
    assert!(!store.is_empty());
}

#[test]
fn test_store_take_modes() {
    let store = ModeInfoStore::new();
    store.add_mode(TestMode);

    let modes = store.take_modes();
    assert_eq!(modes.len(), 1);
    assert_eq!(modes[0].display_name, "TEST");
    assert!(store.is_empty()); // Store should be empty after take
}

#[test]
fn test_mode_info_from_mode() {
    let info = ModeInfo::from_mode(TestMode);
    assert_eq!(info.display_name, "TEST");
    assert!(!info.accepts_char_input);
    assert!(!info.has_selection);
}

#[test]
fn test_mode_info_from_mode_all_fields() {
    let info = ModeInfo::from_mode(TestMode);
    assert_eq!(info.id, ModeId::new(ModuleId::new("test"), "test"));
    assert_eq!(info.display_name, "TEST");
    assert_eq!(info.cursor_style, CursorStyle::Block);
    assert!(!info.accepts_char_input);
    assert!(!info.has_selection);
    assert!(info.inherits_from.is_none());
    assert!(!info.is_entry);
}

#[test]
fn test_store_default() {
    let store = ModeInfoStore::default();
    assert!(store.is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_store_debug() {
    let store = ModeInfoStore::new();
    store.add_mode(TestMode);
    let debug = format!("{store:?}");
    assert!(debug.contains("ModeInfoStore"));
    assert!(debug.contains("count"));
    assert!(debug.contains('1'));
}

#[test]
fn test_store_add_direct() {
    let store = ModeInfoStore::new();
    let info = ModeInfo {
        id: ModeId::new(ModuleId::new("custom"), "custom-mode"),
        display_name: "CUSTOM",
        cursor_style: CursorStyle::Bar,
        accepts_char_input: true,
        has_selection: false,
        inherits_from: None,
        is_entry: true,
    };
    store.add(info);
    assert_eq!(store.len(), 1);

    let modes = store.take_modes();
    assert_eq!(modes[0].display_name, "CUSTOM");
    assert!(modes[0].accepts_char_input);
    assert!(modes[0].is_entry);
    assert_eq!(modes[0].cursor_style, CursorStyle::Bar);
}

#[test]
fn test_store_multiple_modes() {
    let store = ModeInfoStore::new();
    store.add_mode(TestMode);
    store.add_mode(TestMode);
    assert_eq!(store.len(), 2);
}

#[test]
fn test_store_take_clears() {
    let store = ModeInfoStore::new();
    store.add_mode(TestMode);
    store.add_mode(TestMode);

    let modes = store.take_modes();
    assert_eq!(modes.len(), 2);
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);

    // Taking again returns empty
    let modes = store.take_modes();
    assert!(modes.is_empty());
}

#[test]
fn test_mode_info_clone() {
    let info = ModeInfo::from_mode(TestMode);
    let cloned = info.clone();
    assert_eq!(cloned.display_name, "TEST");
    assert_eq!(cloned.id, info.id);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_mode_info_debug() {
    let info = ModeInfo::from_mode(TestMode);
    let debug = format!("{info:?}");
    assert!(debug.contains("ModeInfo"));
    assert!(debug.contains("TEST"));
}

/// Test a mode with inheritance to cover the `inherits_from` path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct InsertMode;

impl Mode for InsertMode {
    fn module() -> ModuleId {
        ModuleId::new("test")
    }
    fn discriminant(&self) -> u16 {
        1
    }
    fn id(&self) -> ModeId {
        ModeId::new(ModuleId::new("test"), "insert")
    }
    fn display_name(&self) -> &'static str {
        "INSERT"
    }
    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::Bar
    }
    fn accepts_char_input(&self) -> bool {
        true
    }
    fn has_selection(&self) -> bool {
        false
    }
    fn inherits_from(&self) -> Option<Self> {
        None
    }
    fn is_entry(&self) -> bool {
        false
    }
}

#[test]
fn test_mode_info_insert_mode_fields() {
    let info = ModeInfo::from_mode(InsertMode);
    assert_eq!(info.display_name, "INSERT");
    assert_eq!(info.cursor_style, CursorStyle::Bar);
    assert!(info.accepts_char_input);
}

#[test]
fn test_store_service_impl() {
    fn accepts_service(_: &dyn Service) {}
    let store = ModeInfoStore::new();
    accepts_service(&store);
}

#[test]
fn test_mode_info_inherits_from_none() {
    let info = ModeInfo::from_mode(TestMode);
    assert!(info.inherits_from.is_none());
}

#[test]
fn test_store_add_mode_uses_from_mode() {
    let store = ModeInfoStore::new();
    store.add_mode(InsertMode);
    let modes = store.take_modes();
    assert_eq!(modes.len(), 1);
    assert_eq!(modes[0].display_name, "INSERT");
    assert!(modes[0].accepts_char_input);
    assert_eq!(modes[0].cursor_style, CursorStyle::Bar);
}

#[test]
fn test_store_empty_after_take() {
    let store = ModeInfoStore::new();
    store.add_mode(TestMode);
    assert_eq!(store.len(), 1);
    let _ = store.take_modes();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn test_test_mode_module_and_discriminant() {
    assert_eq!(TestMode::module().as_str(), "test");
    assert_eq!(TestMode.discriminant(), 0);
}

#[test]
fn test_insert_mode_module_and_discriminant() {
    assert_eq!(InsertMode::module().as_str(), "test");
    assert_eq!(InsertMode.discriminant(), 1);
}

#[test]
fn test_mode_info_with_inherits_from() {
    let info = ModeInfo {
        id: ModeId::new(ModuleId::new("test"), "visual"),
        display_name: "VISUAL",
        cursor_style: CursorStyle::Block,
        accepts_char_input: false,
        has_selection: true,
        inherits_from: Some(ModeId::new(ModuleId::new("test"), "normal")),
        is_entry: false,
    };
    assert!(info.inherits_from.is_some());
    assert!(info.has_selection);
    assert_eq!(info.inherits_from.unwrap().name(), "normal");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_store_debug_empty() {
    let store = ModeInfoStore::new();
    let debug = format!("{store:?}");
    assert!(debug.contains("ModeInfoStore"));
    assert!(debug.contains('0'));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_store_debug_with_multiple() {
    let store = ModeInfoStore::new();
    store.add_mode(TestMode);
    store.add_mode(InsertMode);
    let debug = format!("{store:?}");
    assert!(debug.contains('2'));
}

// =========================================================================
// find_by_name
// =========================================================================

#[test]
fn test_find_by_name_returns_some_for_registered_mode() {
    let store = ModeInfoStore::new();
    store.add_mode(TestMode);
    let found = store.find_by_name("test", "test");
    assert!(found.is_some());
    assert_eq!(found.unwrap(), ModeId::new(ModuleId::new("test"), "test"));
}

#[test]
fn test_find_by_name_returns_none_for_missing_mode() {
    let store = ModeInfoStore::new();
    store.add_mode(TestMode);
    assert!(store.find_by_name("other", "missing").is_none());
}

#[test]
fn test_find_by_name_returns_none_for_wrong_module() {
    let store = ModeInfoStore::new();
    store.add_mode(TestMode);
    assert!(store.find_by_name("wrong", "test").is_none());
}

#[test]
fn test_find_by_name_returns_none_for_wrong_name() {
    let store = ModeInfoStore::new();
    store.add_mode(TestMode);
    assert!(store.find_by_name("test", "wrong").is_none());
}

#[test]
fn test_find_by_name_returns_none_on_empty_store() {
    let store = ModeInfoStore::new();
    assert!(store.find_by_name("test", "test").is_none());
}

#[test]
fn test_find_by_name_with_multiple_modes() {
    let store = ModeInfoStore::new();
    store.add_mode(TestMode);
    store.add_mode(InsertMode);
    assert!(store.find_by_name("test", "test").is_some());
    assert!(store.find_by_name("test", "insert").is_some());
    assert!(store.find_by_name("test", "visual").is_none());
}
