pub enum InnerEvent {
    BufferEvent(BufferEvent),
    WindowEvent,
    RenderSignal,
    KillSignal,
}

pub enum BufferEvent {
    SetContent { buffer_id: usize, content: String },
}
