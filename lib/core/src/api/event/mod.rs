use {
    super::screen::Screen,
    crate::{
        buffer::Buffer,
        event::{Event, EventStream, KeyCode, KeyModifiers},
    },
    futures::{future::FutureExt, select, StreamExt},
    futures_timer::Delay,
    handler::EventHandler,
    std::time::Duration,
};

pub mod handler;

pub struct SubscribeConfig {
    pub delay: Duration,
}

pub async fn subscribe(
    screen: &mut Screen,
    config: SubscribeConfig,
    handler: impl EventHandler,
    buf: &mut Buffer,
) {
    let mut reader = EventStream::new();
    let mut handler = handler;

    loop {
        let mut delay = Delay::new(config.delay).fuse();
        let mut event = reader.next().fuse();
        screen.flush().expect("failed to flush screen");

        select! {
            _ = delay => {},
            maybe_event = event => {
                match maybe_event {
                    Some(Ok(ev)) => {
                        handler.on_event(&ev,buf);
                        if let Event::Key(key_event) = ev {
                            if key_event.code == KeyCode::Char('q') && key_event.modifiers == (KeyModifiers::CONTROL)  {
                                break;
                            }
                        }

                    }
                    Some(Err(e)) => print!("Error: {:?}", e),
                    None => break,
                }
            }
        };
    }
}
