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
    reovim_driver_session::api::{BufferApi, CommandApi, ModeApi, WindowApi},
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
    // Command API
    // ========================================================================

    /// Execute a command by ID string.
    ///
    /// Returns the command result as a string: `"success"`, `"quit"`,
    /// `"force_quit"`, `"detach"`, or `"error: ..."`.
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

        with_runtime(|rt| match rt.execute_command(command_id, ctx) {
            reovim_driver_command_types::CommandResult::Success => "success".to_string(),
            reovim_driver_command_types::CommandResult::Quit => "quit".to_string(),
            reovim_driver_command_types::CommandResult::ForceQuit => "force_quit".to_string(),
            reovim_driver_command_types::CommandResult::Detach => "detach".to_string(),
            reovim_driver_command_types::CommandResult::Error(e) => format!("error: {e}"),
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
mod tests {
    use super::*;

    #[test]
    fn test_py_runtime_api_creation() {
        let _api = PyRuntimeApi::new();
    }

    #[test]
    fn test_active_buffer_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.active_buffer().is_err());
    }

    #[test]
    fn test_buffer_line_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.buffer_line(1, 0).is_err());
    }

    #[test]
    fn test_buffer_line_count_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.buffer_line_count(1).is_err());
    }

    #[test]
    fn test_insert_text_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.insert_text(1, 0, 0, "hello").is_err());
    }

    #[test]
    fn test_delete_range_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.delete_range(1, 0, 0, 0, 5).is_err());
    }

    #[test]
    fn test_create_buffer_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.create_buffer().is_err());
    }

    #[test]
    fn test_active_window_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.active_window().is_err());
    }

    #[test]
    fn test_cursor_position_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.cursor_position().is_err());
    }

    #[test]
    fn test_window_count_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.window_count().is_err());
    }

    #[test]
    fn test_current_mode_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.current_mode().is_err());
    }

    #[test]
    fn test_mode_depth_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.mode_depth().is_err());
    }

    #[test]
    fn test_push_mode_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.push_mode("vim:insert").is_err());
    }

    #[test]
    fn test_push_mode_invalid_format() {
        let api = PyRuntimeApi;
        assert!(api.push_mode("nocolon").is_err());
    }

    #[test]
    fn test_push_mode_empty_module() {
        let api = PyRuntimeApi;
        assert!(api.push_mode(":name").is_err());
    }

    #[test]
    fn test_push_mode_empty_name() {
        let api = PyRuntimeApi;
        assert!(api.push_mode("module:").is_err());
    }

    #[test]
    fn test_pop_mode_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.pop_mode().is_err());
    }

    #[test]
    fn test_set_mode_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.set_mode("vim:visual").is_err());
    }

    #[test]
    fn test_set_mode_invalid_format() {
        let api = PyRuntimeApi;
        assert!(api.set_mode("nocolon").is_err());
    }

    #[test]
    fn test_execute_command_no_runtime() {
        let api = PyRuntimeApi;
        assert!(api.execute_command("vim:delete", None, None).is_err());
    }

    #[test]
    fn test_parse_mode_id_valid() {
        let mode = parse_mode_id("vim:normal").unwrap();
        assert_eq!(mode.module().as_str(), "vim");
        assert_eq!(mode.name(), "normal");
    }

    #[test]
    fn test_parse_mode_id_no_colon() {
        assert!(parse_mode_id("nocolon").is_none());
    }

    #[test]
    fn test_parse_mode_id_empty_parts() {
        assert!(parse_mode_id(":name").is_none());
        assert!(parse_mode_id("module:").is_none());
    }

    #[test]
    fn test_parse_mode_id_multiple_colons() {
        let mode = parse_mode_id("a:b:c").unwrap();
        assert_eq!(mode.module().as_str(), "a");
        assert_eq!(mode.name(), "b:c");
    }
}
