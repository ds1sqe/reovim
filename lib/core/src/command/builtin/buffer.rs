//! Buffer navigation commands

use {
    crate::command::traits::{
        BufferAction, CommandResult, CommandTrait, DeferredAction, ExecutionContext,
    },
    std::any::Any,
};

/// Switch to previous buffer (H or [b)
#[derive(Debug, Clone)]
pub struct BufferPrevCommand;

impl CommandTrait for BufferPrevCommand {
    fn name(&self) -> &'static str {
        "buffer_prev"
    }

    fn description(&self) -> &'static str {
        "Previous buffer"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Buffer(BufferAction::Prev))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_jump(&self) -> bool {
        true
    }
}

/// Switch to next buffer (L or ]b)
#[derive(Debug, Clone)]
pub struct BufferNextCommand;

impl CommandTrait for BufferNextCommand {
    fn name(&self) -> &'static str {
        "buffer_next"
    }

    fn description(&self) -> &'static str {
        "Next buffer"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Buffer(BufferAction::Next))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_jump(&self) -> bool {
        true
    }
}

/// Delete current buffer (<leader>bd)
#[derive(Debug, Clone)]
pub struct BufferDeleteCommand;

impl CommandTrait for BufferDeleteCommand {
    fn name(&self) -> &'static str {
        "buffer_delete"
    }

    fn description(&self) -> &'static str {
        "Delete buffer"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Buffer(BufferAction::Delete { force: false }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::buffer::Buffer};

    fn create_test_context(buffer: &mut Buffer) -> ExecutionContext<'_> {
        ExecutionContext {
            buffer,
            count: None,
            buffer_id: 0,
            window_id: 0,
        }
    }

    #[test]
    fn test_buffer_prev_command_returns_deferred_action() {
        let cmd = BufferPrevCommand;
        let mut buffer = Buffer::empty(0);
        let mut ctx = create_test_context(&mut buffer);

        let result = cmd.execute(&mut ctx);

        match result {
            CommandResult::DeferToRuntime(DeferredAction::Buffer(BufferAction::Prev)) => {}
            _ => panic!("Expected DeferToRuntime(Buffer(Prev)), got {result:?}"),
        }
    }

    #[test]
    fn test_buffer_next_command_returns_deferred_action() {
        let cmd = BufferNextCommand;
        let mut buffer = Buffer::empty(0);
        let mut ctx = create_test_context(&mut buffer);

        let result = cmd.execute(&mut ctx);

        match result {
            CommandResult::DeferToRuntime(DeferredAction::Buffer(BufferAction::Next)) => {}
            _ => panic!("Expected DeferToRuntime(Buffer(Next)), got {result:?}"),
        }
    }

    #[test]
    fn test_buffer_delete_command_returns_deferred_action() {
        let cmd = BufferDeleteCommand;
        let mut buffer = Buffer::empty(0);
        let mut ctx = create_test_context(&mut buffer);

        let result = cmd.execute(&mut ctx);

        match result {
            CommandResult::DeferToRuntime(DeferredAction::Buffer(BufferAction::Delete {
                force: false,
            })) => {}
            _ => {
                panic!("Expected DeferToRuntime(Buffer(Delete {{ force: false }})), got {result:?}")
            }
        }
    }

    #[test]
    fn test_buffer_prev_command_metadata() {
        let cmd = BufferPrevCommand;
        assert_eq!(cmd.name(), "buffer_prev");
        assert_eq!(cmd.description(), "Previous buffer");
    }

    #[test]
    fn test_buffer_next_command_metadata() {
        let cmd = BufferNextCommand;
        assert_eq!(cmd.name(), "buffer_next");
        assert_eq!(cmd.description(), "Next buffer");
    }

    #[test]
    fn test_buffer_delete_command_metadata() {
        let cmd = BufferDeleteCommand;
        assert_eq!(cmd.name(), "buffer_delete");
        assert_eq!(cmd.description(), "Delete buffer");
    }

    #[test]
    fn test_buffer_commands_can_be_cloned() {
        let prev_cmd = BufferPrevCommand;
        let _cloned = prev_cmd.clone_box();

        let next_cmd = BufferNextCommand;
        let _cloned = next_cmd.clone_box();

        let del_cmd = BufferDeleteCommand;
        let _cloned = del_cmd.clone_box();
    }
}
