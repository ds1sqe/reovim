use crate::{
    api::cursor,
    buffer::Buffer,
    event::{Event, KeyCode},
};

pub trait EventHandler: Copy {
    fn on_event(&mut self, event: &Event, buf: &mut Buffer);
    fn on_delay(&mut self);
}

#[derive(Copy, Clone)]
pub struct PrintEvent;

impl EventHandler for PrintEvent {
    fn on_event(&mut self, event: &Event, buf: &mut Buffer) {
        if *event == Event::Key(KeyCode::Char('c').into()) {
            buf.set_content(&format!(
                "\rCursor position: {:?}",
                cursor::position()
            ));
        } else {
            buf.set_content(&format!("\rEvent::{:?}", *event));
        }
    }

    fn on_delay(&mut self) {
        todo!()
    }
}
