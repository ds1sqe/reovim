use reovim_kernel::api::v1::{CursorStyle, Mode, ModeId};

/// Information about a mode for registration.
#[derive(Debug, Clone)]
pub struct ModeInfo {
    /// The mode ID.
    pub id: ModeId,
    /// Display name for statusline.
    pub display_name: &'static str,
    /// Cursor style for this mode.
    pub cursor_style: CursorStyle,
    /// Whether this mode accepts character input.
    pub accepts_char_input: bool,
    /// Whether this mode has an active selection.
    pub has_selection: bool,
    /// Parent mode for keybinding inheritance.
    pub inherits_from: Option<ModeId>,
    /// Whether this is the entry/default mode for new sessions.
    pub is_entry: bool,
}

impl ModeInfo {
    /// Create mode info from a mode implementation.
    #[must_use]
    pub fn from_mode<M: Mode>(mode: M) -> Self {
        Self {
            id: mode.id(),
            display_name: mode.display_name(),
            cursor_style: mode.cursor_style(),
            accepts_char_input: mode.accepts_char_input(),
            has_selection: mode.has_selection(),
            inherits_from: mode.inherits_from().map(|m| m.id()),
            is_entry: mode.is_entry(),
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

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
    fn mode_info_from_mode_copies_runtime_metadata() {
        let mode = TestMode;
        let id = mode.id();
        let info = ModeInfo::from_mode(mode);
        assert_eq!(info.id, id);
        assert_eq!(info.display_name, "TEST");
        assert_eq!(info.cursor_style, CursorStyle::Block);
        assert!(!info.accepts_char_input);
        assert!(!info.has_selection);
        assert!(info.inherits_from.is_none());
        assert!(!info.is_entry);
    }
}
