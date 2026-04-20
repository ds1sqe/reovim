//! Tests for codec driver events.

use {
    super::*,
    reovim_kernel::api::v1::{BufferId, Event, events::kernel::priority},
};

#[test]
fn file_type_changed_construction() {
    let event = FileTypeChanged {
        buffer_id: BufferId::from_raw(1),
        file_type: "rust".to_string(),
    };
    assert_eq!(event.buffer_id, BufferId::from_raw(1));
    assert_eq!(event.file_type, "rust");
}

#[test]
fn file_type_changed_priority() {
    let event = FileTypeChanged {
        buffer_id: BufferId::from_raw(1),
        file_type: "python".to_string(),
    };
    assert_eq!(event.priority(), priority::NORMAL);
}

#[test]
fn file_type_changed_equality() {
    let a = FileTypeChanged {
        buffer_id: BufferId::from_raw(1),
        file_type: "rust".to_string(),
    };
    let b = FileTypeChanged {
        buffer_id: BufferId::from_raw(1),
        file_type: "rust".to_string(),
    };
    assert_eq!(a, b);
}

#[test]
fn file_type_changed_clone() {
    let event = FileTypeChanged {
        buffer_id: BufferId::from_raw(1),
        file_type: "elf".to_string(),
    };
    let cloned = event.clone();
    assert_eq!(cloned, event);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn file_type_changed_debug() {
    let event = FileTypeChanged {
        buffer_id: BufferId::from_raw(1),
        file_type: "pdf".to_string(),
    };
    let debug = format!("{event:?}");
    assert!(debug.contains("FileTypeChanged"));
    assert!(debug.contains("pdf"));
}
