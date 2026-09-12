//! Delivery state is separate from an interface's event payload.

use crate::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Next<Event> {
    Item(Event),
    Pending,
    Gap,
    End { lost: bool, error: Option<Error> },
}
