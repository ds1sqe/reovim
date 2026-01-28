pub use reovim_sys::terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode};

mod terminal_guard;
pub use terminal_guard::TerminalGuard;
