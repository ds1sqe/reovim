use {
    super::{inner::BufferEvent, InnerEvent},
    crate::event::{key::KeyCode, KeyEvent, Subscribe},
    reovim_sys::{cursor, event::KeyModifiers},
    tokio::sync::{broadcast::Receiver, mpsc::Sender},
};

mod command;

pub use command::CommandHandler;

pub struct PrintEventHandler {
    pub buffer_id: usize,
    key_event_rx: Option<Receiver<KeyEvent>>,
    buffer_tx: Sender<InnerEvent>,
}

impl Subscribe<KeyEvent> for PrintEventHandler {
    fn subscribe(&mut self, rx: Receiver<KeyEvent>) {
        self.key_event_rx = Some(rx)
    }
}

impl PrintEventHandler {
    pub fn new(buffer_id: usize, tx: Sender<InnerEvent>) -> Self {
        Self {
            buffer_id,
            key_event_rx: None,
            buffer_tx: tx,
        }
    }

    async fn send_event(&self, content: String) {
        self.buffer_tx
            .send(InnerEvent::BufferEvent(BufferEvent::SetContent {
                buffer_id: self.buffer_id,
                content,
            }))
            .await
            .expect("failed to send event info");
    }

    pub async fn run(mut self) {
        if let Some(rx) = self.key_event_rx.take() {
            async {
                let mut rx = rx;
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            if event == KeyCode::Char('c').into() {
                                self.send_event(format!(
                                    "\rCursor position: {:?}",
                                    cursor::position()
                                ))
                                .await;
                            } else {
                                self.send_event(format!(
                                    "\rEvent::{:?}",
                                    event
                                ))
                                .await;
                            }
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }
            }
            .await;
        }
    }
}

pub struct TerminateHandler {
    key_event_rx: Option<Receiver<KeyEvent>>,
    kill_tx: Sender<InnerEvent>,
}

impl Subscribe<KeyEvent> for TerminateHandler {
    fn subscribe(&mut self, rx: Receiver<KeyEvent>) {
        self.key_event_rx = Some(rx)
    }
}

impl TerminateHandler {
    pub fn new(tx: Sender<InnerEvent>) -> Self {
        Self {
            key_event_rx: None,
            kill_tx: tx,
        }
    }

    async fn send_event(&self) {
        self.kill_tx
            .send(InnerEvent::KillSignal)
            .await
            .expect("failed to send terminate signal");
    }

    pub async fn run(mut self) {
        if let Some(rx) = self.key_event_rx.take() {
            async {
                let mut rx = rx;
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            if event.code == KeyCode::Char('d')
                                && event.modifiers == (KeyModifiers::CONTROL)
                            {
                                self.send_event().await;
                                break;
                            }
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }
            }
            .await;
        }
    }
}
