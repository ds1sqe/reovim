//! Microscope fuzzy finder plugin for reovim
//!
//! This plugin provides fuzzy finding capabilities:
//! - File picker (Space ff)
//! - Buffer picker (Space fb)
//! - Grep picker (Space fg)
//! - Command palette
//! - Themes picker
//! - Keymaps viewer
//! - And more...
//!
//! # Architecture
//!
//! Commands emit `EventBus` events that are handled by the runtime.
//! State is managed via `PluginStateRegistry`.
//! Rendering is done via the `PluginWindow` trait.

pub mod commands;
pub mod microscope;

use std::{any::TypeId, sync::Arc};

use reovim_core::{
    bind::{CommandRef, EditModeKind, KeyMapInner, KeymapScope, SubModeKind},
    display::{DisplayInfo, EditModeKey},
    frame::FrameBuffer,
    highlight::Theme,
    keys,
    modd::ComponentId,
    plugin::{
        EditorContext, Plugin, PluginContext, PluginId, PluginStateRegistry, PluginWindow, Rect,
        WindowConfig,
    },
    rpc::{RpcHandler, RpcHandlerContext, RpcResult},
};

// Re-export unified command-event types
pub use commands::{
    MicroscopeBackspace, MicroscopeClearQuery, MicroscopeClose, MicroscopeCommands,
    MicroscopeConfirm, MicroscopeCursorEnd, MicroscopeCursorLeft, MicroscopeCursorRight,
    MicroscopeCursorStart, MicroscopeDeleteWord, MicroscopeEnterInsert, MicroscopeEnterNormal,
    MicroscopeFindBuffers, MicroscopeFindFiles, MicroscopeFindRecent, MicroscopeGotoFirst,
    MicroscopeGotoLast, MicroscopeHelp, MicroscopeInsertChar, MicroscopeKeymaps,
    MicroscopeLiveGrep, MicroscopeOpen, MicroscopePageDown, MicroscopePageUp, MicroscopeProfiles,
    MicroscopeSelectNext, MicroscopeSelectPrev, MicroscopeThemes, MicroscopeWordBackward,
    MicroscopeWordForward,
};

// Re-export microscope types (non-command/event)
pub use microscope::{
    BufferInfo, LayoutBounds, LayoutConfig, LoadingState, MatcherItem, MatcherStatus,
    MicroscopeAction, MicroscopeData, MicroscopeItem, MicroscopeMatcher, MicroscopeState,
    PanelBounds, Picker, PickerContext, PickerRegistry, PreviewContent, PromptMode, push_item,
    push_items,
};

/// Plugin window for microscope (Helix-style bottom-anchored layout)
pub struct MicroscopePluginWindow;

impl PluginWindow for MicroscopePluginWindow {
    fn window_config(
        &self,
        state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        // First check if active
        let is_active = state
            .with::<MicroscopeState, _, _>(|s| s.active)
            .unwrap_or(false);

        if !is_active {
            return None;
        }

        // Calculate and update layout bounds based on current screen dimensions
        state.with_mut::<MicroscopeState, _, _>(|s| {
            s.update_bounds(ctx.screen_width, ctx.screen_height);
        });

        // Now get the updated bounds
        state.with::<MicroscopeState, _, _>(|microscope| {
            let total = &microscope.bounds.total;
            Some(WindowConfig {
                bounds: Rect::new(total.x, total.y, total.width, total.height),
                z_order: 300, // Floating picker
                visible: true,
            })
        })?
    }

    #[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
    fn render(
        &self,
        state: &Arc<PluginStateRegistry>,
        _ctx: &EditorContext,
        buffer: &mut FrameBuffer,
        _bounds: Rect,
        theme: &Theme,
    ) {
        let Some(microscope) = state.with::<MicroscopeState, _, _>(Clone::clone) else {
            return;
        };

        let border_style = &theme.popup.border;
        let normal_style = &theme.popup.normal;
        let selected_style = &theme.popup.selected;

        let results = microscope.bounds.results;
        let status = microscope.bounds.status;

        // === Render Results Panel ===
        Self::render_results_panel(
            buffer,
            &microscope,
            &results,
            border_style,
            normal_style,
            selected_style,
        );

        // === Render Preview Panel (if enabled) ===
        if let Some(preview_bounds) = microscope.bounds.preview {
            Self::render_preview_panel(
                buffer,
                &microscope,
                &preview_bounds,
                border_style,
                normal_style,
                selected_style, // Use selected style for highlight_line
            );
        }

        // === Render Status Line ===
        Self::render_status_line(buffer, &microscope, &status, border_style, normal_style);
    }
}

impl MicroscopePluginWindow {
    /// Render the results panel (left side)
    #[allow(clippy::cast_possible_truncation)]
    fn render_results_panel(
        buffer: &mut FrameBuffer,
        microscope: &MicroscopeState,
        bounds: &PanelBounds,
        border_style: &reovim_core::highlight::Style,
        normal_style: &reovim_core::highlight::Style,
        selected_style: &reovim_core::highlight::Style,
    ) {
        let x = bounds.x;
        let y = bounds.y;
        let width = bounds.width;
        let height = bounds.height;

        // Top border with title
        buffer.put_char(x, y, '╭', border_style);
        let title = format!(" {} ", microscope.title);
        for (i, ch) in title.chars().enumerate() {
            let cx = x + 1 + i as u16;
            if cx < x + width - 1 {
                buffer.put_char(cx, y, ch, border_style);
            }
        }
        for cx in (x + 1 + title.len() as u16)..(x + width - 1) {
            buffer.put_char(cx, y, '─', border_style);
        }
        buffer.put_char(x + width - 1, y, '╮', border_style);

        // Input line with prompt
        let input_y = y + 1;
        buffer.put_char(x, input_y, '│', border_style);
        buffer.put_char(x + 1, input_y, '>', normal_style);
        buffer.put_char(x + 2, input_y, ' ', normal_style);

        // Query text with cursor position indicator
        for (i, ch) in microscope.query.chars().enumerate() {
            let cx = x + 3 + i as u16;
            if cx < x + width - 1 {
                buffer.put_char(cx, input_y, ch, normal_style);
            }
        }
        // Fill remaining space
        for cx in (x + 3 + microscope.query.len() as u16)..(x + width - 1) {
            buffer.put_char(cx, input_y, ' ', normal_style);
        }
        buffer.put_char(x + width - 1, input_y, '│', border_style);

        // Separator line
        let sep_y = y + 2;
        buffer.put_char(x, sep_y, '├', border_style);
        for cx in (x + 1)..(x + width - 1) {
            buffer.put_char(cx, sep_y, '─', border_style);
        }
        buffer.put_char(x + width - 1, sep_y, '┤', border_style);

        // Results list
        let results_start = y + 3;
        let visible_items = microscope.visible_items();
        let max_results = height.saturating_sub(4) as usize;
        let selected_in_view = microscope
            .selected_index
            .saturating_sub(microscope.scroll_offset);

        for (idx, item) in visible_items.iter().take(max_results).enumerate() {
            let ry = results_start + idx as u16;
            let is_selected = idx == selected_in_view;
            let style = if is_selected {
                selected_style
            } else {
                normal_style
            };

            buffer.put_char(x, ry, '│', border_style);

            // Selection indicator
            let indicator = if is_selected { '>' } else { ' ' };
            buffer.put_char(x + 1, ry, indicator, style);
            buffer.put_char(x + 2, ry, ' ', style);

            // Item icon if present
            let mut text_start = x + 3;
            if let Some(icon) = item.icon {
                buffer.put_char(text_start, ry, icon, style);
                buffer.put_char(text_start + 1, ry, ' ', style);
                text_start += 2;
            }

            // Item display text
            for (i, ch) in item.display.chars().enumerate() {
                let cx = text_start + i as u16;
                if cx < x + width - 1 {
                    buffer.put_char(cx, ry, ch, style);
                }
            }

            // Fill remaining space
            let text_end = text_start + item.display.len() as u16;
            for cx in text_end..(x + width - 1) {
                buffer.put_char(cx, ry, ' ', style);
            }

            buffer.put_char(x + width - 1, ry, '│', border_style);
        }

        // Empty rows
        let items_shown = visible_items.len().min(max_results) as u16;
        for ry in (results_start + items_shown)..(y + height - 1) {
            buffer.put_char(x, ry, '│', border_style);
            for cx in (x + 1)..(x + width - 1) {
                buffer.put_char(cx, ry, ' ', normal_style);
            }
            buffer.put_char(x + width - 1, ry, '│', border_style);
        }

        // Bottom border
        let bottom_y = y + height - 1;
        buffer.put_char(x, bottom_y, '╰', border_style);
        for cx in (x + 1)..(x + width - 1) {
            buffer.put_char(cx, bottom_y, '─', border_style);
        }
        buffer.put_char(x + width - 1, bottom_y, '┴', border_style);
    }

    /// Render the preview panel (right side)
    #[allow(clippy::cast_possible_truncation)]
    fn render_preview_panel(
        buffer: &mut FrameBuffer,
        microscope: &MicroscopeState,
        bounds: &PanelBounds,
        border_style: &reovim_core::highlight::Style,
        normal_style: &reovim_core::highlight::Style,
        highlight_style: &reovim_core::highlight::Style,
    ) {
        let x = bounds.x;
        let y = bounds.y;
        let width = bounds.width;
        let height = bounds.height;

        // Top border with preview title
        buffer.put_char(x, y, '┬', border_style);
        let title = microscope
            .preview
            .as_ref()
            .and_then(|p| p.title.as_deref())
            .unwrap_or(" Preview ");
        let title = format!(" {title} ");
        for (i, ch) in title.chars().enumerate() {
            let cx = x + 1 + i as u16;
            if cx < x + width - 1 {
                buffer.put_char(cx, y, ch, border_style);
            }
        }
        for cx in (x + 1 + title.len() as u16)..(x + width - 1) {
            buffer.put_char(cx, y, '─', border_style);
        }
        buffer.put_char(x + width - 1, y, '╮', border_style);

        // Preview content
        let content_start = y + 1;
        let max_lines = height.saturating_sub(2) as usize;

        if let Some(preview) = &microscope.preview {
            // Check if we have styled_lines for syntax highlighting
            let styled_lines = preview.styled_lines.as_ref();

            for (idx, line) in preview.lines.iter().take(max_lines).enumerate() {
                let ry = content_start + idx as u16;
                buffer.put_char(x, ry, '│', border_style);

                // Check if this line should be highlighted
                let is_highlight_line = preview.highlight_line == Some(idx);
                let base_style = if is_highlight_line {
                    highlight_style
                } else {
                    normal_style
                };

                // Get styled spans for this line if available
                let line_spans = styled_lines.and_then(|lines| lines.get(idx));

                // Render each character with appropriate style
                let mut byte_offset = 0;
                for (i, ch) in line.chars().enumerate() {
                    let cx = x + 1 + i as u16;
                    if cx < x + width - 1 {
                        // Find style for this character position
                        let char_style = if let Some(spans) = line_spans {
                            // Look for a span that covers this byte offset
                            spans
                                .iter()
                                .find(|span| byte_offset >= span.start && byte_offset < span.end)
                                .map(|span| &span.style)
                                .unwrap_or(base_style)
                        } else {
                            base_style
                        };
                        buffer.put_char(cx, ry, ch, char_style);
                    }
                    byte_offset += ch.len_utf8();
                }

                // Fill remaining space
                let line_end = x + 1 + line.chars().count().min((width - 2) as usize) as u16;
                for cx in line_end..(x + width - 1) {
                    buffer.put_char(cx, ry, ' ', base_style);
                }

                buffer.put_char(x + width - 1, ry, '│', border_style);
            }

            // Empty rows after content
            let lines_shown = preview.lines.len().min(max_lines) as u16;
            for ry in (content_start + lines_shown)..(y + height - 1) {
                buffer.put_char(x, ry, '│', border_style);
                for cx in (x + 1)..(x + width - 1) {
                    buffer.put_char(cx, ry, ' ', normal_style);
                }
                buffer.put_char(x + width - 1, ry, '│', border_style);
            }
        } else {
            // No preview content
            for ry in content_start..(y + height - 1) {
                buffer.put_char(x, ry, '│', border_style);
                for cx in (x + 1)..(x + width - 1) {
                    buffer.put_char(cx, ry, ' ', normal_style);
                }
                buffer.put_char(x + width - 1, ry, '│', border_style);
            }
        }

        // Bottom border
        let bottom_y = y + height - 1;
        buffer.put_char(x, bottom_y, '┴', border_style);
        for cx in (x + 1)..(x + width - 1) {
            buffer.put_char(cx, bottom_y, '─', border_style);
        }
        buffer.put_char(x + width - 1, bottom_y, '╯', border_style);
    }

    /// Render the status line (bottom)
    #[allow(clippy::cast_possible_truncation)]
    fn render_status_line(
        buffer: &mut FrameBuffer,
        microscope: &MicroscopeState,
        bounds: &PanelBounds,
        _border_style: &reovim_core::highlight::Style,
        normal_style: &reovim_core::highlight::Style,
    ) {
        let x = bounds.x;
        let y = bounds.y;
        let width = bounds.width;

        // Status text on left
        let status = microscope.status_text();
        for (i, ch) in status.chars().enumerate() {
            let cx = x + i as u16;
            if cx < x + width {
                buffer.put_char(cx, y, ch, normal_style);
            }
        }

        // Help text on right
        let help = "<CR> Confirm | <Esc> Close";
        let help_start = (x + width).saturating_sub(help.len() as u16);
        for (i, ch) in help.chars().enumerate() {
            let cx = help_start + i as u16;
            if cx >= x + status.len() as u16 && cx < x + width {
                buffer.put_char(cx, y, ch, normal_style);
            }
        }

        // Fill gap
        for cx in (x + status.len() as u16)..help_start {
            buffer.put_char(cx, y, ' ', normal_style);
        }
    }
}

/// Component ID for microscope
pub const COMPONENT_ID: ComponentId = ComponentId("microscope");

/// Command IDs for microscope
pub mod command_id {
    use reovim_core::command::CommandId;

    // Picker opening commands
    pub const MICROSCOPE_FIND_FILES: CommandId = CommandId::new("microscope_find_files");
    pub const MICROSCOPE_FIND_BUFFERS: CommandId = CommandId::new("microscope_find_buffers");
    pub const MICROSCOPE_LIVE_GREP: CommandId = CommandId::new("microscope_live_grep");
    pub const MICROSCOPE_RECENT_FILES: CommandId = CommandId::new("microscope_recent_files");
    pub const MICROSCOPE_COMMANDS: CommandId = CommandId::new("microscope_commands");
    pub const MICROSCOPE_HELP_TAGS: CommandId = CommandId::new("microscope_help_tags");
    pub const MICROSCOPE_KEYMAPS: CommandId = CommandId::new("microscope_keymaps");
    pub const MICROSCOPE_THEMES: CommandId = CommandId::new("microscope_themes");

    // Navigation commands
    pub const MICROSCOPE_SELECT_NEXT: CommandId = CommandId::new("microscope_select_next");
    pub const MICROSCOPE_SELECT_PREV: CommandId = CommandId::new("microscope_select_prev");
    pub const MICROSCOPE_PAGE_DOWN: CommandId = CommandId::new("microscope_page_down");
    pub const MICROSCOPE_PAGE_UP: CommandId = CommandId::new("microscope_page_up");
    pub const MICROSCOPE_GOTO_FIRST: CommandId = CommandId::new("microscope_goto_first");
    pub const MICROSCOPE_GOTO_LAST: CommandId = CommandId::new("microscope_goto_last");

    // Action commands
    pub const MICROSCOPE_CONFIRM: CommandId = CommandId::new("microscope_confirm");
    pub const MICROSCOPE_CLOSE: CommandId = CommandId::new("microscope_close");
    pub const MICROSCOPE_BACKSPACE: CommandId = CommandId::new("microscope_backspace");

    // Mode commands
    pub const MICROSCOPE_ENTER_INSERT: CommandId = CommandId::new("microscope_enter_insert");
    pub const MICROSCOPE_ENTER_NORMAL: CommandId = CommandId::new("microscope_enter_normal");

    // Prompt cursor commands
    pub const MICROSCOPE_CURSOR_LEFT: CommandId = CommandId::new("microscope_cursor_left");
    pub const MICROSCOPE_CURSOR_RIGHT: CommandId = CommandId::new("microscope_cursor_right");
    pub const MICROSCOPE_CURSOR_START: CommandId = CommandId::new("microscope_cursor_start");
    pub const MICROSCOPE_CURSOR_END: CommandId = CommandId::new("microscope_cursor_end");
    pub const MICROSCOPE_WORD_FORWARD: CommandId = CommandId::new("microscope_word_forward");
    pub const MICROSCOPE_WORD_BACKWARD: CommandId = CommandId::new("microscope_word_backward");
    pub const MICROSCOPE_CLEAR_QUERY: CommandId = CommandId::new("microscope_clear_query");
    pub const MICROSCOPE_DELETE_WORD: CommandId = CommandId::new("microscope_delete_word");
}

/// Microscope fuzzy finder plugin
///
/// Provides fuzzy finding capabilities:
/// - File picker (Space ff)
/// - Buffer picker (Space fb)
/// - Grep picker (Space fg)
/// - Command palette
/// - And more...
pub struct MicroscopePlugin;

/// RPC handler for state/microscope queries
struct MicroscopeStateHandler;

impl RpcHandler for MicroscopeStateHandler {
    fn method(&self) -> &'static str {
        "state/microscope"
    }

    fn handle(&self, _params: &serde_json::Value, ctx: &RpcHandlerContext) -> RpcResult {
        let state = ctx
            .with_state::<MicroscopeState, _, _>(|s| {
                serde_json::json!({
                    "active": s.active,
                    "query": s.query,
                    "selected_index": s.selected_index,
                    "item_count": s.items.len(),
                    "picker_name": s.picker_name,
                    "title": s.title,
                    "selected_item": s.selected_item().map(|i| i.display.clone()),
                    "prompt_mode": match s.prompt_mode {
                        PromptMode::Insert => "Insert",
                        PromptMode::Normal => "Normal",
                    },
                })
            })
            .unwrap_or_else(|| {
                serde_json::json!({
                    "active": false,
                    "query": "",
                    "selected_index": 0,
                    "item_count": 0,
                    "picker_name": "",
                    "title": "",
                    "selected_item": null,
                    "prompt_mode": "Insert",
                })
            });
        RpcResult::Success(state)
    }

    fn description(&self) -> &'static str {
        "Get microscope fuzzy finder state"
    }
}

impl Plugin for MicroscopePlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:microscope")
    }

    fn name(&self) -> &'static str {
        "Microscope"
    }

    fn description(&self) -> &'static str {
        "Fuzzy finder: files, buffers, grep, commands"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![]
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register display info for status line
        ctx.register_display(COMPONENT_ID, DisplayInfo::new(" MICROSCOPE ", "󰍉 "));
        ctx.register_component_mode_display(
            COMPONENT_ID,
            EditModeKey::Normal,
            DisplayInfo::new(" MICROSCOPE ", "󰍉 "),
        );
        ctx.register_component_mode_display(
            COMPONENT_ID,
            EditModeKey::Insert,
            DisplayInfo::new(" MICROSCOPE | INSERT ", "󰍉 "),
        );

        // Register picker opening commands
        let _ = ctx.register_command(MicroscopeFindFiles);
        let _ = ctx.register_command(MicroscopeFindBuffers);
        let _ = ctx.register_command(MicroscopeLiveGrep);
        let _ = ctx.register_command(MicroscopeFindRecent);
        let _ = ctx.register_command(MicroscopeCommands);
        let _ = ctx.register_command(MicroscopeHelp);
        let _ = ctx.register_command(MicroscopeKeymaps);
        let _ = ctx.register_command(MicroscopeThemes);
        let _ = ctx.register_command(MicroscopeProfiles);

        // Register navigation commands
        let _ = ctx.register_command(MicroscopeSelectNext);
        let _ = ctx.register_command(MicroscopeSelectPrev);
        let _ = ctx.register_command(MicroscopePageDown);
        let _ = ctx.register_command(MicroscopePageUp);
        let _ = ctx.register_command(MicroscopeGotoFirst);
        let _ = ctx.register_command(MicroscopeGotoLast);

        // Register action commands
        let _ = ctx.register_command(MicroscopeConfirm);
        let _ = ctx.register_command(MicroscopeClose);
        let _ = ctx.register_command(MicroscopeBackspace);

        // Register mode commands
        let _ = ctx.register_command(MicroscopeEnterInsert);
        let _ = ctx.register_command(MicroscopeEnterNormal);

        // Register prompt cursor commands
        let _ = ctx.register_command(MicroscopeCursorLeft);
        let _ = ctx.register_command(MicroscopeCursorRight);
        let _ = ctx.register_command(MicroscopeCursorStart);
        let _ = ctx.register_command(MicroscopeCursorEnd);
        let _ = ctx.register_command(MicroscopeWordForward);
        let _ = ctx.register_command(MicroscopeWordBackward);
        let _ = ctx.register_command(MicroscopeClearQuery);
        let _ = ctx.register_command(MicroscopeDeleteWord);

        // Register keybindings (editor normal mode)
        let editor_normal = KeymapScope::editor_normal();

        // Register Space+f prefix for multi-key sequences
        ctx.keymap_mut()
            .get_scope_mut(editor_normal.clone())
            .insert(keys![Space 'f'], KeyMapInner::with_description("+find").with_category("find"));

        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![Space 'f' 'f'],
            CommandRef::Registered(command_id::MICROSCOPE_FIND_FILES),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![Space 'f' 'b'],
            CommandRef::Registered(command_id::MICROSCOPE_FIND_BUFFERS),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![Space 'f' 'g'],
            CommandRef::Registered(command_id::MICROSCOPE_LIVE_GREP),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![Space 'f' 'r'],
            CommandRef::Registered(command_id::MICROSCOPE_RECENT_FILES),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![Space 'f' 'c'],
            CommandRef::Registered(command_id::MICROSCOPE_COMMANDS),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![Space 'f' 'h'],
            CommandRef::Registered(command_id::MICROSCOPE_HELP_TAGS),
        );
        ctx.bind_key_scoped(
            editor_normal,
            keys![Space 'f' 'k'],
            CommandRef::Registered(command_id::MICROSCOPE_KEYMAPS),
        );

        // Register keybindings for when microscope is focused (Normal mode)
        let microscope_normal = KeymapScope::Component {
            id: COMPONENT_ID,
            mode: EditModeKind::Normal,
        };

        // Navigation
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys!['j'],
            CommandRef::Registered(command_id::MICROSCOPE_SELECT_NEXT),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys!['k'],
            CommandRef::Registered(command_id::MICROSCOPE_SELECT_PREV),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys![(Ctrl 'n')],
            CommandRef::Registered(command_id::MICROSCOPE_SELECT_NEXT),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys![(Ctrl 'p')],
            CommandRef::Registered(command_id::MICROSCOPE_SELECT_PREV),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys![(Ctrl 'd')],
            CommandRef::Registered(command_id::MICROSCOPE_PAGE_DOWN),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys![(Ctrl 'u')],
            CommandRef::Registered(command_id::MICROSCOPE_PAGE_UP),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys!['g' 'g'],
            CommandRef::Registered(command_id::MICROSCOPE_GOTO_FIRST),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys!['G'],
            CommandRef::Registered(command_id::MICROSCOPE_GOTO_LAST),
        );

        // Actions
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys![Escape],
            CommandRef::Registered(command_id::MICROSCOPE_CLOSE),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys!['q'],
            CommandRef::Registered(command_id::MICROSCOPE_CLOSE),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys![Enter],
            CommandRef::Registered(command_id::MICROSCOPE_CONFIRM),
        );

        // Mode switching
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys!['i'],
            CommandRef::Registered(command_id::MICROSCOPE_ENTER_INSERT),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys!['a'],
            CommandRef::Registered(command_id::MICROSCOPE_ENTER_INSERT),
        );

        // Prompt cursor movement (Normal mode)
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys!['h'],
            CommandRef::Registered(command_id::MICROSCOPE_CURSOR_LEFT),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys!['l'],
            CommandRef::Registered(command_id::MICROSCOPE_CURSOR_RIGHT),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys!['0'],
            CommandRef::Registered(command_id::MICROSCOPE_CURSOR_START),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys!['$'],
            CommandRef::Registered(command_id::MICROSCOPE_CURSOR_END),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys!['w'],
            CommandRef::Registered(command_id::MICROSCOPE_WORD_FORWARD),
        );
        ctx.bind_key_scoped(
            microscope_normal.clone(),
            keys!['b'],
            CommandRef::Registered(command_id::MICROSCOPE_WORD_BACKWARD),
        );

        // Register g as prefix for gg
        ctx.keymap_mut()
            .get_scope_mut(microscope_normal)
            .insert(keys!['g'], KeyMapInner::new());

        // Register keybindings for microscope Insert mode
        let microscope_insert = KeymapScope::Component {
            id: COMPONENT_ID,
            mode: EditModeKind::Insert,
        };

        // Navigation in insert mode
        ctx.bind_key_scoped(
            microscope_insert.clone(),
            keys![(Ctrl 'n')],
            CommandRef::Registered(command_id::MICROSCOPE_SELECT_NEXT),
        );
        ctx.bind_key_scoped(
            microscope_insert.clone(),
            keys![(Ctrl 'p')],
            CommandRef::Registered(command_id::MICROSCOPE_SELECT_PREV),
        );

        // Editing in insert mode
        ctx.bind_key_scoped(
            microscope_insert.clone(),
            keys![Backspace],
            CommandRef::Registered(command_id::MICROSCOPE_BACKSPACE),
        );
        ctx.bind_key_scoped(
            microscope_insert.clone(),
            keys![(Ctrl 'u')],
            CommandRef::Registered(command_id::MICROSCOPE_CLEAR_QUERY),
        );
        ctx.bind_key_scoped(
            microscope_insert.clone(),
            keys![(Ctrl 'w')],
            CommandRef::Registered(command_id::MICROSCOPE_DELETE_WORD),
        );

        // Actions in insert mode
        ctx.bind_key_scoped(
            microscope_insert.clone(),
            keys![Escape],
            CommandRef::Registered(command_id::MICROSCOPE_ENTER_NORMAL),
        );
        ctx.bind_key_scoped(
            microscope_insert,
            keys![Enter],
            CommandRef::Registered(command_id::MICROSCOPE_CONFIRM),
        );

        // Register keybindings for Interactor sub-mode (text input mode)
        // This is used when microscope is in insert mode for capturing text
        let microscope_interactor = KeymapScope::SubMode(SubModeKind::Interactor(COMPONENT_ID));

        // Navigation in interactor mode
        ctx.bind_key_scoped(
            microscope_interactor.clone(),
            keys![(Ctrl 'n')],
            CommandRef::Registered(command_id::MICROSCOPE_SELECT_NEXT),
        );
        ctx.bind_key_scoped(
            microscope_interactor.clone(),
            keys![(Ctrl 'p')],
            CommandRef::Registered(command_id::MICROSCOPE_SELECT_PREV),
        );

        // Editing in interactor mode
        ctx.bind_key_scoped(
            microscope_interactor.clone(),
            keys![Backspace],
            CommandRef::Registered(command_id::MICROSCOPE_BACKSPACE),
        );
        ctx.bind_key_scoped(
            microscope_interactor.clone(),
            keys![(Ctrl 'u')],
            CommandRef::Registered(command_id::MICROSCOPE_CLEAR_QUERY),
        );
        ctx.bind_key_scoped(
            microscope_interactor.clone(),
            keys![(Ctrl 'w')],
            CommandRef::Registered(command_id::MICROSCOPE_DELETE_WORD),
        );

        // Actions in interactor mode
        ctx.bind_key_scoped(
            microscope_interactor.clone(),
            keys![Escape],
            CommandRef::Registered(command_id::MICROSCOPE_ENTER_NORMAL),
        );
        ctx.bind_key_scoped(
            microscope_interactor,
            keys![Enter],
            CommandRef::Registered(command_id::MICROSCOPE_CONFIRM),
        );

        // Register RPC handler for state/microscope queries
        ctx.register_rpc_handler(Arc::new(MicroscopeStateHandler));
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Initialize MicroscopeState
        registry.register(MicroscopeState::new());

        // Initialize PickerRegistry for dynamic picker registration
        registry.register(PickerRegistry::new());

        // Register the plugin window
        registry.register_plugin_window(Arc::new(MicroscopePluginWindow));
    }

    fn subscribe(&self, bus: &reovim_core::event_bus::EventBus, state: Arc<PluginStateRegistry>) {
        use reovim_core::{
            event_bus::{
                EventResult,
                core_events::{
                    PluginBackspace, PluginTextInput, RequestFocusChange, RequestModeChange,
                },
            },
            modd::{EditMode, ModeState, SubMode},
        };

        // Helper function to load preview for the currently selected item
        fn load_preview(state: &Arc<PluginStateRegistry>, picker_name: &str) {
            let picker = state
                .with::<PickerRegistry, _, _>(|r| r.get(picker_name))
                .flatten();

            if let Some(picker) = picker {
                let selected_item = state
                    .with::<MicroscopeState, _, _>(|s| s.selected_item().cloned())
                    .flatten();

                if let Some(item) = selected_item {
                    // Create picker context with syntax and decoration factories
                    let picker_ctx = PickerContext {
                        syntax_factory: state.syntax_factory(),
                        decoration_factory: state.decoration_factory(),
                        ..PickerContext::default()
                    };

                    let preview = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current()
                            .block_on(picker.preview(&item, &picker_ctx))
                    });
                    state.with_mut::<MicroscopeState, _, _>(|s| {
                        s.set_preview(preview);
                    });
                }
            }
        }

        // Handle MicroscopeOpen event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeOpen, _>(100, move |event, ctx| {
            let picker_name = event.picker.clone();

            // Get picker from registry
            let picker = state_clone
                .with::<PickerRegistry, _, _>(|registry| registry.get(&picker_name))
                .flatten();

            if let Some(picker) = picker {
                // Initialize microscope state with the picker
                state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                    s.open(&picker_name, picker.title(), picker.prompt());
                });

                // Fetch items from picker (synchronously for now)
                // The file walker is synchronous anyway, async is just trait signature
                let picker_ctx = PickerContext::default();
                let items = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(picker.fetch(&picker_ctx))
                });

                // Store items in state
                state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                    s.update_items(items);
                    s.set_loading(microscope::LoadingState::Idle);
                });

                // Load preview for first selected item
                load_preview(&state_clone, &picker_name);

                // Request focus change to microscope
                ctx.emit(RequestFocusChange {
                    target: COMPONENT_ID,
                });

                // Start in Normal mode (for j/k navigation, press 'i' to enter Insert)
                let mode = ModeState::with_interactor_id_and_mode(COMPONENT_ID, EditMode::Normal);
                ctx.emit(RequestModeChange { mode });

                ctx.request_render();
            } else {
                // Picker not found - log would require adding tracing dependency
            }

            EventResult::Handled
        });

        // Handle text input from runtime (PluginTextInput event)
        let state_clone = Arc::clone(&state);
        bus.subscribe::<PluginTextInput, _>(100, move |event, ctx| {
            // Only handle if target is microscope and microscope is active
            if event.target != COMPONENT_ID {
                return EventResult::NotHandled;
            }

            let is_active = state_clone
                .with::<MicroscopeState, _, _>(|s| s.active)
                .unwrap_or(false);

            if is_active {
                state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                    s.insert_char(event.c);
                });
                ctx.request_render();
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        });

        // Handle backspace from runtime (PluginBackspace event)
        let state_clone = Arc::clone(&state);
        bus.subscribe::<PluginBackspace, _>(100, move |event, ctx| {
            // Only handle if we're the target
            if event.target != COMPONENT_ID {
                return EventResult::NotHandled;
            }

            let is_active = state_clone
                .with::<MicroscopeState, _, _>(|s| s.active)
                .unwrap_or(false);

            if is_active {
                state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                    s.delete_char();
                });
                ctx.request_render();
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        });

        // Handle MicroscopeClose event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeClose, _>(100, move |_event, ctx| {
            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.close();
            });

            // Return focus and mode to editor normal
            ctx.emit(RequestFocusChange {
                target: reovim_core::modd::ComponentId("editor"),
            });
            let mode = ModeState::with_interactor_id_and_mode(
                reovim_core::modd::ComponentId::EDITOR,
                EditMode::Normal,
            );
            ctx.emit(RequestModeChange { mode });

            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeSelectNext event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeSelectNext, _>(100, move |_event, ctx| {
            let picker_name = state_clone
                .with::<MicroscopeState, _, _>(|s| s.picker_name.clone())
                .unwrap_or_default();

            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.select_next();
            });

            load_preview(&state_clone, &picker_name);
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeSelectPrev event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeSelectPrev, _>(100, move |_event, ctx| {
            let picker_name = state_clone
                .with::<MicroscopeState, _, _>(|s| s.picker_name.clone())
                .unwrap_or_default();

            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.select_prev();
            });

            load_preview(&state_clone, &picker_name);
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeBackspace event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeBackspace, _>(100, move |_event, ctx| {
            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.delete_char();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeConfirm event - open selected file
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeConfirm, _>(100, move |_event, ctx| {
            let selected = state_clone
                .with::<MicroscopeState, _, _>(|s| s.selected_item().cloned())
                .flatten();

            if let Some(item) = selected {
                // Close microscope first
                state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                    s.close();
                });

                // Return focus and mode to editor
                ctx.emit(RequestFocusChange {
                    target: reovim_core::modd::ComponentId::EDITOR,
                });
                let mode = ModeState::with_interactor_id_and_mode(
                    reovim_core::modd::ComponentId::EDITOR,
                    EditMode::Normal,
                );
                ctx.emit(RequestModeChange { mode });

                // Open the file
                ctx.emit(reovim_core::event_bus::core_events::RequestOpenFile {
                    path: std::path::PathBuf::from(&item.id),
                });

                ctx.request_render();
            }
            EventResult::Handled
        });

        // Handle MicroscopeClearQuery event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeClearQuery, _>(100, move |_event, ctx| {
            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.clear_query();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeDeleteWord event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeDeleteWord, _>(100, move |_event, ctx| {
            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.delete_word();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeCursorLeft event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeCursorLeft, _>(100, move |_event, ctx| {
            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.cursor_left();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeCursorRight event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeCursorRight, _>(100, move |_event, ctx| {
            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.cursor_right();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeCursorStart event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeCursorStart, _>(100, move |_event, ctx| {
            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.cursor_home();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeCursorEnd event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeCursorEnd, _>(100, move |_event, ctx| {
            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.cursor_end();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeWordForward event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeWordForward, _>(100, move |_event, ctx| {
            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.word_forward();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeWordBackward event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeWordBackward, _>(100, move |_event, ctx| {
            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.word_backward();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopePageDown event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopePageDown, _>(100, move |_event, ctx| {
            let picker_name = state_clone
                .with::<MicroscopeState, _, _>(|s| s.picker_name.clone())
                .unwrap_or_default();

            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.page_down();
            });

            load_preview(&state_clone, &picker_name);
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopePageUp event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopePageUp, _>(100, move |_event, ctx| {
            let picker_name = state_clone
                .with::<MicroscopeState, _, _>(|s| s.picker_name.clone())
                .unwrap_or_default();

            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.page_up();
            });

            load_preview(&state_clone, &picker_name);
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeGotoFirst event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeGotoFirst, _>(100, move |_event, ctx| {
            let picker_name = state_clone
                .with::<MicroscopeState, _, _>(|s| s.picker_name.clone())
                .unwrap_or_default();

            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.move_to_first();
            });

            load_preview(&state_clone, &picker_name);
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeGotoLast event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeGotoLast, _>(100, move |_event, ctx| {
            let picker_name = state_clone
                .with::<MicroscopeState, _, _>(|s| s.picker_name.clone())
                .unwrap_or_default();

            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.move_to_last();
            });

            load_preview(&state_clone, &picker_name);
            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeEnterInsert event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeEnterInsert, _>(100, move |_event, ctx| {
            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.enter_insert();
            });

            // Enter Interactor sub-mode so characters route to PluginTextInput handler
            let mode = ModeState::with_interactor_id_sub_mode(
                COMPONENT_ID,
                EditMode::Normal,
                SubMode::Interactor(COMPONENT_ID),
            );
            ctx.emit(RequestModeChange { mode });

            ctx.request_render();
            EventResult::Handled
        });

        // Handle MicroscopeEnterNormal event
        let state_clone = Arc::clone(&state);
        bus.subscribe::<commands::MicroscopeEnterNormal, _>(100, move |_event, ctx| {
            state_clone.with_mut::<MicroscopeState, _, _>(|s| {
                s.enter_normal();
            });

            // Exit Interactor sub-mode, back to normal microscope mode
            let mode = ModeState::with_interactor_id_and_mode(COMPONENT_ID, EditMode::Normal);
            ctx.emit(RequestModeChange { mode });

            ctx.request_render();
            EventResult::Handled
        });
    }
}
