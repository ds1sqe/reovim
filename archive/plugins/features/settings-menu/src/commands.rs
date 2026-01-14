//! Settings menu commands (unified command-event types)

use reovim_core::{
    command::traits::*,
    declare_event_command,
    event_bus::{DynEvent, Event},
};

// === Navigation Commands (unified types) ===
//
// Note: Open/Close use custom Event priority (50 instead of default 100)
// so they're implemented manually instead of using declare_event_command!

/// Open the settings menu
#[derive(Debug, Clone, Copy, Default)]
pub struct SettingsMenuOpen;

impl CommandTrait for SettingsMenuOpen {
    fn name(&self) -> &'static str {
        "settings_menu_open"
    }

    fn description(&self) -> &'static str {
        "Open the settings menu"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(*self))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Event for SettingsMenuOpen {
    fn priority(&self) -> u32 {
        50 // High priority for mode changes
    }
}

/// Close the settings menu
#[derive(Debug, Clone, Copy, Default)]
pub struct SettingsMenuClose;

impl CommandTrait for SettingsMenuClose {
    fn name(&self) -> &'static str {
        "settings_menu_close"
    }

    fn description(&self) -> &'static str {
        "Close the settings menu"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(*self))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Event for SettingsMenuClose {
    fn priority(&self) -> u32 {
        50 // High priority for mode changes
    }
}

declare_event_command! {
    SettingsMenuSelectNext,
    id: "settings_menu_next",
    description: "Select next item in settings menu",
}

declare_event_command! {
    SettingsMenuSelectPrev,
    id: "settings_menu_prev",
    description: "Select previous item in settings menu",
}

// === Action Commands (unified types) ===

declare_event_command! {
    SettingsMenuToggle,
    id: "settings_menu_toggle",
    description: "Toggle current boolean setting",
}

declare_event_command! {
    SettingsMenuCycleNext,
    id: "settings_menu_cycle_next",
    description: "Cycle to next choice option",
}

declare_event_command! {
    SettingsMenuCyclePrev,
    id: "settings_menu_cycle_prev",
    description: "Cycle to previous choice option",
}

declare_event_command! {
    SettingsMenuIncrement,
    id: "settings_menu_increment",
    description: "Increment current number setting",
}

declare_event_command! {
    SettingsMenuDecrement,
    id: "settings_menu_decrement",
    description: "Decrement current number setting",
}

declare_event_command! {
    SettingsMenuExecuteAction,
    id: "settings_menu_execute",
    description: "Execute current action item",
}

// === Quick Select Command (unified type with data) ===

/// Quick select option by number (1-9)
#[derive(Debug, Clone, Copy)]
pub struct SettingsMenuQuickSelect {
    /// The quick select number (1-9)
    pub number: u8,
}

impl SettingsMenuQuickSelect {
    /// Create instance for specific number
    #[must_use]
    pub const fn new(number: u8) -> Self {
        Self { number }
    }
}

impl CommandTrait for SettingsMenuQuickSelect {
    fn name(&self) -> &'static str {
        "settings_menu_quick_select"
    }

    fn description(&self) -> &'static str {
        "Quick select option by number"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(*self))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Event for SettingsMenuQuickSelect {
    fn priority(&self) -> u32 {
        100
    }
}

// === Individual quick select commands (1-9) ===
// These are type aliases that create instances of SettingsMenuQuickSelect

macro_rules! quick_select_command {
    ($name:ident, $num:literal, $cmd_name:literal) => {
        #[derive(Debug, Clone, Copy)]
        pub struct $name;

        impl CommandTrait for $name {
            fn name(&self) -> &'static str {
                $cmd_name
            }

            fn description(&self) -> &'static str {
                concat!("Quick select option ", stringify!($num))
            }

            fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
                CommandResult::EmitEvent(DynEvent::new(SettingsMenuQuickSelect::new($num)))
            }

            fn clone_box(&self) -> Box<dyn CommandTrait> {
                Box::new(*self)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    };
}

quick_select_command!(SettingsMenuQuick1, 1, "settings_menu_quick_1");
quick_select_command!(SettingsMenuQuick2, 2, "settings_menu_quick_2");
quick_select_command!(SettingsMenuQuick3, 3, "settings_menu_quick_3");
quick_select_command!(SettingsMenuQuick4, 4, "settings_menu_quick_4");
quick_select_command!(SettingsMenuQuick5, 5, "settings_menu_quick_5");
quick_select_command!(SettingsMenuQuick6, 6, "settings_menu_quick_6");
quick_select_command!(SettingsMenuQuick7, 7, "settings_menu_quick_7");
quick_select_command!(SettingsMenuQuick8, 8, "settings_menu_quick_8");
quick_select_command!(SettingsMenuQuick9, 9, "settings_menu_quick_9");
