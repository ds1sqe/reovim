use {
    reovim_core::{self, runtime::Runtime, screen::Screen},
    std::io::{self},
};

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    reovim_core::command::terminal::enable_raw_mode()?;
    let mut screen = Screen::default();
    screen.initialize()?;
    let runtime = Runtime::new(screen);
    runtime.init().await;

    reovim_core::command::terminal::disable_raw_mode()
}
