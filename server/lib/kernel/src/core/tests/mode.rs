use super::*;

fn test_module() -> ModuleId {
    ModuleId::new("test-module")
}

// ========================================================================
// CursorStyle tests
// ========================================================================

#[test]
fn test_cursor_style_default() {
    let style = CursorStyle::default();
    assert_eq!(style, CursorStyle::Block);
}

#[test]
fn test_cursor_style_is_visible() {
    assert!(CursorStyle::Block.is_visible());
    assert!(CursorStyle::Bar.is_visible());
    assert!(CursorStyle::Underline.is_visible());
    assert!(!CursorStyle::Hidden.is_visible());
}

#[test]
fn test_cursor_style_name() {
    assert_eq!(CursorStyle::Block.name(), "block");
    assert_eq!(CursorStyle::Bar.name(), "bar");
    assert_eq!(CursorStyle::Underline.name(), "underline");
    assert_eq!(CursorStyle::Hidden.name(), "hidden");
}

// ========================================================================
// ModeId tests
// ========================================================================

#[test]
fn test_mode_id_new() {
    let module = test_module();
    let mode = ModeId::new(module.clone(), "normal");
    assert_eq!(mode.module(), &module);
    assert_eq!(mode.name(), "normal");
    assert_eq!(mode.discriminant(), 0); // default discriminant
}

#[test]
fn test_mode_id_with_discriminant() {
    let module = test_module();
    let mode = ModeId::with_discriminant(module.clone(), "INSERT", 1);
    assert_eq!(mode.module(), &module);
    assert_eq!(mode.name(), "INSERT");
    assert_eq!(mode.discriminant(), 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_mode_id_display() {
    let module = test_module();
    let mode = ModeId::with_discriminant(module, "NORMAL", 0);
    assert_eq!(format!("{mode}"), "test-module:NORMAL");
}

#[test]
fn test_mode_id_equality_by_discriminant() {
    let module = test_module();
    // Same module + discriminant = equal (even if names differ)
    let mode1 = ModeId::with_discriminant(module.clone(), "normal", 0);
    let mode2 = ModeId::with_discriminant(module.clone(), "NORMAL", 0);
    let mode3 = ModeId::with_discriminant(module, "insert", 1);
    assert_eq!(mode1, mode2); // Same discriminant
    assert_ne!(mode1, mode3); // Different discriminant
}

#[test]
fn test_mode_id_hash_by_discriminant() {
    use std::collections::HashSet;
    let module = test_module();
    // Same module + discriminant = same hash
    let mode1 = ModeId::with_discriminant(module.clone(), "normal", 0);
    let mode2 = ModeId::with_discriminant(module.clone(), "NORMAL", 0);
    let mode3 = ModeId::with_discriminant(module, "insert", 1);

    let mut set = HashSet::new();
    set.insert(mode1);
    assert!(set.contains(&mode2)); // Same discriminant
    assert!(!set.contains(&mode3)); // Different discriminant
}

// ========================================================================
// CommandId tests
// ========================================================================

#[test]
fn test_command_id_new() {
    let module = test_module();
    let cmd = CommandId::new(module.clone(), "cursor-down");
    assert_eq!(cmd.module(), &module);
    assert_eq!(cmd.name(), "cursor-down");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_id_display() {
    let module = test_module();
    let cmd = CommandId::new(module, "cursor-down");
    assert_eq!(format!("{cmd}"), "test-module:cursor-down");
}

#[test]
fn test_command_id_equality() {
    let module = test_module();
    let cmd1 = CommandId::new(module.clone(), "cursor-down");
    let cmd2 = CommandId::new(module.clone(), "cursor-down");
    let cmd3 = CommandId::new(module, "cursor-up");
    assert_eq!(cmd1, cmd2);
    assert_ne!(cmd1, cmd3);
}

// Note: OperatorId tests moved to modules/vim (Epic #385)

// ========================================================================
// ModeStack tests
// ========================================================================

#[test]
fn test_mode_stack_new() {
    let module = test_module();
    let normal = ModeId::new(module, "normal");
    let stack = ModeStack::new(normal.clone());
    assert_eq!(stack.current(), &normal);
    assert_eq!(stack.depth(), 1);
    assert!(stack.is_base());
}

#[test]
fn test_mode_stack_push_pop() {
    let module = test_module();
    let normal = ModeId::new(module.clone(), "normal");
    let op_pending = ModeId::new(module, "operator-pending");

    let mut stack = ModeStack::new(normal.clone());
    stack.push(op_pending.clone());

    assert_eq!(stack.current(), &op_pending);
    assert_eq!(stack.depth(), 2);
    assert!(!stack.is_base());

    assert_eq!(stack.pop(), Some(op_pending));
    assert_eq!(stack.current(), &normal);
    assert_eq!(stack.depth(), 1);
    assert!(stack.is_base());
}

#[test]
fn test_mode_stack_pop_base() {
    let module = test_module();
    let normal = ModeId::new(module, "normal");
    let mut stack = ModeStack::new(normal.clone());

    // Cannot pop the base mode
    assert_eq!(stack.pop(), None);
    assert_eq!(stack.current(), &normal);
    assert_eq!(stack.depth(), 1);
}

#[test]
fn test_mode_stack_set() {
    let module = test_module();
    let normal = ModeId::new(module.clone(), "normal");
    let insert = ModeId::new(module, "insert");

    let mut stack = ModeStack::new(normal);
    stack.set(insert.clone());

    assert_eq!(stack.current(), &insert);
    assert_eq!(stack.depth(), 1);
}

#[test]
fn test_mode_stack_as_slice() {
    let module = test_module();
    let normal = ModeId::new(module.clone(), "normal");
    let op_pending = ModeId::new(module, "operator-pending");

    let mut stack = ModeStack::new(normal.clone());
    stack.push(op_pending.clone());

    let slice = stack.as_slice();
    assert_eq!(slice.len(), 2);
    assert_eq!(slice[0], normal);
    assert_eq!(slice[1], op_pending);
}

#[test]
fn test_mode_stack_home() {
    let module = test_module();
    let normal = ModeId::new(module.clone(), "normal");
    let insert = ModeId::new(module, "insert");

    let mut stack = ModeStack::new(normal.clone());
    assert_eq!(stack.home(), &normal);

    // Home doesn't change when we push
    stack.push(insert);
    assert_eq!(stack.home(), &normal);

    // Home doesn't change when we pop
    stack.pop();
    assert_eq!(stack.home(), &normal);
}

#[test]
fn test_mode_stack_contains() {
    let module = test_module();
    // Use different discriminants for different modes
    let normal = ModeId::with_discriminant(module.clone(), "normal", 0);
    let insert = ModeId::with_discriminant(module.clone(), "insert", 1);
    let visual = ModeId::with_discriminant(module, "visual", 2);

    let mut stack = ModeStack::new(normal.clone());
    assert!(stack.contains(&normal));
    assert!(!stack.contains(&insert));
    assert!(!stack.contains(&visual));

    stack.push(insert.clone());
    assert!(stack.contains(&normal));
    assert!(stack.contains(&insert));
    assert!(!stack.contains(&visual));

    stack.pop();
    assert!(stack.contains(&normal));
    assert!(!stack.contains(&insert));
}

// ========================================================================
// Mode trait tests
// ========================================================================

// The Mode trait is intentionally NOT object-safe.
// It requires Copy + Clone + PartialEq + Eq + Hash, making dyn Mode invalid.
// This is by design: runtime storage uses ModeId, not dyn Mode.

// Test implementation of Mode trait
const TEST_MODULE: ModuleId = ModuleId::new("test");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
enum TestMode {
    Normal = 0,
    Insert = 1,
    Visual = 2,
}

impl Mode for TestMode {
    fn module() -> ModuleId {
        TEST_MODULE
    }

    fn discriminant(&self) -> u16 {
        *self as u16
    }

    fn display_name(&self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
            Self::Visual => "VISUAL",
        }
    }

    fn cursor_style(&self) -> CursorStyle {
        match self {
            Self::Insert => CursorStyle::Bar,
            _ => CursorStyle::Block,
        }
    }

    fn accepts_char_input(&self) -> bool {
        matches!(self, Self::Insert)
    }

    fn has_selection(&self) -> bool {
        matches!(self, Self::Visual)
    }

    fn inherits_from(&self) -> Option<Self> {
        match self {
            Self::Visual => Some(Self::Normal),
            _ => None,
        }
    }
}

#[test]
fn test_mode_implementation() {
    let mode = TestMode::Normal;
    assert_eq!(mode.id().name(), "NORMAL");
    assert_eq!(mode.id().module().as_str(), "test");
    assert_eq!(mode.discriminant(), 0);
}

#[test]
fn test_mode_cursor_style() {
    assert_eq!(TestMode::Normal.cursor_style(), CursorStyle::Block);
    assert_eq!(TestMode::Insert.cursor_style(), CursorStyle::Bar);
    assert_eq!(TestMode::Visual.cursor_style(), CursorStyle::Block);
}

#[test]
fn test_mode_accepts_char_input() {
    assert!(!TestMode::Normal.accepts_char_input());
    assert!(TestMode::Insert.accepts_char_input());
    assert!(!TestMode::Visual.accepts_char_input());
}

#[test]
fn test_mode_has_selection() {
    assert!(!TestMode::Normal.has_selection());
    assert!(!TestMode::Insert.has_selection());
    assert!(TestMode::Visual.has_selection());
}

#[test]
fn test_mode_inherits_from() {
    assert_eq!(TestMode::Normal.inherits_from(), None);
    assert_eq!(TestMode::Insert.inherits_from(), None);
    assert_eq!(TestMode::Visual.inherits_from(), Some(TestMode::Normal));
}

#[test]
fn test_mode_into_mode_id() {
    let mode = TestMode::Insert;
    let id: ModeId = mode.into();
    assert_eq!(id.name(), "INSERT");
    assert_eq!(id.discriminant(), 1);
    assert_eq!(id.module(), &TEST_MODULE);
}

#[test]
fn test_mode_stack_with_mode_trait() {
    // ModeStack accepts impl Into<ModeId>, so Mode types work directly
    let mut stack = ModeStack::new(TestMode::Normal);
    assert_eq!(stack.current().discriminant(), 0);

    stack.push(TestMode::Visual);
    assert_eq!(stack.current().discriminant(), 2);

    stack.set(TestMode::Insert);
    assert_eq!(stack.current().discriminant(), 1);
}

/// Test that `ModeId`s created with static vs dynamic `ModuleId`s are equal.
///
/// This is critical for the keymap wiring scenario where:
/// - Session mode is created with `ModeId::new(ModuleId::new("editor"), "normal")`
/// - Wired keybindings use `ModeId::new(ModuleId::from_string("editor".to_string()), "normal")`
///
/// Both should hash and compare equal for `HashMap` lookups to work.
#[test]
fn test_mode_id_static_vs_dynamic_module() {
    use std::collections::HashMap;

    // Static module ID (like in fallback_default_mode)
    let static_module = ModuleId::new("editor");
    let mode_a = ModeId::new(static_module, "normal");

    // Dynamic module ID (like in wire_module_keybindings)
    let dynamic_module = ModuleId::from_string("editor".to_string());
    let mode_b = ModeId::new(dynamic_module, "normal");

    // Should be equal
    assert_eq!(mode_a, mode_b, "ModeIds with same content should be equal");
    assert_eq!(mode_a.module(), mode_b.module(), "Module IDs should be equal");
    assert_eq!(mode_a.discriminant(), mode_b.discriminant(), "Discriminants should be equal");

    // Should have same hash (HashMap lookup must work)
    let mut map: HashMap<ModeId, &str> = HashMap::new();
    map.insert(mode_a.clone(), "from_a");

    assert_eq!(map.get(&mode_a), Some(&"from_a"), "Lookup with same ModeId should work");
    assert_eq!(map.get(&mode_b), Some(&"from_a"), "Lookup with equivalent ModeId should work");
}

// === CommandId::from_qualified_leaked without colon ===

#[test]
fn test_command_id_from_qualified_leaked_with_colon() {
    let cmd = CommandId::from_qualified_leaked("editor:cursor-down".to_string());
    assert_eq!(cmd.module().as_str(), "editor");
    assert_eq!(cmd.name(), "cursor-down");
}

#[test]
fn test_command_id_from_qualified_leaked_without_colon() {
    let cmd = CommandId::from_qualified_leaked("cursor-down".to_string());
    assert_eq!(cmd.module().as_str(), "unknown");
    assert_eq!(cmd.name(), "cursor-down");
}

// === Mode::is_entry default ===

#[test]
fn test_mode_is_entry_default() {
    // Default impl should return false
    assert!(!TestMode::Normal.is_entry());
    assert!(!TestMode::Insert.is_entry());
    assert!(!TestMode::Visual.is_entry());
}

// === Mode::id() default impl ===

#[test]
fn test_mode_id_default_impl() {
    let id = TestMode::Normal.id();
    assert_eq!(id.module(), &TEST_MODULE);
    assert_eq!(id.name(), "NORMAL");
    assert_eq!(id.discriminant(), 0);
}
