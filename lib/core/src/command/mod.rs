pub mod terminal {
    pub use reovim_sys::terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType,
    };
}
