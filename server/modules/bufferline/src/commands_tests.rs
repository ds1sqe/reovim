use {reovim_driver_command::Command, reovim_kernel::api::v1::CommandId};

use super::*;

#[test]
fn pin_buffer_id() {
    assert_eq!(PinBuffer.id(), ids::PIN_BUFFER);
}

#[test]
fn pin_buffer_description() {
    assert!(!PinBuffer.description().is_empty());
}

#[test]
fn unpin_buffer_id() {
    assert_eq!(UnpinBuffer.id(), ids::UNPIN_BUFFER);
}

#[test]
fn unpin_buffer_description() {
    assert!(!UnpinBuffer.description().is_empty());
}

#[test]
fn close_buffer_id() {
    assert_eq!(CloseBuffer.id(), ids::CLOSE_BUFFER);
}

#[test]
fn close_buffer_description() {
    assert!(!CloseBuffer.description().is_empty());
}

#[test]
fn command_handlers_count() {
    assert_eq!(command_handlers().len(), 3);
}

#[test]
fn command_ids_match() {
    let handlers = command_handlers();
    let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
    assert!(ids.contains(&ids::PIN_BUFFER));
    assert!(ids.contains(&ids::UNPIN_BUFFER));
    assert!(ids.contains(&ids::CLOSE_BUFFER));
}
