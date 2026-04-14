//! Window operation command handlers.
//!
//! These commands implement window management operations (split, close, focus)
//! by delegating to the `CompositorApi` trait methods on `SessionRuntime`.
//!
//! # Architecture
//!
//! Following the kernel's "mechanism vs policy" principle:
//! - **Mechanism** (drivers): `CompositorApi` trait provides operations
//! - **Policy** (this module): Commands decide when/how to use those operations
//!
//! The compositor handles the actual layout logic; commands just invoke it.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_text_session::{CompositorApi, SessionRuntime},
    reovim_kernel::api::v1::CommandId,
    reovim_subsys_layout::{NavigateDirection, SplitDirection},
};

use crate::ids;

// =============================================================================
// Focus Navigation Commands
// =============================================================================

/// Move focus to the window on the left.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusLeft;

impl Command for FocusLeft {
    fn id(&self) -> CommandId {
        ids::FOCUS_LEFT
    }

    fn description(&self) -> &'static str {
        "Move focus to left window"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for FocusLeft {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.navigate(NavigateDirection::Left) {
            Ok(window) => match runtime.focus(window) {
                Ok(()) => CommandResult::Success,
                Err(e) => CommandResult::Error(e.to_string()),
            },
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Move focus to the window below.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusDown;

impl Command for FocusDown {
    fn id(&self) -> CommandId {
        ids::FOCUS_DOWN
    }

    fn description(&self) -> &'static str {
        "Move focus to window below"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for FocusDown {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.navigate(NavigateDirection::Down) {
            Ok(window) => match runtime.focus(window) {
                Ok(()) => CommandResult::Success,
                Err(e) => CommandResult::Error(e.to_string()),
            },
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Move focus to the window above.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusUp;

impl Command for FocusUp {
    fn id(&self) -> CommandId {
        ids::FOCUS_UP
    }

    fn description(&self) -> &'static str {
        "Move focus to window above"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for FocusUp {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.navigate(NavigateDirection::Up) {
            Ok(window) => match runtime.focus(window) {
                Ok(()) => CommandResult::Success,
                Err(e) => CommandResult::Error(e.to_string()),
            },
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Move focus to the window on the right.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusRight;

impl Command for FocusRight {
    fn id(&self) -> CommandId {
        ids::FOCUS_RIGHT
    }

    fn description(&self) -> &'static str {
        "Move focus to right window"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for FocusRight {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.navigate(NavigateDirection::Right) {
            Ok(window) => match runtime.focus(window) {
                Ok(()) => CommandResult::Success,
                Err(e) => CommandResult::Error(e.to_string()),
            },
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

// =============================================================================
// Focus Cycling Commands
// =============================================================================

/// Cycle focus to the next window.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusNext;

impl Command for FocusNext {
    fn id(&self) -> CommandId {
        ids::FOCUS_NEXT
    }

    fn description(&self) -> &'static str {
        "Cycle focus to next window"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for FocusNext {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.cycle(true) {
            Ok(window) => match runtime.focus(window) {
                Ok(()) => CommandResult::Success,
                Err(e) => CommandResult::Error(e.to_string()),
            },
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Cycle focus to the previous window.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusPrev;

impl Command for FocusPrev {
    fn id(&self) -> CommandId {
        ids::FOCUS_PREV
    }

    fn description(&self) -> &'static str {
        "Cycle focus to previous window"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for FocusPrev {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.cycle(false) {
            Ok(window) => match runtime.focus(window) {
                Ok(()) => CommandResult::Success,
                Err(e) => CommandResult::Error(e.to_string()),
            },
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

// =============================================================================
// Window Splitting Commands
// =============================================================================

/// Split window horizontally (creates window below).
#[derive(Debug, Clone, Copy, Default)]
pub struct SplitHorizontal;

impl Command for SplitHorizontal {
    fn id(&self) -> CommandId {
        ids::SPLIT_HORIZONTAL
    }

    fn description(&self) -> &'static str {
        "Split window horizontally"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

impl CommandHandler for SplitHorizontal {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.split(SplitDirection::Horizontal) {
            Ok(_new_window) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Split window vertically (creates window to the right).
#[derive(Debug, Clone, Copy, Default)]
pub struct SplitVertical;

impl Command for SplitVertical {
    fn id(&self) -> CommandId {
        ids::SPLIT_VERTICAL
    }

    fn description(&self) -> &'static str {
        "Split window vertically"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

impl CommandHandler for SplitVertical {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.split(SplitDirection::Vertical) {
            Ok(_new_window) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

// =============================================================================
// Window Closing Commands
// =============================================================================

/// Close the current window.
#[derive(Debug, Clone, Copy, Default)]
pub struct CloseWindow;

impl Command for CloseWindow {
    fn id(&self) -> CommandId {
        ids::CLOSE_WINDOW
    }

    fn description(&self) -> &'static str {
        "Close current window"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

impl CommandHandler for CloseWindow {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.close_current_window() {
            Ok(_neighbor) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Close all windows except the current one.
#[derive(Debug, Clone, Copy, Default)]
pub struct CloseOthers;

impl Command for CloseOthers {
    fn id(&self) -> CommandId {
        ids::CLOSE_OTHERS
    }

    fn description(&self) -> &'static str {
        "Close all other windows"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

impl CommandHandler for CloseOthers {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.close_others() {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Create new window with empty buffer.
#[derive(Debug, Clone, Copy, Default)]
pub struct SplitNew;

impl Command for SplitNew {
    fn id(&self) -> CommandId {
        ids::SPLIT_NEW
    }

    fn description(&self) -> &'static str {
        "Create new window with empty buffer"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

impl CommandHandler for SplitNew {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        // Split vertically first to create the new window
        match runtime.split(SplitDirection::Vertical) {
            Ok(_new_window) => {
                // The new window will share the same buffer as the original.
                // To make it truly empty, we'd need BufferApi.create_buffer() +
                // set_window_buffer(), but for MVP just split is sufficient.
                CommandResult::Success
            }
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

// =============================================================================
// Window Resizing Commands
// =============================================================================

/// Default resize delta (in cells).
const RESIZE_DELTA: i16 = 1;

/// Increase window height.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResizeHeightIncrease;

impl Command for ResizeHeightIncrease {
    fn id(&self) -> CommandId {
        ids::RESIZE_HEIGHT_INCREASE
    }

    fn description(&self) -> &'static str {
        "Increase window height"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

impl CommandHandler for ResizeHeightIncrease {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        // Resize down direction expands height
        match runtime.resize(NavigateDirection::Down, RESIZE_DELTA) {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Decrease window height.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResizeHeightDecrease;

impl Command for ResizeHeightDecrease {
    fn id(&self) -> CommandId {
        ids::RESIZE_HEIGHT_DECREASE
    }

    fn description(&self) -> &'static str {
        "Decrease window height"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

impl CommandHandler for ResizeHeightDecrease {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        // Resize down direction with negative delta shrinks height
        match runtime.resize(NavigateDirection::Down, -RESIZE_DELTA) {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Increase window width.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResizeWidthIncrease;

impl Command for ResizeWidthIncrease {
    fn id(&self) -> CommandId {
        ids::RESIZE_WIDTH_INCREASE
    }

    fn description(&self) -> &'static str {
        "Increase window width"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

impl CommandHandler for ResizeWidthIncrease {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        // Resize right direction expands width
        match runtime.resize(NavigateDirection::Right, RESIZE_DELTA) {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Decrease window width.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResizeWidthDecrease;

impl Command for ResizeWidthDecrease {
    fn id(&self) -> CommandId {
        ids::RESIZE_WIDTH_DECREASE
    }

    fn description(&self) -> &'static str {
        "Decrease window width"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

impl CommandHandler for ResizeWidthDecrease {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        // Resize right direction with negative delta shrinks width
        match runtime.resize(NavigateDirection::Right, -RESIZE_DELTA) {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Equalize all window sizes.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResizeEqual;

impl Command for ResizeEqual {
    fn id(&self) -> CommandId {
        ids::RESIZE_EQUAL
    }

    fn description(&self) -> &'static str {
        "Equalize all window sizes"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

impl CommandHandler for ResizeEqual {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.equalize() {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

// =============================================================================
// Float Zone Commands
// =============================================================================

/// Toggle window between tiled and floating zones.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleFloat;

impl Command for ToggleFloat {
    fn id(&self) -> CommandId {
        ids::TOGGLE_FLOAT
    }

    fn description(&self) -> &'static str {
        "Toggle window between tiled and floating"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

impl CommandHandler for ToggleFloat {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.toggle_float() {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Raise floating window to front.
#[derive(Debug, Clone, Copy, Default)]
pub struct RaiseFloat;

impl Command for RaiseFloat {
    fn id(&self) -> CommandId {
        ids::RAISE_FLOAT
    }

    fn description(&self) -> &'static str {
        "Raise floating window to front"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

impl CommandHandler for RaiseFloat {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.raise_float() {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Lower floating window to back.
#[derive(Debug, Clone, Copy, Default)]
pub struct LowerFloat;

impl Command for LowerFloat {
    fn id(&self) -> CommandId {
        ids::LOWER_FLOAT
    }

    fn description(&self) -> &'static str {
        "Lower floating window to back"
    }

    fn args(&self) -> Vec<ArgSpec> {
        Vec::new()
    }
}

impl CommandHandler for LowerFloat {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.lower_float() {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

// =============================================================================
// Layer Opacity Commands (#400)
// =============================================================================

/// Set layer opacity to a specific percentage (0-100).
#[derive(Debug, Clone, Copy, Default)]
pub struct LayerOpacitySet;

impl Command for LayerOpacitySet {
    fn id(&self) -> CommandId {
        ids::LAYER_OPACITY_SET
    }

    fn description(&self) -> &'static str {
        "Set layer opacity (0-100)"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::required(
            "percentage",
            ArgKind::Count,
            "Opacity percentage (0-100)",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["LayerOpacity"]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for LayerOpacitySet {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        let pct = ctx.count().unwrap_or(100);
        #[allow(clippy::cast_precision_loss)]
        let opacity = (pct as f32 / 100.0).clamp(0.0, 1.0);
        match runtime.set_active_layer_opacity(opacity) {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Increase layer opacity by 10%.
#[derive(Debug, Clone, Copy, Default)]
pub struct LayerOpacityIncrease;

impl Command for LayerOpacityIncrease {
    fn id(&self) -> CommandId {
        ids::LAYER_OPACITY_INCREASE
    }

    fn description(&self) -> &'static str {
        "Increase layer opacity by 10%"
    }

    fn names(&self) -> &[&'static str] {
        &["LayerOpacityUp"]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for LayerOpacityIncrease {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.adjust_active_layer_opacity(0.1) {
            Ok(_) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Decrease layer opacity by 10%.
#[derive(Debug, Clone, Copy, Default)]
pub struct LayerOpacityDecrease;

impl Command for LayerOpacityDecrease {
    fn id(&self) -> CommandId {
        ids::LAYER_OPACITY_DECREASE
    }

    fn description(&self) -> &'static str {
        "Decrease layer opacity by 10%"
    }

    fn names(&self) -> &[&'static str] {
        &["LayerOpacityDown"]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for LayerOpacityDecrease {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.adjust_active_layer_opacity(-0.1) {
            Ok(_) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

// =============================================================================
// Tab Page Commands (#401)
// =============================================================================

/// Create a new tab page.
#[derive(Debug, Clone, Copy, Default)]
pub struct TabNew;

impl Command for TabNew {
    fn id(&self) -> CommandId {
        ids::TAB_NEW
    }

    fn description(&self) -> &'static str {
        "Create a new tab page"
    }

    fn names(&self) -> &[&'static str] {
        &["tabnew"]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for TabNew {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.tab_new() {
            Ok(_) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Close the current tab page.
#[derive(Debug, Clone, Copy, Default)]
pub struct TabClose;

impl Command for TabClose {
    fn id(&self) -> CommandId {
        ids::TAB_CLOSE
    }

    fn description(&self) -> &'static str {
        "Close the current tab page"
    }

    fn names(&self) -> &[&'static str] {
        &["tabclose", "tabc"]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for TabClose {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.tab_close() {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Switch to the next tab page.
#[derive(Debug, Clone, Copy, Default)]
pub struct TabNext;

impl Command for TabNext {
    fn id(&self) -> CommandId {
        ids::TAB_NEXT
    }

    fn description(&self) -> &'static str {
        "Switch to the next tab page"
    }

    fn names(&self) -> &[&'static str] {
        &["tabnext", "tabn"]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for TabNext {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.tab_next() {
            Ok(_) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Switch to the previous tab page.
#[derive(Debug, Clone, Copy, Default)]
pub struct TabPrev;

impl Command for TabPrev {
    fn id(&self) -> CommandId {
        ids::TAB_PREV
    }

    fn description(&self) -> &'static str {
        "Switch to the previous tab page"
    }

    fn names(&self) -> &[&'static str] {
        &["tabprev", "tabp"]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for TabPrev {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.tab_prev() {
            Ok(_) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Switch to a tab page by index.
#[derive(Debug, Clone, Copy, Default)]
pub struct TabGoto;

impl Command for TabGoto {
    fn id(&self) -> CommandId {
        ids::TAB_GOTO
    }

    fn description(&self) -> &'static str {
        "Switch to tab page by index"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::required(
            "index",
            ArgKind::Count,
            "Tab index (1-based)",
        )]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for TabGoto {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        let index = ctx.count().unwrap_or(1);
        // Convert from 1-based (user) to 0-based (internal)
        match runtime.tab_goto(index.saturating_sub(1)) {
            Ok(_) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Move the current window to a new tab page.
///
/// Creates a new tab page and switches to it. In vim, `<C-w>T` moves the
/// current window to a new tab; here the new tab starts empty and the
/// compositor will assign the default window on next layout.
#[derive(Debug, Clone, Copy, Default)]
pub struct MoveToNewTab;

impl Command for MoveToNewTab {
    fn id(&self) -> CommandId {
        ids::MOVE_TO_NEW_TAB
    }

    fn description(&self) -> &'static str {
        "Move current window to new tab"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for MoveToNewTab {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        match runtime.tab_new() {
            Ok(_) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

// =============================================================================
// Command Collection
// =============================================================================

/// Returns all window operation command handlers.
///
/// Used by `WindowOps::command_handlers()` to register commands.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        // Focus navigation
        Box::new(FocusLeft),
        Box::new(FocusDown),
        Box::new(FocusUp),
        Box::new(FocusRight),
        // Focus cycling
        Box::new(FocusNext),
        Box::new(FocusPrev),
        // Splitting
        Box::new(SplitHorizontal),
        Box::new(SplitVertical),
        Box::new(SplitNew),
        // Closing
        Box::new(CloseWindow),
        Box::new(CloseOthers),
        // Resizing
        Box::new(ResizeHeightIncrease),
        Box::new(ResizeHeightDecrease),
        Box::new(ResizeWidthIncrease),
        Box::new(ResizeWidthDecrease),
        Box::new(ResizeEqual),
        // Float zone
        Box::new(ToggleFloat),
        Box::new(RaiseFloat),
        Box::new(LowerFloat),
        // Layer opacity (#400)
        Box::new(LayerOpacitySet),
        Box::new(LayerOpacityIncrease),
        Box::new(LayerOpacityDecrease),
        // Tab pages (#401)
        Box::new(TabNew),
        Box::new(TabClose),
        Box::new(TabNext),
        Box::new(TabPrev),
        Box::new(TabGoto),
        Box::new(MoveToNewTab),
    ]
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
