/// Provides context for command execution
#[derive(Debug, Clone)]
pub struct CommandContext {
    pub buffer_id: usize,
    pub window_id: usize,
    /// For repeat counts like 5j
    pub count: Option<usize>,
}

impl Default for CommandContext {
    fn default() -> Self {
        Self {
            buffer_id: 0,
            window_id: 0,
            count: None,
        }
    }
}
