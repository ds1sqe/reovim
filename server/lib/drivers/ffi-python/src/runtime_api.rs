//! Python wrappers for the FFI runtime API.
//!
//! Provides `PyRuntimeApi` -- a Python class that exposes buffer, window,
//! mode, and command operations via the thread-local `RuntimeGuard` bridge.
//!
//! All methods require an active `RuntimeGuard` (must be called during
//! a command callback). Raises `RuntimeError` otherwise.

use pyo3::{exceptions::PyRuntimeError, prelude::*};

use {
    reovim_driver_ffi::with_runtime,
    reovim_driver_session::api::{
        BufferApi, ClipboardApi, CommandApi, ModeApi, RegisterApi, UndoApi, WindowApi,
    },
};

/// Python API for accessing the editor runtime during command callbacks.
///
/// All methods require an active runtime context (must be called during
/// a command callback). Raises `RuntimeError` if no runtime is active.
///
/// # Python Usage
///
/// ```python
/// from reovim import RuntimeApi
///
/// api = RuntimeApi()
///
/// # During a command callback:
/// buf_id = api.active_buffer()
/// line = api.buffer_line(buf_id, 0)
/// api.insert_text(buf_id, 0, 0, "hello")
/// ```
#[pyclass(name = "RuntimeApi", module = "reovim")]
pub struct PyRuntimeApi;

// PyO3 methods require `&self` even when not needed by Rust, and cannot be
// `const`. Suppress these clippy lints for the entire impl block.
#[allow(clippy::unused_self, clippy::missing_const_for_fn)]
#[pymethods]
impl PyRuntimeApi {
    #[new]
    fn new() -> Self {
        Self
    }

    // ========================================================================
    // Buffer API
    // ========================================================================

    /// Get the active buffer ID.
    ///
    /// Returns the buffer ID or `None` if no active buffer.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    fn active_buffer(&self) -> PyResult<Option<u64>> {
        with_runtime(|rt| rt.active_buffer().map(|id| id.as_usize() as u64))
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Get a line from a buffer.
    ///
    /// Returns the line content as a string, or `None` if buffer/line not found.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[pyo3(signature = (buffer_id, line))]
    fn buffer_line(&self, buffer_id: u64, line: u32) -> PyResult<Option<String>> {
        #[allow(clippy::cast_possible_truncation)]
        let bid = reovim_kernel::api::v1::BufferId::from_raw(buffer_id as usize);

        with_runtime(|rt| rt.buffer_line(bid, line as usize))
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Get the number of lines in a buffer.
    ///
    /// Returns the line count, or `None` if buffer not found.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[pyo3(signature = (buffer_id))]
    #[allow(clippy::cast_possible_truncation)]
    fn buffer_line_count(&self, buffer_id: u64) -> PyResult<Option<u32>> {
        let bid = reovim_kernel::api::v1::BufferId::from_raw(buffer_id as usize);

        with_runtime(|rt| rt.buffer_line_count(bid).map(|n| n as u32))
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Insert text at a position in a buffer.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[pyo3(signature = (buffer_id, line, column, text))]
    #[allow(clippy::cast_possible_truncation)]
    fn insert_text(&self, buffer_id: u64, line: u32, column: u32, text: &str) -> PyResult<()> {
        let bid = reovim_kernel::api::v1::BufferId::from_raw(buffer_id as usize);
        let pos = reovim_kernel::api::v1::Position::new(line as usize, column as usize);

        with_runtime(|rt| rt.insert_text(bid, pos, text))
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Delete a range of text from a buffer.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[pyo3(signature = (buffer_id, start_line, start_col, end_line, end_col))]
    #[allow(clippy::cast_possible_truncation)]
    fn delete_range(
        &self,
        buffer_id: u64,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) -> PyResult<()> {
        let bid = reovim_kernel::api::v1::BufferId::from_raw(buffer_id as usize);
        let start = reovim_kernel::api::v1::Position::new(start_line as usize, start_col as usize);
        let end = reovim_kernel::api::v1::Position::new(end_line as usize, end_col as usize);

        with_runtime(|rt| rt.delete_range(bid, start, end))
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Create a new empty buffer.
    ///
    /// Returns the new buffer ID.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    fn create_buffer(&self) -> PyResult<u64> {
        with_runtime(|rt| rt.create_buffer(None, "").as_usize() as u64)
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    // ========================================================================
    // Window API
    // ========================================================================

    /// Get the active window ID.
    ///
    /// Returns the window ID or `None` if no active window.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    fn active_window(&self) -> PyResult<Option<u64>> {
        with_runtime(|rt| rt.active_window().map(|id| id.as_usize() as u64))
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Get the cursor position from the active window.
    ///
    /// Returns `(line, column)` tuple or `None` if no active window.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[allow(clippy::cast_possible_truncation)]
    fn cursor_position(&self) -> PyResult<Option<(u32, u32)>> {
        with_runtime(|rt| {
            rt.cursor_position()
                .map(|pos| (pos.line as u32, pos.column as u32))
        })
        .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Get the number of windows.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[allow(clippy::cast_possible_truncation)]
    fn window_count(&self) -> PyResult<u32> {
        with_runtime(|rt| rt.window_count() as u32)
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    // ========================================================================
    // Mode API
    // ========================================================================

    /// Get the current mode as a string.
    ///
    /// Returns the mode ID string (e.g., `"vim:normal"`).
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    fn current_mode(&self) -> PyResult<String> {
        with_runtime(|rt| rt.current_mode().to_string())
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Get the mode stack depth.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[allow(clippy::cast_possible_truncation)]
    fn mode_depth(&self) -> PyResult<u32> {
        with_runtime(|rt| rt.mode_depth() as u32)
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Push a mode onto the mode stack.
    ///
    /// The `mode_id` must be in `"module:name"` format (e.g., `"vim:insert"`).
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    /// Raises `ValueError` if `mode_id` format is invalid.
    #[pyo3(signature = (mode_id))]
    fn push_mode(&self, mode_id: &str) -> PyResult<()> {
        let Some(mode) = parse_mode_id(mode_id) else {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "mode_id must be in 'module:name' format",
            ));
        };

        with_runtime(|rt| {
            rt.push_mode(mode, reovim_driver_session::TransitionContext::new());
        })
        .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Pop the current mode from the stack.
    ///
    /// Raises `RuntimeError` if called outside a command callback or
    /// if trying to pop the home mode.
    fn pop_mode(&self) -> PyResult<()> {
        with_runtime(|rt| rt.pop_mode(None))
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
            .and_then(|r| r.map_err(|_| PyRuntimeError::new_err("cannot pop home mode")))
    }

    /// Replace the current mode (pop + push atomically).
    ///
    /// The `mode_id` must be in `"module:name"` format.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    /// Raises `ValueError` if `mode_id` format is invalid.
    #[pyo3(signature = (mode_id))]
    fn set_mode(&self, mode_id: &str) -> PyResult<()> {
        let Some(mode) = parse_mode_id(mode_id) else {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "mode_id must be in 'module:name' format",
            ));
        };

        with_runtime(|rt| {
            rt.set_mode(mode, reovim_driver_session::TransitionContext::new());
        })
        .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    // ========================================================================
    // Clipboard API
    // ========================================================================

    /// Copy text to the system clipboard (`+` register).
    ///
    /// Returns `True` if copy succeeded, `False` if clipboard unavailable.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[pyo3(signature = (text))]
    fn copy_to_clipboard(&self, text: &str) -> PyResult<bool> {
        with_runtime(|rt| rt.copy_to_clipboard(text))
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Paste text from the system clipboard (`+` register).
    ///
    /// Returns the clipboard content or `None` if unavailable/empty.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    fn paste_from_clipboard(&self) -> PyResult<Option<String>> {
        with_runtime(|rt| rt.paste_from_clipboard())
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Copy text to the selection clipboard (`*` register, X11 primary selection).
    ///
    /// Returns `True` if copy succeeded, `False` if unavailable.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[pyo3(signature = (text))]
    fn copy_to_selection(&self, text: &str) -> PyResult<bool> {
        with_runtime(|rt| rt.copy_to_selection(text))
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Paste text from the selection clipboard (`*` register).
    ///
    /// Returns the selection content or `None` if unavailable/empty.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    fn paste_from_selection(&self) -> PyResult<Option<String>> {
        with_runtime(|rt| rt.paste_from_selection())
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    // ========================================================================
    // Register API
    // ========================================================================

    /// Get register contents.
    ///
    /// Returns `(text, yank_type)` tuple where `yank_type` is `"characterwise"`
    /// or `"linewise"`. Returns `None` if the register is empty.
    ///
    /// Pass `name=None` or omit for the unnamed register.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[pyo3(signature = (name=None))]
    fn get_register(&self, name: Option<char>) -> PyResult<Option<(String, String)>> {
        with_runtime(|rt| {
            rt.get_register(name).map(|content| {
                let yt_str = if content.is_linewise() {
                    "linewise"
                } else {
                    "characterwise"
                };
                (content.text, yt_str.to_string())
            })
        })
        .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Set register contents.
    ///
    /// `yank_type` must be `"characterwise"` (default) or `"linewise"`.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    /// Raises `ValueError` if `yank_type` is invalid.
    #[pyo3(signature = (text, name=None, yank_type="characterwise"))]
    fn set_register(&self, text: &str, name: Option<char>, yank_type: &str) -> PyResult<()> {
        let yt = match yank_type {
            "characterwise" => reovim_kernel::api::v1::YankType::Characterwise,
            "linewise" => reovim_kernel::api::v1::YankType::Linewise,
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "yank_type must be 'characterwise' or 'linewise'",
                ));
            }
        };
        let content = reovim_kernel::api::v1::RegisterContent::new(text, yt);

        with_runtime(|rt| rt.set_register(name, content))
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    // ========================================================================
    // Undo API
    // ========================================================================

    /// Undo the last change for a buffer.
    ///
    /// Returns cursor position `(line, column)` or `None` if nothing to undo.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[pyo3(signature = (buffer_id))]
    #[allow(clippy::cast_possible_truncation)]
    fn undo(&self, buffer_id: u64) -> PyResult<Option<(u32, u32)>> {
        let bid = reovim_kernel::api::v1::BufferId::from_raw(buffer_id as usize);

        with_runtime(|rt| {
            rt.undo(bid)
                .map(|result| (result.cursor.line as u32, result.cursor.column as u32))
        })
        .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Redo the last undone change for a buffer.
    ///
    /// Returns cursor position `(line, column)` or `None` if nothing to redo.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[pyo3(signature = (buffer_id))]
    #[allow(clippy::cast_possible_truncation)]
    fn redo(&self, buffer_id: u64) -> PyResult<Option<(u32, u32)>> {
        let bid = reovim_kernel::api::v1::BufferId::from_raw(buffer_id as usize);

        with_runtime(|rt| {
            rt.redo(bid)
                .map(|result| (result.cursor.line as u32, result.cursor.column as u32))
        })
        .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Check if undo is available for a buffer.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[pyo3(signature = (buffer_id))]
    #[allow(clippy::cast_possible_truncation)]
    fn can_undo(&self, buffer_id: u64) -> PyResult<bool> {
        let bid = reovim_kernel::api::v1::BufferId::from_raw(buffer_id as usize);

        with_runtime(|rt| rt.can_undo(bid))
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    /// Check if redo is available for a buffer.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[pyo3(signature = (buffer_id))]
    #[allow(clippy::cast_possible_truncation)]
    fn can_redo(&self, buffer_id: u64) -> PyResult<bool> {
        let bid = reovim_kernel::api::v1::BufferId::from_raw(buffer_id as usize);

        with_runtime(|rt| rt.can_redo(bid))
            .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }

    // ========================================================================
    // Command API
    // ========================================================================

    /// Execute a command by ID string.
    ///
    /// Returns the command result as a string: `"success"`, `"quit"`,
    /// or `"error: ..."`.
    ///
    /// Raises `RuntimeError` if called outside a command callback.
    #[pyo3(signature = (cmd_id, count=None, register=None))]
    fn execute_command(
        &self,
        cmd_id: &str,
        count: Option<usize>,
        register: Option<char>,
    ) -> PyResult<String> {
        let command_id =
            reovim_kernel::api::v1::CommandId::from_qualified_leaked(cmd_id.to_string());

        let mut ctx = reovim_driver_command_types::CommandContext::new();
        if let Some(n) = count {
            ctx.set("count", reovim_driver_command_types::ArgValue::Count(n));
        }
        if let Some(r) = register {
            ctx.set("register", reovim_driver_command_types::ArgValue::Register(r));
        }

        with_runtime(|rt| {
            let result = rt.execute_command(command_id, ctx);
            let signals = rt.take_signals();
            if signals
                .iter()
                .any(|s| matches!(s, reovim_driver_command_types::RuntimeSignal::Quit))
            {
                "quit".to_string()
            } else {
                match result {
                    reovim_driver_command_types::CommandResult::Success => "success".to_string(),
                    reovim_driver_command_types::CommandResult::Error(e) => format!("error: {e}"),
                }
            }
        })
        .map_err(|_| PyRuntimeError::new_err("no active runtime"))
    }
}

/// Parse a `"module:name"` mode ID string into a `ModeId`.
fn parse_mode_id(s: &str) -> Option<reovim_kernel::api::v1::ModeId> {
    let (module_str, name_str) = s.split_once(':')?;
    if module_str.is_empty() || name_str.is_empty() {
        return None;
    }

    let module = reovim_kernel::api::v1::ModuleId::from_string(module_str.to_string());
    // SAFETY: intentional leak -- mode names live for program lifetime
    let name: &'static str = Box::leak(name_str.to_string().into_boxed_str());
    Some(reovim_kernel::api::v1::ModeId::new(module, name))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "runtime_api_tests.rs"]
mod tests;
