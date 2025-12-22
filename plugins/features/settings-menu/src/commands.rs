//! Settings menu commands

use {
    crate::settings_menu::{
        SettingsMenuCloseEvent, SettingsMenuCycleNextEvent, SettingsMenuCyclePrevEvent,
        SettingsMenuDecrementEvent, SettingsMenuExecuteActionEvent, SettingsMenuIncrementEvent,
        SettingsMenuOpenEvent, SettingsMenuQuickSelectEvent, SettingsMenuSelectNextEvent,
        SettingsMenuSelectPrevEvent, SettingsMenuToggleEvent,
    },
    reovim_core::{
        command::traits::{CommandResult, CommandTrait, ExecutionContext},
        event_bus::DynEvent,
    },
    std::any::Any,
};

/// Open the settings menu
#[derive(Debug, Clone)]
pub struct SettingsMenuOpenCommand;

impl CommandTrait for SettingsMenuOpenCommand {
    fn name(&self) -> &'static str {
        "settings_menu_open"
    }

    fn description(&self) -> &'static str {
        "Open the settings menu"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(SettingsMenuOpenEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Close the settings menu
#[derive(Debug, Clone)]
pub struct SettingsMenuCloseCommand;

impl CommandTrait for SettingsMenuCloseCommand {
    fn name(&self) -> &'static str {
        "settings_menu_close"
    }

    fn description(&self) -> &'static str {
        "Close the settings menu"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(SettingsMenuCloseEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Select next item in settings menu
#[derive(Debug, Clone)]
pub struct SettingsMenuNextCommand;

impl CommandTrait for SettingsMenuNextCommand {
    fn name(&self) -> &'static str {
        "settings_menu_next"
    }

    fn description(&self) -> &'static str {
        "Select next item in settings menu"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(SettingsMenuSelectNextEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Select previous item in settings menu
#[derive(Debug, Clone)]
pub struct SettingsMenuPrevCommand;

impl CommandTrait for SettingsMenuPrevCommand {
    fn name(&self) -> &'static str {
        "settings_menu_prev"
    }

    fn description(&self) -> &'static str {
        "Select previous item in settings menu"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(SettingsMenuSelectPrevEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Toggle boolean setting
#[derive(Debug, Clone)]
pub struct SettingsMenuToggleCommand;

impl CommandTrait for SettingsMenuToggleCommand {
    fn name(&self) -> &'static str {
        "settings_menu_toggle"
    }

    fn description(&self) -> &'static str {
        "Toggle current boolean setting"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(SettingsMenuToggleEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Cycle to next choice option
#[derive(Debug, Clone)]
pub struct SettingsMenuCycleNextCommand;

impl CommandTrait for SettingsMenuCycleNextCommand {
    fn name(&self) -> &'static str {
        "settings_menu_cycle_next"
    }

    fn description(&self) -> &'static str {
        "Cycle to next choice option"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(SettingsMenuCycleNextEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Cycle to previous choice option
#[derive(Debug, Clone)]
pub struct SettingsMenuCyclePrevCommand;

impl CommandTrait for SettingsMenuCyclePrevCommand {
    fn name(&self) -> &'static str {
        "settings_menu_cycle_prev"
    }

    fn description(&self) -> &'static str {
        "Cycle to previous choice option"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(SettingsMenuCyclePrevEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Increment number setting
#[derive(Debug, Clone)]
pub struct SettingsMenuIncrementCommand;

impl CommandTrait for SettingsMenuIncrementCommand {
    fn name(&self) -> &'static str {
        "settings_menu_increment"
    }

    fn description(&self) -> &'static str {
        "Increment current number setting"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(SettingsMenuIncrementEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Decrement number setting
#[derive(Debug, Clone)]
pub struct SettingsMenuDecrementCommand;

impl CommandTrait for SettingsMenuDecrementCommand {
    fn name(&self) -> &'static str {
        "settings_menu_decrement"
    }

    fn description(&self) -> &'static str {
        "Decrement current number setting"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(SettingsMenuDecrementEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Execute action item
#[derive(Debug, Clone)]
pub struct SettingsMenuExecuteCommand;

impl CommandTrait for SettingsMenuExecuteCommand {
    fn name(&self) -> &'static str {
        "settings_menu_execute"
    }

    fn description(&self) -> &'static str {
        "Execute current action item"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(SettingsMenuExecuteActionEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Macro to generate quick select commands (1-9)
macro_rules! quick_select_command {
    ($name:ident, $num:literal, $cmd_name:literal) => {
        #[derive(Debug, Clone)]
        pub struct $name;

        impl CommandTrait for $name {
            fn name(&self) -> &'static str {
                $cmd_name
            }

            fn description(&self) -> &'static str {
                concat!("Quick select option ", stringify!($num))
            }

            fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
                CommandResult::EmitEvent(DynEvent::new(SettingsMenuQuickSelectEvent {
                    number: $num,
                }))
            }

            fn clone_box(&self) -> Box<dyn CommandTrait> {
                Box::new(self.clone())
            }

            fn as_any(&self) -> &dyn Any {
                self
            }
        }
    };
}

quick_select_command!(SettingsMenuQuick1Command, 1, "settings_menu_quick_1");
quick_select_command!(SettingsMenuQuick2Command, 2, "settings_menu_quick_2");
quick_select_command!(SettingsMenuQuick3Command, 3, "settings_menu_quick_3");
quick_select_command!(SettingsMenuQuick4Command, 4, "settings_menu_quick_4");
quick_select_command!(SettingsMenuQuick5Command, 5, "settings_menu_quick_5");
quick_select_command!(SettingsMenuQuick6Command, 6, "settings_menu_quick_6");
quick_select_command!(SettingsMenuQuick7Command, 7, "settings_menu_quick_7");
quick_select_command!(SettingsMenuQuick8Command, 8, "settings_menu_quick_8");
quick_select_command!(SettingsMenuQuick9Command, 9, "settings_menu_quick_9");
