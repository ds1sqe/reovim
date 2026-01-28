mod color;
mod span;
pub mod store;
mod style;
mod theme;
mod theme_override;

pub use {
    color::{ColorMode, downgrade_color, parse_color, rgb_to_ansi256},
    span::Span,
    store::{BufferHighlights, HighlightStore, LineHighlight},
    style::{Attributes, Style},
    theme::{
        BracketStyles, DiagnosticStyles, LeapStyles, SemanticStyles, StatusLineModeStyles, Theme,
        ThemeName,
    },
    theme_override::{StyleOverride, ThemeOverrideError, ThemeOverrides},
};

// Re-export Color from reovim_sys for plugin use
pub use reovim_sys::style::Color;

/// Identifies the source/type of highlight for layering and management
/// Lower values have lower priority (get overridden by higher values)
///
/// # Priority Layers
/// - **0-9**: Syntax layer (base highlighting)
/// - **10-19**: Search/navigation layer
/// - **20-29**: Selection layer
/// - **30-39**: Diagnostic layer
/// - **40-59**: Cursor/focus layer
/// - **60-69**: Mode indicator layer
/// - **70-89**: Semantic token layer
/// - **90-100**: User/plugin layer
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HighlightGroup {
    // === Syntax Layer (0-9) ===
    /// Base syntax highlighting (lowest priority)
    Syntax = 0,
    /// Semantic tokens from LSP (above basic syntax)
    Semantic = 3,
    /// Rainbow bracket coloring (just above syntax)
    RainbowBracket = 5,

    // === Search/Navigation Layer (10-19) ===
    /// Search matches
    Search = 10,
    /// Incremental search (current match)
    IncSearch = 15,
    /// Leap navigation labels
    LeapLabel = 17,

    // === Selection Layer (20-29) ===
    /// Visual selection
    Visual = 20,

    // === Diagnostic Layer (30-39) ===
    /// Diagnostics (increasing severity)
    DiagnosticHint = 30,
    DiagnosticInfo = 31,
    DiagnosticWarn = 32,
    DiagnosticError = 33,
    /// Deprecated/obsolete code (strikethrough)
    DiagnosticDeprecated = 34,
    /// Unnecessary/unused code (dimmed)
    DiagnosticUnnecessary = 35,

    // === Cursor/Focus Layer (40-59) ===
    /// Cursor line highlight
    CursorLine = 40,
    /// Cursor column highlight (if enabled)
    CursorColumn = 41,
    /// Matched bracket pair (high priority, visible over cursor line)
    MatchedBracket = 45,

    // === Mode Indicator Layer (60-69) ===
    /// Mode-specific styling for UI elements
    ModeNormal = 60,
    ModeInsert = 61,
    ModeVisual = 62,
    ModeCommand = 63,
    ModeReplace = 64,
    ModeOperatorPending = 65,

    // === Semantic Token Layer (70-89) ===
    /// Rust lifetime annotations ('a, 'static)
    SemanticLifetime = 70,
    /// Trait bounds (: Trait, where T: Bound)
    SemanticTraitBound = 71,
    /// async keyword
    SemanticAsync = 72,
    /// await keyword
    SemanticAwait = 73,
    /// unsafe keyword and blocks
    SemanticUnsafe = 74,
    /// Macro invocations
    SemanticMacro = 75,
    /// Attribute annotations (#[...])
    SemanticAttribute = 76,
    /// Mutable variables (underlined typically)
    SemanticMutable = 77,
    /// Constant values
    SemanticConst = 78,
    /// Static variables
    SemanticStatic = 79,

    // === User/Plugin Layer (90-100) ===
    /// Plugin-defined highlights
    Plugin = 90,
    /// Custom user highlights (highest priority)
    Custom = 100,
}

impl HighlightGroup {
    /// Check if this is a diagnostic group
    #[must_use]
    pub const fn is_diagnostic(self) -> bool {
        matches!(
            self,
            Self::DiagnosticHint
                | Self::DiagnosticInfo
                | Self::DiagnosticWarn
                | Self::DiagnosticError
                | Self::DiagnosticDeprecated
                | Self::DiagnosticUnnecessary
        )
    }

    /// Check if this is a mode indicator group
    #[must_use]
    pub const fn is_mode_indicator(self) -> bool {
        matches!(
            self,
            Self::ModeNormal
                | Self::ModeInsert
                | Self::ModeVisual
                | Self::ModeCommand
                | Self::ModeReplace
                | Self::ModeOperatorPending
        )
    }

    /// Check if this is a semantic token group
    #[must_use]
    pub const fn is_semantic(self) -> bool {
        matches!(
            self,
            Self::Semantic
                | Self::SemanticLifetime
                | Self::SemanticTraitBound
                | Self::SemanticAsync
                | Self::SemanticAwait
                | Self::SemanticUnsafe
                | Self::SemanticMacro
                | Self::SemanticAttribute
                | Self::SemanticMutable
                | Self::SemanticConst
                | Self::SemanticStatic
        )
    }

    /// Check if this is a cursor-related group
    #[must_use]
    pub const fn is_cursor_related(self) -> bool {
        matches!(self, Self::CursorLine | Self::CursorColumn)
    }
}

/// A single highlight entry combining span, style, and group
#[derive(Debug, Clone)]
pub struct Highlight {
    pub span: Span,
    pub style: Style,
    pub group: HighlightGroup,
}

impl Highlight {
    #[must_use]
    pub const fn new(span: Span, style: Style, group: HighlightGroup) -> Self {
        Self { span, style, group }
    }
}
