use std::collections::BTreeMap;

use crate::event::{
    BufferEvent, InputEventBroker, PrintEventHandler, TerminateHandler,
};

use {
    crate::{buffer::Buffer, event::InnerEvent, screen::Screen, state::State},
    tokio::sync::{broadcast, mpsc},
};

// own buffers and screen, windows
pub struct Runtime {
    pub buffers: BTreeMap<usize, Buffer>,
    pub screen: Screen,
    // state: State,
    pub tx: mpsc::Sender<InnerEvent>,
    pub rx: mpsc::Receiver<InnerEvent>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new(Screen::default())
    }
}

impl Runtime {
    pub fn new(screen: Screen) -> Self {
        let (tx, rx) = mpsc::channel(255);
        Self {
            buffers: BTreeMap::new(),
            screen,
            tx,
            rx,
        }
    }

    pub async fn init(mut self) {
        self.buffers.insert(0, Buffer::empty(0));
        let input_broker = InputEventBroker::default();
        let mut print_ev_hdr = PrintEventHandler::new(0, self.tx.clone());
        let mut terminate_hdr = TerminateHandler::new(self.tx.clone());
        input_broker.key_broker.enlist(&mut print_ev_hdr);
        input_broker.key_broker.enlist(&mut terminate_hdr);

        tokio::spawn(async move { print_ev_hdr.run().await });
        tokio::spawn(async move { terminate_hdr.run().await });
        tokio::spawn(async move { input_broker.subscribe().await });

        loop {
            let next = self.rx.recv().await;
            match next {
                Some(ev) => match ev {
                    InnerEvent::BufferEvent(buffer_event) => match buffer_event
                    {
                        BufferEvent::SetContent { buffer_id, content } => {
                            if let Some(b) = self.buffers.get_mut(&buffer_id) {
                                b.set_content(&content);

                                let buffers: Vec<Buffer> =
                                    self.buffers.values().cloned().collect();
                                self.screen
                                    .render(&buffers)
                                    .expect("failed to render");
                                self.screen.flush().expect("failed to flush");
                            }
                        }
                    },
                    InnerEvent::WindowEvent => todo!(),
                    InnerEvent::RenderSignal => {
                        let buffers: Vec<Buffer> =
                            self.buffers.values().cloned().collect();
                        self.screen.render(&buffers).expect("failed to render");
                    }
                    InnerEvent::KillSignal => {
                        break;
                    }
                },
                None => {
                    self.tx
                        .send(InnerEvent::KillSignal)
                        .await
                        .expect("cannot broadcast kill signal");
                    break;
                }
            }
        }
        self.screen.finalize();
    }
}
