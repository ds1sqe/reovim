//! Command execution context

/// Provides context for command execution
#[derive(Debug, Clone, Default)]
pub struct CommandContext {
    pub buffer_id: usize,
    pub window_id: usize,
    /// For repeat counts like 5j
    pub count: Option<usize>,
}
