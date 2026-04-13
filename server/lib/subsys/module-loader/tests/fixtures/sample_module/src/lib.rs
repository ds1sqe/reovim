//! Sample server module fixture for #729 end-to-end validation.

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use {
    reovim_driver_command::{Command, CommandHandler, CommandHandlerStore, CommandProvider},
    reovim_driver_session::{BufferApi, SessionRuntime, WindowApi},
    reovim_kernel::api::v1::{
        CommandId, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
    reovim_subsys_command_types::{CommandContext, CommandResult},
};

const SAMPLE_MODULE: ModuleId = ModuleId::new("sample");
const SAMPLE_BUFFER_NAME: &str = "[sample-count]";

pub struct SampleModule {
    counter: Arc<AtomicU32>,
}

impl Default for SampleModule {
    fn default() -> Self {
        Self::new()
    }
}

impl SampleModule {
    #[must_use]
    pub fn new() -> Self {
        Self {
            counter: Arc::new(AtomicU32::new(0)),
        }
    }
}

impl Module for SampleModule {
    fn id(&self) -> ModuleId {
        SAMPLE_MODULE
    }

    fn name(&self) -> &'static str {
        "Sample Module"
    }

    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in self.command_handlers() {
            command_store.add(handler);
        }
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

impl CommandProvider for SampleModule {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        vec![
            Box::new(SampleIncCommand::new(Arc::clone(&self.counter))),
            Box::new(SampleGetCommand::new(Arc::clone(&self.counter))),
        ]
    }
}

struct SampleIncCommand {
    counter: Arc<AtomicU32>,
}

impl SampleIncCommand {
    const fn new(counter: Arc<AtomicU32>) -> Self {
        Self { counter }
    }
}

impl Command for SampleIncCommand {
    fn id(&self) -> CommandId {
        CommandId::new(SAMPLE_MODULE, "sample-inc")
    }

    fn description(&self) -> &'static str {
        "Increment the sample module counter"
    }

    fn names(&self) -> &[&'static str] {
        &["sample-inc", "sampleinc"]
    }
}

impl CommandHandler for SampleIncCommand {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        self.counter.fetch_add(1, Ordering::Relaxed);
        CommandResult::Success
    }
}

struct SampleGetCommand {
    counter: Arc<AtomicU32>,
}

impl SampleGetCommand {
    const fn new(counter: Arc<AtomicU32>) -> Self {
        Self { counter }
    }
}

impl Command for SampleGetCommand {
    fn id(&self) -> CommandId {
        CommandId::new(SAMPLE_MODULE, "sample-get")
    }

    fn description(&self) -> &'static str {
        "Show the current sample module counter"
    }

    fn names(&self) -> &[&'static str] {
        &["sample-get", "sampleget"]
    }
}

impl CommandHandler for SampleGetCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let value = self.counter.load(Ordering::Relaxed);
        let content = format!("sample-count:{value}");
        let buffer_id = runtime.create_buffer(Some(SAMPLE_BUFFER_NAME), &content);
        runtime.set_buffer_modified(buffer_id, false);
        runtime.set_active_buffer(Some(buffer_id));
        if let Some(window_id) = runtime.active_window() {
            let _ = runtime.set_window_buffer(window_id, buffer_id);
        }
        CommandResult::Success
    }
}

reovim_module_macros::declare_module!(SampleModule);
