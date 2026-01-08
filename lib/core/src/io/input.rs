//! Input abstraction for terminal events
//!
//! Provides the `KeySource` trait for abstracting key event sources,
//! enabling tests to inject mock key sequences.

use std::{collections::VecDeque, error::Error, time::Duration};

use {
    futures::StreamExt,
    reovim_sys::event::{Event, EventStream, KeyEvent, KeyEventKind},
    tokio::sync::mpsc,
};

use crate::event::{RuntimeEvent, key::ScopedKeyEvent};

/// Trait for abstracting key event sources.
///
/// This allows swapping between real terminal input (`EventStream`) and
/// mock input sources for testing.
pub trait KeySource: Send {
    /// Poll for the next key event.
    ///
    /// Returns `Poll::Ready(Some(Ok(event)))` when a key is available,
    /// `Poll::Ready(Some(Err(e)))` on error,
    /// `Poll::Ready(None)` when the source is exhausted,
    /// or `Poll::Pending` when no event is currently available.
    fn poll_next_key(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<ScopedKeyEvent, Box<dyn Error + Send>>>>;

    /// Check if the source has been exhausted (no more events will arrive).
    fn is_exhausted(&self) -> bool;
}

/// Mock key source that yields pre-defined key events.
///
/// Used in tests to simulate user input without a real terminal.
pub struct MockKeySource {
    events: VecDeque<KeyEvent>,
    delay: Option<Duration>,
    delay_future: Option<std::pin::Pin<Box<tokio::time::Sleep>>>,
}

impl MockKeySource {
    /// Create a new mock key source with the given events.
    #[must_use]
    pub fn new(events: Vec<KeyEvent>) -> Self {
        Self {
            events: events.into(),
            delay: None,
            delay_future: None,
        }
    }

    /// Set a delay between events (useful for testing timing-sensitive code).
    #[must_use]
    pub const fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }
}

impl KeySource for MockKeySource {
    fn poll_next_key(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<ScopedKeyEvent, Box<dyn Error + Send>>>> {
        use std::task::Poll;

        // Handle delay if configured
        if let Some(delay) = self.delay {
            if self.delay_future.is_none() && !self.events.is_empty() {
                self.delay_future = Some(Box::pin(tokio::time::sleep(delay)));
            }

            if let Some(ref mut future) = self.delay_future {
                match future.as_mut().poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(()) => {
                        self.delay_future = None;
                    }
                }
            }
        }

        self.events.pop_front().map_or_else(
            || Poll::Ready(None),
            |event| Poll::Ready(Some(Ok(ScopedKeyEvent::new(event)))),
        )
    }

    fn is_exhausted(&self) -> bool {
        self.events.is_empty()
    }
}

/// Channel-based key source for dynamic event injection.
///
/// Allows sending key events from another task during test execution.
/// Events include optional `EventScope` for lifecycle tracking.
pub struct ChannelKeySource {
    rx: mpsc::Receiver<ScopedKeyEvent>,
}

impl ChannelKeySource {
    /// Create a new channel key source, returning the sender and receiver.
    #[must_use]
    pub fn new() -> (mpsc::Sender<ScopedKeyEvent>, Self) {
        let (tx, rx) = mpsc::channel(256);
        (tx, Self { rx })
    }
}

impl KeySource for ChannelKeySource {
    fn poll_next_key(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<ScopedKeyEvent, Box<dyn Error + Send>>>> {
        use std::task::Poll;

        match self.rx.poll_recv(cx) {
            Poll::Ready(Some(event)) => Poll::Ready(Some(Ok(event))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_exhausted(&self) -> bool {
        self.rx.is_closed()
    }
}

/// Real terminal event source wrapping crossterm's `EventStream`.
///
/// This is the default implementation used in production.
pub struct EventStreamKeySource {
    stream: EventStream,
    /// Optional sender for resize events
    event_sender: Option<mpsc::Sender<RuntimeEvent>>,
}

impl Default for EventStreamKeySource {
    fn default() -> Self {
        Self::new()
    }
}

impl EventStreamKeySource {
    /// Create a new event stream key source.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stream: EventStream::new(),
            event_sender: None,
        }
    }

    /// Create a new event stream key source with a resize event sender.
    #[must_use]
    pub fn with_event_sender(event_sender: mpsc::Sender<RuntimeEvent>) -> Self {
        Self {
            stream: EventStream::new(),
            event_sender: Some(event_sender),
        }
    }
}

impl KeySource for EventStreamKeySource {
    fn poll_next_key(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<ScopedKeyEvent, Box<dyn Error + Send>>>> {
        use std::task::Poll;

        loop {
            match self.stream.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(Event::Key(key_event)))) => {
                    // Only handle Press events, ignore Release/Repeat
                    if key_event.kind == KeyEventKind::Press {
                        // Real terminal events don't have a scope
                        return Poll::Ready(Some(Ok(ScopedKeyEvent::new(key_event))));
                    }
                    // Continue polling for Press events (fall through to loop)
                }
                Poll::Ready(Some(Ok(Event::Resize(cols, rows)))) => {
                    // Send resize event if we have a sender
                    if let Some(ref sender) = self.event_sender {
                        let _ = sender.try_send(RuntimeEvent::screen_resize(cols, rows));
                    }
                    // Continue polling for key events
                }
                Poll::Ready(Some(Ok(Event::Mouse(mouse_event)))) => {
                    // Forward mouse events to runtime
                    if let Some(ref sender) = self.event_sender {
                        let _ = sender.try_send(RuntimeEvent::mouse(mouse_event.into()));
                    }
                    // Continue polling for key events
                }
                Poll::Ready(Some(Ok(_))) => {
                    // Skip other non-key events (Focus, Paste)
                    // Fall through to loop
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(Box::new(e))));
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }

    fn is_exhausted(&self) -> bool {
        false // Real terminal never exhausts
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_sys::event::{KeyCode, KeyEventState, KeyModifiers},
        std::task::{Context, Poll, Wake, Waker},
    };

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    struct NoopWaker;
    impl Wake for NoopWaker {
        fn wake(self: std::sync::Arc<Self>) {}
    }

    fn noop_context() -> Context<'static> {
        let waker = Waker::from(std::sync::Arc::new(NoopWaker));
        // SAFETY: We're creating a context that will only be used synchronously
        Context::from_waker(Box::leak(Box::new(waker)))
    }

    #[test]
    fn test_mock_key_source_yields_events() {
        let events = vec![
            key_event(KeyCode::Char('i')),
            key_event(KeyCode::Char('h')),
            key_event(KeyCode::Esc),
        ];
        let mut source = MockKeySource::new(events);

        let mut cx = noop_context();

        // Should yield all events in order (wrapped in ScopedKeyEvent)
        match source.poll_next_key(&mut cx) {
            Poll::Ready(Some(Ok(e))) => assert_eq!(e.key.code, KeyCode::Char('i')),
            _ => panic!("Expected char 'i'"),
        }
        match source.poll_next_key(&mut cx) {
            Poll::Ready(Some(Ok(e))) => assert_eq!(e.key.code, KeyCode::Char('h')),
            _ => panic!("Expected char 'h'"),
        }
        match source.poll_next_key(&mut cx) {
            Poll::Ready(Some(Ok(e))) => assert_eq!(e.key.code, KeyCode::Esc),
            _ => panic!("Expected Esc"),
        }

        // Should be exhausted
        assert!(source.is_exhausted());
        match source.poll_next_key(&mut cx) {
            Poll::Ready(None) => {}
            _ => panic!("Expected None"),
        }
    }
}
