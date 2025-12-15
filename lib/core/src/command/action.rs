/// Represents all possible editor commands/actions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    // === Cursor Movement ===
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    CursorLineStart,
    CursorLineEnd,
    CursorWordForward,
    CursorWordBackward,

    // === Mode Switching ===
    EnterNormalMode,
    EnterInsertMode,
    EnterInsertModeAfter,
    EnterVisualMode,
    EnterCommandMode,

    // === Text Operations ===
    InsertChar(char),
    DeleteCharBackward,
    DeleteCharForward,
    DeleteLine,

    // === Visual Mode ===
    VisualExtendUp,
    VisualExtendDown,
    VisualExtendLeft,
    VisualExtendRight,
    VisualDelete,
    VisualYank,

    // === Command Line Mode ===
    CommandLineChar(char),
    CommandLineBackspace,
    CommandLineExecute,
    CommandLineCancel,

    // === Clipboard ===
    Paste,
    PasteBefore,

    // === System ===
    Quit,
    Noop,
}
