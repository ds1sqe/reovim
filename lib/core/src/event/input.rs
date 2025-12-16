use {
    futures::{future::FutureExt, StreamExt},
    futures_timer::Delay,
    reovim_sys::event::{Event, EventStream, KeyEventKind},
    std::{
        error::Error,
        io::{self, Write},
        time::Duration,
    },
    tokio::select,
};

use crate::event::key;

const DEFAULT_DELAY: u64 = 1;

pub struct InputEventBroker {
    delay: Duration,
    pub key_broker: key::KeyEventBroker,
    error_out: Box<dyn Write + Send>,
}

impl Default for InputEventBroker {
    fn default() -> Self {
        Self {
            delay: Duration::from_millis(DEFAULT_DELAY),
            key_broker: key::KeyEventBroker::default(),
            error_out: Box::new(io::stdout()),
        }
    }
}

impl InputEventBroker {
    #[allow(clippy::missing_panics_doc)]
    pub fn handle_error(&mut self, err: impl Error) {
        tracing::error!(error = %err, "Input event error");
        self.error_out
            .write_all(&err.to_string().into_bytes())
            .expect("Failed to send error event");
    }

    #[allow(clippy::ignored_unit_patterns)]
    #[allow(clippy::match_same_arms)]
    pub async fn subscribe(mut self) {
        let mut reader = EventStream::new();

        loop {
            let delay = Delay::new(self.delay).fuse();
            let event = reader.next().fuse();

            select! {
                () = delay => {},
                maybe_event = event => {
                    match maybe_event {
                        Some(Ok(ev)) => {
                            match ev {
                                Event::Key(key_event) => {
                                    // Only handle Press events, ignore Release/Repeat
                                    if key_event.kind != KeyEventKind::Press {
                                        continue;
                                    }
                                    if let Err(e) = self.key_broker.handle(key_event) {
                                        self.handle_error(e);
                                        break;
                                    }
                                }
                                Event::Mouse(_) |
                                Event::FocusGained |
                                Event::FocusLost |
                                Event::Paste(_) |
                                Event::Resize(_, _) => ()
                            }
                        }
                        Some(Err(e)) => {
                            self.handle_error(e);
                            break;

                        }
                        None => break,
                    }
                }
            };
        }
    }
}
