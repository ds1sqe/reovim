use reovim_kernel::api::v1::{CursorStyle, Mode, ModeId};

use crate::modes::*;

#[test]
fn test_vim_mode_module() {
    assert_eq!(VimMode::module(), VIM_MODULE);
    assert_eq!(VIM_MODULE.as_str(), "vim");
}

#[test]
fn test_vim_mode_discriminants() {
    assert_eq!(VimMode::Normal.discriminant(), 0);
    assert_eq!(VimMode::Insert.discriminant(), 1);
    assert_eq!(VimMode::Visual.discriminant(), 2);
    assert_eq!(VimMode::VisualLine.discriminant(), 3);
    assert_eq!(VimMode::VisualBlock.discriminant(), 4);
    assert_eq!(VimMode::Replace.discriminant(), 5);
    assert_eq!(VimMode::CommandLine.discriminant(), 6);
    assert_eq!(VimMode::Window.discriminant(), 8);
    assert_eq!(VimMode::Delete.discriminant(), 9);
    assert_eq!(VimMode::Yank.discriminant(), 10);
    assert_eq!(VimMode::Change.discriminant(), 11);
}

#[test]
fn test_vim_mode_display_names() {
    assert_eq!(VimMode::Normal.display_name(), "NORMAL");
    assert_eq!(VimMode::Insert.display_name(), "INSERT");
    assert_eq!(VimMode::Visual.display_name(), "VISUAL");
    assert_eq!(VimMode::VisualLine.display_name(), "V-LINE");
    assert_eq!(VimMode::VisualBlock.display_name(), "V-BLOCK");
    assert_eq!(VimMode::Replace.display_name(), "REPLACE");
    assert_eq!(VimMode::CommandLine.display_name(), "COMMAND");
    assert_eq!(VimMode::Window.display_name(), "WINDOW");
    assert_eq!(VimMode::Delete.display_name(), "DELETE");
    assert_eq!(VimMode::Yank.display_name(), "YANK");
    assert_eq!(VimMode::Change.display_name(), "CHANGE");
}

#[test]
fn test_vim_mode_cursor_styles() {
    assert_eq!(VimMode::Normal.cursor_style(), CursorStyle::Block);
    assert_eq!(VimMode::Insert.cursor_style(), CursorStyle::Bar);
    assert_eq!(VimMode::Visual.cursor_style(), CursorStyle::Block);
    assert_eq!(VimMode::VisualLine.cursor_style(), CursorStyle::Block);
    assert_eq!(VimMode::VisualBlock.cursor_style(), CursorStyle::Block);
    assert_eq!(VimMode::Replace.cursor_style(), CursorStyle::Underline);
    assert_eq!(VimMode::CommandLine.cursor_style(), CursorStyle::Bar);
    assert_eq!(VimMode::Window.cursor_style(), CursorStyle::Block);
    assert_eq!(VimMode::Delete.cursor_style(), CursorStyle::Block);
    assert_eq!(VimMode::Yank.cursor_style(), CursorStyle::Block);
    assert_eq!(VimMode::Change.cursor_style(), CursorStyle::Block);
}

#[test]
fn test_vim_mode_accepts_char_input() {
    assert!(!VimMode::Normal.accepts_char_input());
    assert!(VimMode::Insert.accepts_char_input());
    assert!(!VimMode::Visual.accepts_char_input());
    assert!(!VimMode::VisualLine.accepts_char_input());
    assert!(!VimMode::VisualBlock.accepts_char_input());
    assert!(VimMode::Replace.accepts_char_input());
    assert!(VimMode::CommandLine.accepts_char_input());
    assert!(!VimMode::Window.accepts_char_input());
    assert!(!VimMode::Delete.accepts_char_input());
    assert!(!VimMode::Yank.accepts_char_input());
    assert!(!VimMode::Change.accepts_char_input());
}

#[test]
fn test_vim_mode_has_selection() {
    assert!(!VimMode::Normal.has_selection());
    assert!(!VimMode::Insert.has_selection());
    assert!(VimMode::Visual.has_selection());
    assert!(VimMode::VisualLine.has_selection());
    assert!(VimMode::VisualBlock.has_selection());
    assert!(!VimMode::Replace.has_selection());
    assert!(!VimMode::CommandLine.has_selection());
    assert!(!VimMode::Window.has_selection());
    assert!(!VimMode::Delete.has_selection());
    assert!(!VimMode::Yank.has_selection());
    assert!(!VimMode::Change.has_selection());
}

#[test]
fn test_vim_mode_inheritance() {
    // Normal: no parent
    assert_eq!(VimMode::Normal.inherits_from(), None);

    // Insert: no parent
    assert_eq!(VimMode::Insert.inherits_from(), None);

    // CommandLine: no parent
    assert_eq!(VimMode::CommandLine.inherits_from(), None);

    // Visual: inherits from Normal
    assert_eq!(VimMode::Visual.inherits_from(), Some(VimMode::Normal));

    // VisualLine/VisualBlock: inherit from Visual
    assert_eq!(VimMode::VisualLine.inherits_from(), Some(VimMode::Visual));
    assert_eq!(VimMode::VisualBlock.inherits_from(), Some(VimMode::Visual));

    // Replace: inherits from Insert
    assert_eq!(VimMode::Replace.inherits_from(), Some(VimMode::Insert));

    // Window: inherits from Normal
    assert_eq!(VimMode::Window.inherits_from(), Some(VimMode::Normal));

    // Operator modes: inherit from Normal (allows motion keys)
    assert_eq!(VimMode::Delete.inherits_from(), Some(VimMode::Normal));
    assert_eq!(VimMode::Yank.inherits_from(), Some(VimMode::Normal));
    assert_eq!(VimMode::Change.inherits_from(), Some(VimMode::Normal));
}

#[test]
fn test_vim_mode_id_round_trip() {
    for mode in VimMode::ALL {
        let id = mode.id();
        assert_eq!(id.module(), &VIM_MODULE);
        // ModeId name is lowercase, display_name is uppercase
        assert_ne!(id.name(), mode.display_name());
        assert_eq!(id.discriminant(), mode.discriminant());
    }
}

#[test]
fn test_vim_mode_id_equality() {
    // Same mode produces equal IDs
    assert_eq!(VimMode::Normal.id(), VimMode::Normal.id());
    assert_eq!(VimMode::Insert.id(), VimMode::Insert.id());

    // Different modes produce different IDs
    assert_ne!(VimMode::Normal.id(), VimMode::Insert.id());
    assert_ne!(VimMode::Visual.id(), VimMode::VisualLine.id());
}

#[test]
fn test_vim_mode_all_count() {
    // 14 modes total (7 original + Window + 3 operator modes + 3 case operator modes)
    assert_eq!(VimMode::ALL.len(), 14);
}

#[test]
fn test_vim_mode_copy_clone() {
    // Verify Copy and Clone work
    let mode = VimMode::Normal;
    let copied = mode;
    #[allow(clippy::clone_on_copy)]
    let cloned = mode.clone(); // Explicit clone to test Clone impl
    assert_eq!(mode, copied);
    assert_eq!(mode, cloned);
}

#[test]
fn test_vim_mode_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    for mode in VimMode::ALL {
        set.insert(*mode);
    }
    assert_eq!(set.len(), VimMode::ALL.len());
}

#[test]
fn test_vim_mode_into_mode_id() {
    // Test blanket impl From<M> for ModeId
    let mode = VimMode::Insert;
    let id: ModeId = mode.into();
    assert_eq!(id.discriminant(), VimMode::Insert.discriminant());
}

#[test]
fn test_vim_mode_is_entry() {
    // Only Normal mode is the entry mode
    assert!(VimMode::Normal.is_entry());

    // All other modes are not entry modes
    assert!(!VimMode::Insert.is_entry());
    assert!(!VimMode::Visual.is_entry());
    assert!(!VimMode::VisualLine.is_entry());
    assert!(!VimMode::VisualBlock.is_entry());
    assert!(!VimMode::Replace.is_entry());
    assert!(!VimMode::CommandLine.is_entry());
    assert!(!VimMode::Window.is_entry());
}

// ========================================================================
// Additional mode tests
// ========================================================================

#[test]
fn test_vim_mode_debug() {
    assert!(format!("{:?}", VimMode::Normal).contains("Normal"));
    assert!(format!("{:?}", VimMode::Insert).contains("Insert"));
    assert!(format!("{:?}", VimMode::Visual).contains("Visual"));
    assert!(format!("{:?}", VimMode::VisualLine).contains("VisualLine"));
    assert!(format!("{:?}", VimMode::VisualBlock).contains("VisualBlock"));
    assert!(format!("{:?}", VimMode::Replace).contains("Replace"));
    assert!(format!("{:?}", VimMode::CommandLine).contains("CommandLine"));
    assert!(format!("{:?}", VimMode::Window).contains("Window"));
    assert!(format!("{:?}", VimMode::Delete).contains("Delete"));
    assert!(format!("{:?}", VimMode::Yank).contains("Yank"));
    assert!(format!("{:?}", VimMode::Change).contains("Change"));
}

#[test]
fn test_vim_mode_mode_id_names() {
    assert_eq!(VimMode::Normal.id().name(), "normal");
    assert_eq!(VimMode::Insert.id().name(), "insert");
    assert_eq!(VimMode::Visual.id().name(), "visual");
    assert_eq!(VimMode::VisualLine.id().name(), "visual-line");
    assert_eq!(VimMode::VisualBlock.id().name(), "visual-block");
    assert_eq!(VimMode::Replace.id().name(), "replace");
    assert_eq!(VimMode::CommandLine.id().name(), "command");
    assert_eq!(VimMode::Window.id().name(), "window");
    assert_eq!(VimMode::Delete.id().name(), "delete");
    assert_eq!(VimMode::Yank.id().name(), "yank");
    assert_eq!(VimMode::Change.id().name(), "change");
}

#[test]
fn test_vim_mode_constants_match_ids() {
    assert_eq!(VimMode::Normal.id(), VimMode::NORMAL_ID);
    assert_eq!(VimMode::Insert.id(), VimMode::INSERT_ID);
    assert_eq!(VimMode::Visual.id(), VimMode::VISUAL_ID);
    assert_eq!(VimMode::VisualLine.id(), VimMode::VISUAL_LINE_ID);
    assert_eq!(VimMode::VisualBlock.id(), VimMode::VISUAL_BLOCK_ID);
    assert_eq!(VimMode::Replace.id(), VimMode::REPLACE_ID);
    assert_eq!(VimMode::CommandLine.id(), VimMode::COMMANDLINE_ID);
    assert_eq!(VimMode::Window.id(), VimMode::WINDOW_ID);
    assert_eq!(VimMode::Delete.id(), VimMode::DELETE_ID);
    assert_eq!(VimMode::Yank.id(), VimMode::YANK_ID);
    assert_eq!(VimMode::Change.id(), VimMode::CHANGE_ID);
}

#[test]
fn test_vim_mode_operator_modes_inherit_from_normal() {
    // All operator modes should inherit from Normal
    assert_eq!(VimMode::Delete.inherits_from(), Some(VimMode::Normal));
    assert_eq!(VimMode::Yank.inherits_from(), Some(VimMode::Normal));
    assert_eq!(VimMode::Change.inherits_from(), Some(VimMode::Normal));
}

#[test]
fn test_vim_mode_operator_modes_dont_accept_char() {
    assert!(!VimMode::Delete.accepts_char_input());
    assert!(!VimMode::Yank.accepts_char_input());
    assert!(!VimMode::Change.accepts_char_input());
}

#[test]
fn test_vim_mode_operator_modes_not_entry() {
    assert!(!VimMode::Delete.is_entry());
    assert!(!VimMode::Yank.is_entry());
    assert!(!VimMode::Change.is_entry());
}

#[test]
fn test_vim_mode_visual_modes_have_selection() {
    // All visual modes have selection
    assert!(VimMode::Visual.has_selection());
    assert!(VimMode::VisualLine.has_selection());
    assert!(VimMode::VisualBlock.has_selection());

    // Non-visual modes do not
    assert!(!VimMode::Normal.has_selection());
    assert!(!VimMode::Insert.has_selection());
    assert!(!VimMode::Window.has_selection());
    assert!(!VimMode::Delete.has_selection());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_vim_mode_all_discriminants_unique() {
    use std::collections::HashSet;
    let mut discriminants = HashSet::new();
    for mode in VimMode::ALL {
        assert!(
            discriminants.insert(mode.discriminant()),
            "Duplicate discriminant: {}",
            mode.discriminant()
        );
    }
}

#[test]
fn test_vim_mode_all_ids_unique() {
    use std::collections::HashSet;
    let mut ids = HashSet::new();
    for mode in VimMode::ALL {
        assert!(ids.insert(mode.id()), "Duplicate id: {:?}", mode.id());
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_vim_mode_all_display_names_unique() {
    use std::collections::HashSet;
    let mut names = HashSet::new();
    for mode in VimMode::ALL {
        assert!(
            names.insert(mode.display_name()),
            "Duplicate display name: {}",
            mode.display_name()
        );
    }
}

#[test]
fn test_vim_module_constant() {
    assert_eq!(VIM_MODULE.as_str(), "vim");
}

#[test]
fn test_vim_mode_replace_inherits_from_insert() {
    assert_eq!(VimMode::Replace.inherits_from(), Some(VimMode::Insert));
}

#[test]
fn test_vim_mode_visual_line_block_inherit_from_visual() {
    assert_eq!(VimMode::VisualLine.inherits_from(), Some(VimMode::Visual));
    assert_eq!(VimMode::VisualBlock.inherits_from(), Some(VimMode::Visual));
}

#[test]
fn test_vim_mode_inequality() {
    assert_ne!(VimMode::Normal, VimMode::Insert);
    assert_ne!(VimMode::Visual, VimMode::VisualLine);
    assert_ne!(VimMode::Delete, VimMode::Yank);
    assert_ne!(VimMode::Yank, VimMode::Change);
}
