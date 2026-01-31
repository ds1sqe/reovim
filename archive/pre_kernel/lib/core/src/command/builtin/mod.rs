//! Built-in command implementations

mod buffer;
mod clipboard;
mod command_line;
mod cursor;
mod history;
mod jump;
mod mode;
mod operator;
mod system;
mod tab;
mod text;
mod visual;
mod window;

// Note: Completion commands are now in reovim-plugin-completion crate
// Note: Settings Menu commands are now in reovim-plugin-settings-menu crate

pub use {
    buffer::*, clipboard::*, command_line::*, cursor::*, history::*, jump::*, mode::*, operator::*,
    system::*, tab::*, text::*, visual::*, window::*,
};
