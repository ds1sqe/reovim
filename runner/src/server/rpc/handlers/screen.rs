//! Screen content RPC handler.
//!
//! Handler for `state/screen_content` method.
//!
//! Note: The headless server doesn't have an actual terminal screen.
//! This handler returns the content of the active buffer formatted
//! according to the requested format.
//!
//! # Multi-Window Support
//!
//! When multiple windows are present, this handler composes their content
//! into a single screen representation, with separators between windows.

use {
    reovim_driver_display::WindowView,
    reovim_kernel::api::v1::BufferId,
    reovim_protocol::v1::{RpcError, ScreenContentResult, ScreenFormat, StateScreenContentParams},
};

use super::super::dispatcher::{HandlerFuture, RpcContext};

/// Vertical separator character for window borders.
const VSEP: char = '│';

/// Horizontal separator character for window borders.
const HSEP: char = '─';

/// Handler for `state/screen_content` method.
///
/// Returns screen content in requested format (`plain_text`, `raw_ansi`, `cell_grid`).
///
/// The handler supports multi-window rendering:
/// - Gets window views from the window registry
/// - Renders each window's buffer content within its bounds
/// - Adds separators between windows
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "state/screen_content", "params": {"format": "plain_text"}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"width": 80, "height": 24, "format": "plain_text", "content": "..."}}
/// ```
///
/// # Panics
///
/// This function will not panic as `ScreenContentResult` serialization is infallible.
#[must_use]
pub fn state_screen_content(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: StateScreenContentParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        let result = ctx
            .session
            .with_state(|state| {
                let width = state.app.terminal_width;
                let height = state.app.terminal_height;

                // Get window views from registry
                let window_views = state.app.windows.arrange((width, height));

                // If no windows or single window, fall back to simple single-buffer render
                if window_views.len() <= 1 {
                    return render_single_window(state, params.format, width, height);
                }

                // Multi-window render
                render_multi_window(state, &window_views, params.format, width, height)
            })
            .await;

        Ok(serde_json::to_value(result).expect("ScreenContentResult serialization cannot fail"))
    })
}

/// Render screen with a single window or no windows.
///
/// For backward compatibility, single-window mode returns dimensions
/// based on buffer content, not terminal size.
fn render_single_window(
    state: &crate::session::SessionState,
    format: ScreenFormat,
    _width: u16,
    _height: u16,
) -> ScreenContentResult {
    let content = state
        .app
        .active_buffer
        .and_then(|id: BufferId| state.app.kernel.buffers.get(id))
        .map(|arc| {
            let buf = arc.read();
            buf.content()
        })
        .unwrap_or_default();

    // Calculate dimensions from content (backward compatible behavior)
    let lines: Vec<&str> = content.lines().collect();
    #[allow(clippy::cast_possible_truncation)]
    let content_height = lines.len().min(usize::from(u16::MAX)) as u16;
    #[allow(clippy::cast_possible_truncation)]
    let content_width = lines
        .iter()
        .map(|l| l.len())
        .max()
        .unwrap_or(80)
        .min(usize::from(u16::MAX)) as u16;

    format_content(&content, format, content_width.max(1), content_height.max(1))
}

/// Render screen with multiple windows.
fn render_multi_window(
    state: &crate::session::SessionState,
    views: &[WindowView],
    format: ScreenFormat,
    width: u16,
    height: u16,
) -> ScreenContentResult {
    // Build a character grid for the screen
    let w = usize::from(width);
    let h = usize::from(height);
    let mut grid: Vec<Vec<char>> = vec![vec![' '; w]; h];

    // Track which window is active for potential highlighting
    let active_window = state.app.windows.active_window();

    // Render each window's content into the grid
    for view in views {
        let buffer_content = state
            .app
            .windows
            .get(view.window_id)
            .and_then(|ws| ws.buffer_id)
            .and_then(|id| state.app.kernel.buffers.get(id))
            .map(|arc| {
                let buf = arc.read();
                buf.content()
            })
            .unwrap_or_default();

        let lines: Vec<&str> = buffer_content.lines().collect();
        let bounds = &view.bounds;

        // Render buffer lines into the window area
        for (row_idx, line) in lines.iter().take(bounds.height as usize).enumerate() {
            let y = bounds.y as usize + row_idx;
            if y >= h {
                break;
            }

            for (col_idx, ch) in line.chars().take(bounds.width as usize).enumerate() {
                let x = bounds.x as usize + col_idx;
                if x >= w {
                    break;
                }
                grid[y][x] = ch;
            }
        }
    }

    // Find separator positions (where windows meet)
    let separators = find_separators(views, width, height);

    // Draw separators
    for (x, y, is_vertical) in separators {
        let ux = x as usize;
        let uy = y as usize;
        if ux < w && uy < h {
            grid[uy][ux] = if is_vertical { VSEP } else { HSEP };
        }
    }

    // Add active window indicator (mark corners with different char)
    if let Some(view) = active_window.and_then(|id| views.iter().find(|v| v.window_id == id)) {
        let bounds = &view.bounds;
        // Mark top-left corner of active window (if within bounds)
        let corner_x = bounds.x.saturating_sub(1) as usize;
        let corner_y = bounds.y.saturating_sub(1) as usize;
        if corner_x < w && corner_y < h && (bounds.x > 0 || bounds.y > 0) {
            // Use a special char to mark active window corner
            if grid[corner_y][corner_x] == VSEP || grid[corner_y][corner_x] == HSEP {
                grid[corner_y][corner_x] = '┼';
            }
        }
    }

    // Convert grid to string
    let content: String = grid
        .iter()
        .map(|row| row.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    format_content(&content, format, width, height)
}

/// Find separator positions between windows.
///
/// Returns a list of (x, y, `is_vertical`) tuples indicating where separators should be drawn.
fn find_separators(views: &[WindowView], width: u16, height: u16) -> Vec<(u16, u16, bool)> {
    let mut separators = Vec::new();

    for i in 0..views.len() {
        for j in (i + 1)..views.len() {
            let a = &views[i].bounds;
            let b = &views[j].bounds;

            // Check for vertical separator (windows side by side)
            // Window A is to the left of Window B
            if a.x + a.width == b.x && a.y < b.y + b.height && b.y < a.y + a.height {
                let sep_x = a.x + a.width;
                if sep_x > 0 && sep_x <= width {
                    let y_start = a.y.max(b.y);
                    let y_end = (a.y + a.height).min(b.y + b.height).min(height);
                    for y in y_start..y_end {
                        separators.push((sep_x - 1, y, true));
                    }
                }
            }
            // Window B is to the left of Window A
            if b.x + b.width == a.x && b.y < a.y + a.height && a.y < b.y + b.height {
                let sep_x = b.x + b.width;
                if sep_x > 0 && sep_x <= width {
                    let y_start = a.y.max(b.y);
                    let y_end = (a.y + a.height).min(b.y + b.height).min(height);
                    for y in y_start..y_end {
                        separators.push((sep_x - 1, y, true));
                    }
                }
            }

            // Check for horizontal separator (windows stacked)
            // Window A is above Window B
            if a.y + a.height == b.y && a.x < b.x + b.width && b.x < a.x + a.width {
                let sep_y = a.y + a.height;
                if sep_y > 0 && sep_y <= height {
                    let x_start = a.x.max(b.x);
                    let x_end = (a.x + a.width).min(b.x + b.width).min(width);
                    for x in x_start..x_end {
                        separators.push((x, sep_y - 1, false));
                    }
                }
            }
            // Window B is above Window A
            if b.y + b.height == a.y && b.x < a.x + a.width && a.x < b.x + b.width {
                let sep_y = b.y + b.height;
                if sep_y > 0 && sep_y <= height {
                    let x_start = a.x.max(b.x);
                    let x_end = (a.x + a.width).min(b.x + b.width).min(width);
                    for x in x_start..x_end {
                        separators.push((x, sep_y - 1, false));
                    }
                }
            }
        }
    }

    separators
}

/// Format content according to the requested screen format.
fn format_content(
    content: &str,
    format: ScreenFormat,
    width: u16,
    height: u16,
) -> ScreenContentResult {
    let formatted_content = match format {
        ScreenFormat::PlainText => content.to_string(),
        ScreenFormat::RawAnsi => {
            // For headless server, just return plain text
            // Real ANSI would require a terminal emulator
            content.to_string()
        }
        ScreenFormat::CellGrid => {
            // Return JSON cell grid representation
            let lines: Vec<&str> = content.lines().collect();
            let cells: Vec<Vec<serde_json::Value>> = lines
                .iter()
                .map(|line| {
                    line.chars()
                        .map(|c| {
                            serde_json::json!({
                                "char": c.to_string(),
                                "fg": "default",
                                "bg": "default"
                            })
                        })
                        .collect()
                })
                .collect();
            serde_json::to_string(&cells).unwrap_or_else(|_| "[]".to_string())
        }
    };

    ScreenContentResult {
        width: width.max(1),
        height: height.max(1),
        format,
        content: formatted_content,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use {
        super::*,
        crate::{
            server::rpc::handlers::test_utils::test_ctx_with_session,
            session::{Session, SessionId},
        },
        reovim_arch::sync::RwLock,
        reovim_driver_vfs::{MockVfs, VfsDriver},
        reovim_kernel::api::v1::{
            Buffer, BufferError, BufferManager, KernelContext, ModeId, ModuleId,
        },
        std::collections::HashMap,
    };

    fn test_vfs() -> Arc<dyn VfsDriver> {
        Arc::new(MockVfs::new())
    }

    /// Test-only buffer manager that actually stores buffers.
    struct TestBufferManager {
        buffers: RwLock<HashMap<BufferId, Arc<RwLock<Buffer>>>>,
    }

    impl TestBufferManager {
        fn new() -> Self {
            Self {
                buffers: RwLock::new(HashMap::new()),
            }
        }
    }

    impl BufferManager for TestBufferManager {
        fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
            self.buffers.read().get(&id).cloned()
        }

        fn create(&self) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(Buffer::new()));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn register(&self, buffer: Buffer) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(buffer));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn unregister(&self, id: BufferId) -> Result<Buffer, BufferError> {
            self.buffers
                .write()
                .remove(&id)
                .map_or(Err(BufferError::NotFound(id)), |arc_buffer| {
                    Arc::try_unwrap(arc_buffer)
                        .map_or_else(|arc| Ok(arc.read().clone()), |rwlock| Ok(rwlock.into_inner()))
                })
        }

        fn list(&self) -> Vec<BufferId> {
            self.buffers.read().keys().copied().collect()
        }

        fn count(&self) -> usize {
            self.buffers.read().len()
        }
    }

    /// Create a `KernelContext` with a real buffer manager for testing.
    fn test_kernel() -> KernelContext {
        use reovim_kernel::api::v1::{
            EventBus, MarkBank, MotionEngine, OptionRegistry, RegisterBank, TextObjectEngine,
        };

        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(RegisterBank::new())),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
        )
    }

    fn test_session() -> Arc<Session> {
        Session::new(
            SessionId::new("test"),
            test_kernel(),
            ModeId::new(ModuleId::new("test"), "normal"),
            test_vfs(),
        )
    }

    /// Create a test session with a buffer containing the given content.
    async fn test_session_with_buffer(content: &str) -> Arc<Session> {
        let session = test_session();
        let content_owned = content.to_string();
        session
            .with_state_mut(|state| {
                let buffer = Buffer::from_string(&content_owned);
                let id = state.app.kernel.buffers.register(buffer);
                state.app.active_buffer = Some(id);
            })
            .await;
        session
    }

    #[tokio::test]
    async fn test_screen_content_default_format() {
        let session = test_session();
        let ctx = test_ctx_with_session(session);

        // Default format (plain_text)
        let result = state_screen_content(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("width").is_some());
        assert!(value.get("height").is_some());
        assert!(value.get("format").is_some());
        assert!(value.get("content").is_some());
    }

    #[tokio::test]
    async fn test_screen_content_cell_grid() {
        let session = test_session();
        let ctx = test_ctx_with_session(session);

        let result = state_screen_content(ctx, serde_json::json!({"format": "cell_grid"})).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value.get("format").unwrap(), "cell_grid");
    }

    #[tokio::test]
    async fn test_screen_content_with_buffer() {
        let session = test_session_with_buffer("Hello\nWorld\nTest").await;
        let ctx = test_ctx_with_session(session);

        let result = state_screen_content(ctx, serde_json::json!({"format": "plain_text"})).await;
        assert!(result.is_ok());
        let value = result.unwrap();

        // Verify dimensions match buffer content
        assert_eq!(value.get("height").unwrap().as_u64().unwrap(), 3);
        assert_eq!(value.get("width").unwrap().as_u64().unwrap(), 5); // "World" is longest

        // Verify content
        let content = value.get("content").unwrap().as_str().unwrap();
        assert!(content.contains("Hello"));
        assert!(content.contains("World"));
    }

    #[tokio::test]
    async fn test_screen_content_cell_grid_structure() {
        let session = test_session_with_buffer("AB\nCD").await;
        let ctx = test_ctx_with_session(session);

        let result = state_screen_content(ctx, serde_json::json!({"format": "cell_grid"})).await;
        assert!(result.is_ok());
        let value = result.unwrap();

        // Cell grid content should be valid JSON
        let content = value.get("content").unwrap().as_str().unwrap();
        let cells: Vec<Vec<serde_json::Value>> =
            serde_json::from_str(content).expect("Cell grid should be valid JSON");

        // Should have 2 rows
        assert_eq!(cells.len(), 2);

        // First row should have 2 cells (A, B)
        assert_eq!(cells[0].len(), 2);
        assert_eq!(cells[0][0].get("char").unwrap().as_str().unwrap(), "A");
        assert_eq!(cells[0][1].get("char").unwrap().as_str().unwrap(), "B");

        // Second row should have 2 cells (C, D)
        assert_eq!(cells[1].len(), 2);
        assert_eq!(cells[1][0].get("char").unwrap().as_str().unwrap(), "C");
        assert_eq!(cells[1][1].get("char").unwrap().as_str().unwrap(), "D");
    }

    // ========================================================================
    // Multi-Window Rendering Tests
    // ========================================================================

    #[test]
    fn test_find_separators_vertical_split() {
        use reovim_driver_display::{Rect, WindowId, WindowView};

        // Two windows side by side (vertical split)
        // Window 1: 0,0 to 39,24 (left half)
        // Window 2: 40,0 to 79,24 (right half)
        let views = vec![
            WindowView::new(WindowId::new(1), Rect::new(0, 0, 40, 24)),
            WindowView::new(WindowId::new(2), Rect::new(40, 0, 40, 24)),
        ];

        let separators = super::find_separators(&views, 80, 24);

        // Should have 24 separator positions (one per row) at x=39
        assert!(!separators.is_empty());

        // All separators should be vertical
        for (x, y, is_vertical) in &separators {
            assert!(is_vertical, "Expected vertical separator at ({x}, {y})");
            assert_eq!(*x, 39, "Separator x should be 39");
            assert!(*y < 24, "Separator y should be < 24");
        }
    }

    #[test]
    fn test_find_separators_horizontal_split() {
        use reovim_driver_display::{Rect, WindowId, WindowView};

        // Two windows stacked (horizontal split)
        // Window 1: 0,0 to 80,12 (top half)
        // Window 2: 0,12 to 80,12 (bottom half)
        let views = vec![
            WindowView::new(WindowId::new(1), Rect::new(0, 0, 80, 12)),
            WindowView::new(WindowId::new(2), Rect::new(0, 12, 80, 12)),
        ];

        let separators = super::find_separators(&views, 80, 24);

        // Should have 80 separator positions (one per column) at y=11
        assert!(!separators.is_empty());

        // All separators should be horizontal
        for (x, y, is_vertical) in &separators {
            assert!(!is_vertical, "Expected horizontal separator at ({x}, {y})");
            assert_eq!(*y, 11, "Separator y should be 11");
            assert!(*x < 80, "Separator x should be < 80");
        }
    }

    #[test]
    fn test_find_separators_no_adjacent_windows() {
        use reovim_driver_display::{Rect, WindowId, WindowView};

        // Single window - no separators needed
        let views = vec![WindowView::new(WindowId::new(1), Rect::new(0, 0, 80, 24))];

        let separators = super::find_separators(&views, 80, 24);
        assert!(separators.is_empty(), "Single window should have no separators");
    }

    #[test]
    fn test_format_content_plain_text() {
        let content = "Hello\nWorld";
        let result = super::format_content(content, ScreenFormat::PlainText, 10, 5);

        assert_eq!(result.content, "Hello\nWorld");
        assert_eq!(result.width, 10);
        assert_eq!(result.height, 5);
        assert_eq!(result.format, ScreenFormat::PlainText);
    }

    #[test]
    fn test_format_content_cell_grid() {
        let content = "AB";
        let result = super::format_content(content, ScreenFormat::CellGrid, 10, 5);

        // Should be valid JSON
        let cells: Vec<Vec<serde_json::Value>> =
            serde_json::from_str(&result.content).expect("Should be valid JSON");
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].len(), 2);
        assert_eq!(cells[0][0].get("char").unwrap().as_str().unwrap(), "A");
        assert_eq!(cells[0][1].get("char").unwrap().as_str().unwrap(), "B");
    }
}
