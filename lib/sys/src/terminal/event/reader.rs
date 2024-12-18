use {super::Event, std::collections::VecDeque};

pub(crate) struct EventReader {
    events: VecDeque<Event>,
}
