use {
    futures::future::FutureExt,
    futures_timer::Delay,
    std::{
        error::Error,
        future::poll_fn,
        io::{self, Write},
        time::Duration,
    },
    tokio::{select, sync::mpsc},
};

use crate::{
    event::{InnerEvent, key},
    io::input::{EventStreamKeySource, KeySource},
};

const DEFAULT_DELAY: u64 = 1;

/// Input event broker that reads key events and dispatches to handlers.
///
/// Generic over `KeySource` to allow injecting mock input for testing.
/// Defaults to `EventStreamKeySource` which reads from the real terminal.
pub struct InputEventBroker<K: KeySource = EventStreamKeySource> {
    delay: Duration,
    /// Grace period to wait after source exhaustion before exiting.
    /// This allows pending events to propagate through the async pipeline.
    /// Useful for tests where the source is immediately exhausted.
    grace_period: Duration,
    pub key_broker: key::KeyEventBroker,
    error_out: Box<dyn Write + Send>,
    key_source: K,
}

impl Default for InputEventBroker<EventStreamKeySource> {
    fn default() -> Self {
        Self {
            delay: Duration::from_millis(DEFAULT_DELAY),
            grace_period: Duration::ZERO,
            key_broker: key::KeyEventBroker::default(),
            error_out: Box::new(io::stdout()),
            key_source: EventStreamKeySource::default(),
        }
    }
}

impl InputEventBroker<EventStreamKeySource> {
    /// Create an `InputEventBroker` with an event sender for resize events.
    #[must_use]
    pub fn with_event_sender(event_sender: mpsc::Sender<InnerEvent>) -> Self {
        Self {
            delay: Duration::from_millis(DEFAULT_DELAY),
            grace_period: Duration::ZERO,
            key_broker: key::KeyEventBroker::default(),
            error_out: Box::new(io::stdout()),
            key_source: EventStreamKeySource::with_event_sender(event_sender),
        }
    }
}

impl<K: KeySource> InputEventBroker<K> {
    /// Create an `InputEventBroker` with a custom key source.
    ///
    /// This is primarily used for testing to inject mock key events.
    pub fn with_key_source(key_source: K) -> Self {
        Self {
            delay: Duration::from_millis(DEFAULT_DELAY),
            grace_period: Duration::ZERO,
            key_broker: key::KeyEventBroker::default(),
            error_out: Box::new(io::stdout()),
            key_source,
        }
    }

    /// Set the grace period to wait after source exhaustion before exiting.
    ///
    /// This allows pending events to propagate through the async pipeline
    /// before the broker exits. Useful for tests where events need time
    /// to be processed by handlers.
    #[must_use]
    pub const fn with_grace_period(mut self, grace_period: Duration) -> Self {
        self.grace_period = grace_period;
        self
    }

    #[allow(clippy::missing_panics_doc)]
    pub fn handle_error(&mut self, err: impl Error) {
        tracing::error!(error = %err, "Input event error");
        self.error_out
            .write_all(&err.to_string().into_bytes())
            .expect("Failed to send error event");
    }

    #[allow(clippy::ignored_unit_patterns)]
    pub async fn subscribe(mut self) {
        loop {
            let delay = Delay::new(self.delay).fuse();

            // Use poll_fn to convert the polling KeySource to a future
            let next_key = poll_fn(|cx| self.key_source.poll_next_key(cx));

            select! {
                () = delay => {},
                maybe_event = next_key => {
                    match maybe_event {
                        Some(Ok(key_event)) => {
                            if let Err(e) = self.key_broker.handle(key_event) {
                                self.handle_error(e);
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            self.handle_error(&*e);
                            break;
                        }
                        None => {
                            // Source exhausted
                            if self.key_source.is_exhausted() {
                                // Wait for grace period to allow pending events to propagate
                                if !self.grace_period.is_zero() {
                                    tokio::time::sleep(self.grace_period).await;
                                }
                                break;
                            }
                        }
                    }
                }
            };
        }
    }
}
