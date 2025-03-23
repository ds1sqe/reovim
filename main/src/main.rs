use {
    reovim_core::{
        api::{
            self,
            event::{handler::PrintEvent, SubscribeConfig},
            screen::Screen,
        },
        buffer::Buffer,
    },
    std::{
        io::{self},
        time::Duration,
    },
};

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    api::enable_raw_mode()?;
    let config = SubscribeConfig {
        delay: Duration::from_millis(50),
    };
    let mut screen = Screen::default();
    screen.initialize()?;
    let mut event_print_buffer = Buffer::empty(0);
    screen
        .attach_handler(config, PrintEvent, &mut event_print_buffer)
        .await;
    api::run(&mut screen, &event_print_buffer).await?;

    api::disable_raw_mode()
}
