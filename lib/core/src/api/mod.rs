use std::time::Duration;

pub use {
    crate::command::terminal::{disable_raw_mode, enable_raw_mode},
    reovim_sys::terminal::window_size,
};
use {futures::FutureExt, futures_timer::Delay, screen::Screen};

pub mod event;
pub mod screen;

pub mod cursor {
    pub use reovim_sys::cursor::position;
}

pub async fn run(
    scr: &mut Screen,
    buf: &crate::buffer::Buffer,
) -> Result<(), std::io::Error> {
    loop {
        let delay = Delay::new(Duration::from_millis(100)).fuse();
        delay.await;
        scr.render(buf)?;
    }
}
