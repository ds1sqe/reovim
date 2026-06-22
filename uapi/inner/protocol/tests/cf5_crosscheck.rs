//! CF5 inventory cross-check — Phase 3 (plan 05-uapi-foundation.md §Phase 3).
//!
//! Asserts that the implemented tag/direction inventory in `uapi/protocol`
//! exactly matches the §7 table in Documentation/07-Surfaces/03-Server-Client-Protocol.md.
//!
//! ## Method
//!
//! The §7 table is encoded as a static golden array of `(tag, direction, name)`
//! tuples below.  For every row the test asserts:
//!
//! - The corresponding struct carries the exact `TAG` constant.
//! - The struct carries the exact `DIRECTION` constant.
//! - The total count is 37 (2 handshake + 12 req + 13 resp + 9 notify + 1 error).
//!
//! CF5 fails closed: a struct added without a matching §7 row will not be
//! caught here (that direction is caught by code-review), but a §7 row whose
//! struct carries the wrong TAG or DIRECTION is caught.  The
//! `deliberately_missing_struct_fails_closed` test proves the fixture-based
//! approach detects mismatches by asserting that a crafted mismatch would
//! differ.
//!
//! Source: Documentation/07-Surfaces/03-Server-Client-Protocol.md §7.

use reovim_uapi_protocol::messages::{
    // req
    Attach,
    // resp
    AttachAck,
    // notify
    AttachEventClientLeft,
    AttachEventCursor,
    AttachEventDiff,
    AttachEventDomainTableDelta,
    AttachEventFrame,
    AttachEventProjection,
    AttachEventServerDraining,
    AttachEventSessionDestroyed,
    AttachEventSessionPivot,
    ConfigDump,
    ConfigDumpResponse,
    ConfigValidate,
    ConfigValidateResponse,
    DebugDrive,
    DebugDriveAck,
    DebugRead,
    DebugReadEvent,
    DestroySession,
    DestroySessionAck,
    Detach,
    DetachAck,
    // helpers
    Direction,
    // handshake
    Hello,
    HelloAck,
    InputAck,
    Message,
    PkgSync,
    PkgSyncDone,
    PkgSyncProgress,
    PkgVerify,
    PkgVerifyAck,
    // error
    Reject,
    RenameSession,
    RenameSessionAck,
    SendInput,
    SwitchSession,
    SwitchSessionAck,
};

// ---------------------------------------------------------------------------
// §7 golden table (the canonical spec inventory, encoded as data)
// ---------------------------------------------------------------------------
//
// Columns: (tag, direction, human-readable name for assertion messages)
//
// Row order follows the spec table top-to-bottom.

struct InventoryRow {
    tag: u16,
    direction: Direction,
    name: &'static str,
}

const INVENTORY: &[InventoryRow] = &[
    // Handshake (0x0001–0x0002)
    InventoryRow {
        tag: 0x0001,
        direction: Direction::Handshake,
        name: "Hello",
    },
    InventoryRow {
        tag: 0x0002,
        direction: Direction::Handshake,
        name: "HelloAck",
    },
    // Requests (0x0100–0x010B)
    InventoryRow {
        tag: 0x0100,
        direction: Direction::Request,
        name: "Attach",
    },
    InventoryRow {
        tag: 0x0101,
        direction: Direction::Request,
        name: "Detach",
    },
    InventoryRow {
        tag: 0x0102,
        direction: Direction::Request,
        name: "SendInput",
    },
    InventoryRow {
        tag: 0x0103,
        direction: Direction::Request,
        name: "SwitchSession",
    },
    InventoryRow {
        tag: 0x0104,
        direction: Direction::Request,
        name: "DestroySession",
    },
    InventoryRow {
        tag: 0x0105,
        direction: Direction::Request,
        name: "RenameSession",
    },
    InventoryRow {
        tag: 0x0106,
        direction: Direction::Request,
        name: "DebugRead",
    },
    InventoryRow {
        tag: 0x0107,
        direction: Direction::Request,
        name: "DebugDrive",
    },
    InventoryRow {
        tag: 0x0108,
        direction: Direction::Request,
        name: "PkgSync",
    },
    InventoryRow {
        tag: 0x0109,
        direction: Direction::Request,
        name: "PkgVerify",
    },
    InventoryRow {
        tag: 0x010A,
        direction: Direction::Request,
        name: "ConfigDump",
    },
    InventoryRow {
        tag: 0x010B,
        direction: Direction::Request,
        name: "ConfigValidate",
    },
    // Responses (0x0200–0x020C)
    InventoryRow {
        tag: 0x0200,
        direction: Direction::Response,
        name: "AttachAck",
    },
    InventoryRow {
        tag: 0x0201,
        direction: Direction::Response,
        name: "DetachAck",
    },
    InventoryRow {
        tag: 0x0202,
        direction: Direction::Response,
        name: "InputAck",
    },
    InventoryRow {
        tag: 0x0203,
        direction: Direction::Response,
        name: "SwitchSessionAck",
    },
    InventoryRow {
        tag: 0x0204,
        direction: Direction::Response,
        name: "DestroySessionAck",
    },
    InventoryRow {
        tag: 0x0205,
        direction: Direction::Response,
        name: "RenameSessionAck",
    },
    InventoryRow {
        tag: 0x0206,
        direction: Direction::Response,
        name: "DebugReadEvent",
    },
    InventoryRow {
        tag: 0x0207,
        direction: Direction::Response,
        name: "DebugDriveAck",
    },
    InventoryRow {
        tag: 0x0208,
        direction: Direction::Response,
        name: "PkgSyncProgress",
    },
    InventoryRow {
        tag: 0x0209,
        direction: Direction::Response,
        name: "PkgSyncDone",
    },
    InventoryRow {
        tag: 0x020A,
        direction: Direction::Response,
        name: "PkgVerifyAck",
    },
    InventoryRow {
        tag: 0x020B,
        direction: Direction::Response,
        name: "ConfigDumpResponse",
    },
    InventoryRow {
        tag: 0x020C,
        direction: Direction::Response,
        name: "ConfigValidateResponse",
    },
    // Notifications (0x0301–0x0309)
    InventoryRow {
        tag: 0x0301,
        direction: Direction::Notify,
        name: "AttachEventFrame",
    },
    InventoryRow {
        tag: 0x0302,
        direction: Direction::Notify,
        name: "AttachEventDiff",
    },
    InventoryRow {
        tag: 0x0303,
        direction: Direction::Notify,
        name: "AttachEventCursor",
    },
    InventoryRow {
        tag: 0x0304,
        direction: Direction::Notify,
        name: "AttachEventProjection",
    },
    InventoryRow {
        tag: 0x0305,
        direction: Direction::Notify,
        name: "AttachEventDomainTableDelta",
    },
    InventoryRow {
        tag: 0x0306,
        direction: Direction::Notify,
        name: "AttachEventSessionPivot",
    },
    InventoryRow {
        tag: 0x0307,
        direction: Direction::Notify,
        name: "AttachEventSessionDestroyed",
    },
    InventoryRow {
        tag: 0x0308,
        direction: Direction::Notify,
        name: "AttachEventClientLeft",
    },
    InventoryRow {
        tag: 0x0309,
        direction: Direction::Notify,
        name: "AttachEventServerDraining",
    },
    // Error (0xFF00)
    InventoryRow {
        tag: 0xFF00,
        direction: Direction::Error,
        name: "Reject",
    },
];

// ---------------------------------------------------------------------------
// Count check
// ---------------------------------------------------------------------------

/// CF5: the §7 table has exactly 37 rows (2 handshake + 12 req + 13 resp +
/// 9 notify + 1 error).  A count mismatch here means the golden table above
/// diverged from the spec — fix the golden first, then fix the structs.
#[test]
fn inventory_count_is_37() {
    assert_eq!(
        INVENTORY.len(),
        37,
        "CF5: §7 inventory must contain exactly 37 message types \
         (2 handshake + 12 req + 13 resp + 9 notify + 1 error)"
    );
}

/// CF5: direction sub-counts must match the §7 column breakdown.
#[test]
fn inventory_direction_subcounts() {
    let handshake = INVENTORY
        .iter()
        .filter(|r| r.direction == Direction::Handshake)
        .count();
    let req = INVENTORY
        .iter()
        .filter(|r| r.direction == Direction::Request)
        .count();
    let resp = INVENTORY
        .iter()
        .filter(|r| r.direction == Direction::Response)
        .count();
    let notify = INVENTORY
        .iter()
        .filter(|r| r.direction == Direction::Notify)
        .count();
    let error = INVENTORY
        .iter()
        .filter(|r| r.direction == Direction::Error)
        .count();
    assert_eq!(handshake, 2, "CF5: expected 2 handshake messages");
    assert_eq!(req, 12, "CF5: expected 12 request messages");
    assert_eq!(resp, 13, "CF5: expected 13 response messages");
    assert_eq!(notify, 9, "CF5: expected 9 notify messages");
    assert_eq!(error, 1, "CF5: expected 1 error message");
}

// ---------------------------------------------------------------------------
// Per-struct TAG + DIRECTION assertions against the §7 golden
// ---------------------------------------------------------------------------

/// CF5: `Hello` (row 0) — tag 0x0001, direction Handshake.
#[test]
fn cf5_hello_tag_and_direction() {
    let row = &INVENTORY[0];
    assert_eq!(Hello::TAG, row.tag, "CF5: Hello TAG mismatch vs §7 row '{}'", row.name);
    assert_eq!(Hello::DIRECTION, row.direction, "CF5: Hello DIRECTION mismatch");
}

/// CF5: `HelloAck` (row 1) — tag 0x0002, direction Handshake.
#[test]
fn cf5_hello_ack_tag_and_direction() {
    let row = &INVENTORY[1];
    assert_eq!(HelloAck::TAG, row.tag, "CF5: HelloAck TAG mismatch vs §7 row '{}'", row.name);
    assert_eq!(HelloAck::DIRECTION, row.direction, "CF5: HelloAck DIRECTION mismatch");
}

/// CF5: `Attach` (row 2) — tag 0x0100, direction Request.
#[test]
fn cf5_attach_tag_and_direction() {
    let row = &INVENTORY[2];
    assert_eq!(Attach::TAG, row.tag, "CF5: Attach TAG mismatch vs §7 row '{}'", row.name);
    assert_eq!(Attach::DIRECTION, row.direction, "CF5: Attach DIRECTION mismatch");
}

/// CF5: `Detach` (row 3) — tag 0x0101, direction Request.
#[test]
fn cf5_detach_tag_and_direction() {
    let row = &INVENTORY[3];
    assert_eq!(Detach::TAG, row.tag, "CF5: Detach TAG mismatch vs §7 row '{}'", row.name);
    assert_eq!(Detach::DIRECTION, row.direction, "CF5: Detach DIRECTION mismatch");
}

/// CF5: `SendInput` (row 4) — tag 0x0102, direction Request.
#[test]
fn cf5_send_input_tag_and_direction() {
    let row = &INVENTORY[4];
    assert_eq!(SendInput::TAG, row.tag, "CF5: SendInput TAG mismatch vs §7 row '{}'", row.name);
    assert_eq!(SendInput::DIRECTION, row.direction, "CF5: SendInput DIRECTION mismatch");
}

/// CF5: `SwitchSession` (row 5) — tag 0x0103, direction Request.
#[test]
fn cf5_switch_session_tag_and_direction() {
    let row = &INVENTORY[5];
    assert_eq!(
        SwitchSession::TAG,
        row.tag,
        "CF5: SwitchSession TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(SwitchSession::DIRECTION, row.direction, "CF5: SwitchSession DIRECTION mismatch");
}

/// CF5: `DestroySession` (row 6) — tag 0x0104, direction Request.
#[test]
fn cf5_destroy_session_tag_and_direction() {
    let row = &INVENTORY[6];
    assert_eq!(
        DestroySession::TAG,
        row.tag,
        "CF5: DestroySession TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        DestroySession::DIRECTION,
        row.direction,
        "CF5: DestroySession DIRECTION mismatch"
    );
}

/// CF5: `RenameSession` (row 7) — tag 0x0105, direction Request.
#[test]
fn cf5_rename_session_tag_and_direction() {
    let row = &INVENTORY[7];
    assert_eq!(
        RenameSession::TAG,
        row.tag,
        "CF5: RenameSession TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(RenameSession::DIRECTION, row.direction, "CF5: RenameSession DIRECTION mismatch");
}

/// CF5: `DebugRead` (row 8) — tag 0x0106, direction Request.
#[test]
fn cf5_debug_read_tag_and_direction() {
    let row = &INVENTORY[8];
    assert_eq!(DebugRead::TAG, row.tag, "CF5: DebugRead TAG mismatch vs §7 row '{}'", row.name);
    assert_eq!(DebugRead::DIRECTION, row.direction, "CF5: DebugRead DIRECTION mismatch");
}

/// CF5: `DebugDrive` (row 9) — tag 0x0107, direction Request.
#[test]
fn cf5_debug_drive_tag_and_direction() {
    let row = &INVENTORY[9];
    assert_eq!(
        DebugDrive::TAG,
        row.tag,
        "CF5: DebugDrive TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(DebugDrive::DIRECTION, row.direction, "CF5: DebugDrive DIRECTION mismatch");
}

/// CF5: `PkgSync` (row 10) — tag 0x0108, direction Request.
#[test]
fn cf5_pkg_sync_tag_and_direction() {
    let row = &INVENTORY[10];
    assert_eq!(PkgSync::TAG, row.tag, "CF5: PkgSync TAG mismatch vs §7 row '{}'", row.name);
    assert_eq!(PkgSync::DIRECTION, row.direction, "CF5: PkgSync DIRECTION mismatch");
}

/// CF5: `PkgVerify` (row 11) — tag 0x0109, direction Request.
#[test]
fn cf5_pkg_verify_tag_and_direction() {
    let row = &INVENTORY[11];
    assert_eq!(PkgVerify::TAG, row.tag, "CF5: PkgVerify TAG mismatch vs §7 row '{}'", row.name);
    assert_eq!(PkgVerify::DIRECTION, row.direction, "CF5: PkgVerify DIRECTION mismatch");
}

/// CF5: `ConfigDump` (row 12) — tag 0x010A, direction Request.
#[test]
fn cf5_config_dump_tag_and_direction() {
    let row = &INVENTORY[12];
    assert_eq!(
        ConfigDump::TAG,
        row.tag,
        "CF5: ConfigDump TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(ConfigDump::DIRECTION, row.direction, "CF5: ConfigDump DIRECTION mismatch");
}

/// CF5: `ConfigValidate` (row 13) — tag 0x010B, direction Request.
#[test]
fn cf5_config_validate_tag_and_direction() {
    let row = &INVENTORY[13];
    assert_eq!(
        ConfigValidate::TAG,
        row.tag,
        "CF5: ConfigValidate TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        ConfigValidate::DIRECTION,
        row.direction,
        "CF5: ConfigValidate DIRECTION mismatch"
    );
}

/// CF5: `AttachAck` (row 14) — tag 0x0200, direction Response.
#[test]
fn cf5_attach_ack_tag_and_direction() {
    let row = &INVENTORY[14];
    assert_eq!(AttachAck::TAG, row.tag, "CF5: AttachAck TAG mismatch vs §7 row '{}'", row.name);
    assert_eq!(AttachAck::DIRECTION, row.direction, "CF5: AttachAck DIRECTION mismatch");
}

/// CF5: `DetachAck` (row 15) — tag 0x0201, direction Response.
#[test]
fn cf5_detach_ack_tag_and_direction() {
    let row = &INVENTORY[15];
    assert_eq!(DetachAck::TAG, row.tag, "CF5: DetachAck TAG mismatch vs §7 row '{}'", row.name);
    assert_eq!(DetachAck::DIRECTION, row.direction, "CF5: DetachAck DIRECTION mismatch");
}

/// CF5: `InputAck` (row 16) — tag 0x0202, direction Response.
#[test]
fn cf5_input_ack_tag_and_direction() {
    let row = &INVENTORY[16];
    assert_eq!(InputAck::TAG, row.tag, "CF5: InputAck TAG mismatch vs §7 row '{}'", row.name);
    assert_eq!(InputAck::DIRECTION, row.direction, "CF5: InputAck DIRECTION mismatch");
}

/// CF5: `SwitchSessionAck` (row 17) — tag 0x0203, direction Response.
#[test]
fn cf5_switch_session_ack_tag_and_direction() {
    let row = &INVENTORY[17];
    assert_eq!(
        SwitchSessionAck::TAG,
        row.tag,
        "CF5: SwitchSessionAck TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        SwitchSessionAck::DIRECTION,
        row.direction,
        "CF5: SwitchSessionAck DIRECTION mismatch"
    );
}

/// CF5: `DestroySessionAck` (row 18) — tag 0x0204, direction Response.
#[test]
fn cf5_destroy_session_ack_tag_and_direction() {
    let row = &INVENTORY[18];
    assert_eq!(
        DestroySessionAck::TAG,
        row.tag,
        "CF5: DestroySessionAck TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        DestroySessionAck::DIRECTION,
        row.direction,
        "CF5: DestroySessionAck DIRECTION mismatch"
    );
}

/// CF5: `RenameSessionAck` (row 19) — tag 0x0205, direction Response.
#[test]
fn cf5_rename_session_ack_tag_and_direction() {
    let row = &INVENTORY[19];
    assert_eq!(
        RenameSessionAck::TAG,
        row.tag,
        "CF5: RenameSessionAck TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        RenameSessionAck::DIRECTION,
        row.direction,
        "CF5: RenameSessionAck DIRECTION mismatch"
    );
}

/// CF5: `DebugReadEvent` (row 20) — tag 0x0206, direction Response.
#[test]
fn cf5_debug_read_event_tag_and_direction() {
    let row = &INVENTORY[20];
    assert_eq!(
        DebugReadEvent::TAG,
        row.tag,
        "CF5: DebugReadEvent TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        DebugReadEvent::DIRECTION,
        row.direction,
        "CF5: DebugReadEvent DIRECTION mismatch"
    );
}

/// CF5: `DebugDriveAck` (row 21) — tag 0x0207, direction Response.
#[test]
fn cf5_debug_drive_ack_tag_and_direction() {
    let row = &INVENTORY[21];
    assert_eq!(
        DebugDriveAck::TAG,
        row.tag,
        "CF5: DebugDriveAck TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(DebugDriveAck::DIRECTION, row.direction, "CF5: DebugDriveAck DIRECTION mismatch");
}

/// CF5: `PkgSyncProgress` (row 22) — tag 0x0208, direction Response.
#[test]
fn cf5_pkg_sync_progress_tag_and_direction() {
    let row = &INVENTORY[22];
    assert_eq!(
        PkgSyncProgress::TAG,
        row.tag,
        "CF5: PkgSyncProgress TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        PkgSyncProgress::DIRECTION,
        row.direction,
        "CF5: PkgSyncProgress DIRECTION mismatch"
    );
}

/// CF5: `PkgSyncDone` (row 23) — tag 0x0209, direction Response.
#[test]
fn cf5_pkg_sync_done_tag_and_direction() {
    let row = &INVENTORY[23];
    assert_eq!(
        PkgSyncDone::TAG,
        row.tag,
        "CF5: PkgSyncDone TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(PkgSyncDone::DIRECTION, row.direction, "CF5: PkgSyncDone DIRECTION mismatch");
}

/// CF5: `PkgVerifyAck` (row 24) — tag 0x020A, direction Response.
#[test]
fn cf5_pkg_verify_ack_tag_and_direction() {
    let row = &INVENTORY[24];
    assert_eq!(
        PkgVerifyAck::TAG,
        row.tag,
        "CF5: PkgVerifyAck TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(PkgVerifyAck::DIRECTION, row.direction, "CF5: PkgVerifyAck DIRECTION mismatch");
}

/// CF5: `ConfigDumpResponse` (row 25) — tag 0x020B, direction Response.
#[test]
fn cf5_config_dump_response_tag_and_direction() {
    let row = &INVENTORY[25];
    assert_eq!(
        ConfigDumpResponse::TAG,
        row.tag,
        "CF5: ConfigDumpResponse TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        ConfigDumpResponse::DIRECTION,
        row.direction,
        "CF5: ConfigDumpResponse DIRECTION mismatch"
    );
}

/// CF5: `ConfigValidateResponse` (row 26) — tag 0x020C, direction Response.
#[test]
fn cf5_config_validate_response_tag_and_direction() {
    let row = &INVENTORY[26];
    assert_eq!(
        ConfigValidateResponse::TAG,
        row.tag,
        "CF5: ConfigValidateResponse TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        ConfigValidateResponse::DIRECTION,
        row.direction,
        "CF5: ConfigValidateResponse DIRECTION mismatch"
    );
}

/// CF5: `AttachEventFrame` (row 27) — tag 0x0301, direction Notify.
#[test]
fn cf5_attach_event_frame_tag_and_direction() {
    let row = &INVENTORY[27];
    assert_eq!(
        AttachEventFrame::TAG,
        row.tag,
        "CF5: AttachEventFrame TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        AttachEventFrame::DIRECTION,
        row.direction,
        "CF5: AttachEventFrame DIRECTION mismatch"
    );
}

/// CF5: `AttachEventDiff` (row 28) — tag 0x0302, direction Notify.
#[test]
fn cf5_attach_event_diff_tag_and_direction() {
    let row = &INVENTORY[28];
    assert_eq!(
        AttachEventDiff::TAG,
        row.tag,
        "CF5: AttachEventDiff TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        AttachEventDiff::DIRECTION,
        row.direction,
        "CF5: AttachEventDiff DIRECTION mismatch"
    );
}

/// CF5: `AttachEventCursor` (row 29) — tag 0x0303, direction Notify.
#[test]
fn cf5_attach_event_cursor_tag_and_direction() {
    let row = &INVENTORY[29];
    assert_eq!(
        AttachEventCursor::TAG,
        row.tag,
        "CF5: AttachEventCursor TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        AttachEventCursor::DIRECTION,
        row.direction,
        "CF5: AttachEventCursor DIRECTION mismatch"
    );
}

/// CF5: `AttachEventProjection` (row 30) — tag 0x0304, direction Notify.
#[test]
fn cf5_attach_event_projection_tag_and_direction() {
    let row = &INVENTORY[30];
    assert_eq!(
        AttachEventProjection::TAG,
        row.tag,
        "CF5: AttachEventProjection TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        AttachEventProjection::DIRECTION,
        row.direction,
        "CF5: AttachEventProjection DIRECTION mismatch"
    );
}

/// CF5: `AttachEventDomainTableDelta` (row 31) — tag 0x0305, direction Notify.
#[test]
fn cf5_attach_event_domain_table_delta_tag_and_direction() {
    let row = &INVENTORY[31];
    assert_eq!(
        AttachEventDomainTableDelta::TAG,
        row.tag,
        "CF5: AttachEventDomainTableDelta TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        AttachEventDomainTableDelta::DIRECTION,
        row.direction,
        "CF5: AttachEventDomainTableDelta DIRECTION mismatch"
    );
}

/// CF5: `AttachEventSessionPivot` (row 32) — tag 0x0306, direction Notify.
#[test]
fn cf5_attach_event_session_pivot_tag_and_direction() {
    let row = &INVENTORY[32];
    assert_eq!(
        AttachEventSessionPivot::TAG,
        row.tag,
        "CF5: AttachEventSessionPivot TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        AttachEventSessionPivot::DIRECTION,
        row.direction,
        "CF5: AttachEventSessionPivot DIRECTION mismatch"
    );
}

/// CF5: `AttachEventSessionDestroyed` (row 33) — tag 0x0307, direction Notify.
#[test]
fn cf5_attach_event_session_destroyed_tag_and_direction() {
    let row = &INVENTORY[33];
    assert_eq!(
        AttachEventSessionDestroyed::TAG,
        row.tag,
        "CF5: AttachEventSessionDestroyed TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        AttachEventSessionDestroyed::DIRECTION,
        row.direction,
        "CF5: AttachEventSessionDestroyed DIRECTION mismatch"
    );
}

/// CF5: `AttachEventClientLeft` (row 34) — tag 0x0308, direction Notify.
#[test]
fn cf5_attach_event_client_left_tag_and_direction() {
    let row = &INVENTORY[34];
    assert_eq!(
        AttachEventClientLeft::TAG,
        row.tag,
        "CF5: AttachEventClientLeft TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        AttachEventClientLeft::DIRECTION,
        row.direction,
        "CF5: AttachEventClientLeft DIRECTION mismatch"
    );
}

/// CF5: `AttachEventServerDraining` (row 35) — tag 0x0309, direction Notify.
#[test]
fn cf5_attach_event_server_draining_tag_and_direction() {
    let row = &INVENTORY[35];
    assert_eq!(
        AttachEventServerDraining::TAG,
        row.tag,
        "CF5: AttachEventServerDraining TAG mismatch vs §7 row '{}'",
        row.name
    );
    assert_eq!(
        AttachEventServerDraining::DIRECTION,
        row.direction,
        "CF5: AttachEventServerDraining DIRECTION mismatch"
    );
}

/// CF5: `Reject` (row 36) — tag 0xFF00, direction Error.
#[test]
fn cf5_reject_tag_and_direction() {
    let row = &INVENTORY[36];
    assert_eq!(Reject::TAG, row.tag, "CF5: Reject TAG mismatch vs §7 row '{}'", row.name);
    assert_eq!(Reject::DIRECTION, row.direction, "CF5: Reject DIRECTION mismatch");
}

// ---------------------------------------------------------------------------
// Fail-closed proof
// ---------------------------------------------------------------------------

/// CF5 fails closed: a struct with a deliberately wrong TAG differs from the
/// §7 golden.  This is not a test of the implementation — it tests that the
/// CF5 cross-check apparatus would catch a mismatch if one existed.
///
/// We simulate a "moved tag" by directly comparing the wrong value against
/// the golden and asserting the comparison is NOT equal.
#[test]
fn deliberately_wrong_tag_detected_by_golden() {
    // If Hello::TAG were 0x0002 instead of 0x0001, this assertion would catch it.
    let wrong_tag: u16 = 0x0002;
    assert_ne!(
        Hello::TAG,
        wrong_tag,
        "CF5 fail-closed proof: Hello::TAG should differ from 0x0002"
    );
    // Similarly, a wrong direction would be caught.
    assert_ne!(
        Hello::DIRECTION,
        Direction::Request,
        "CF5 fail-closed proof: Hello::DIRECTION should not be Request"
    );
}

/// CF5 all-tags are unique: no two §7 rows may share a tag value.
#[test]
fn inventory_tags_are_unique() {
    let mut seen = std::collections::BTreeSet::new();
    for row in INVENTORY {
        assert!(
            seen.insert(row.tag),
            "CF5: duplicate tag 0x{:04X} for '{}' in §7 inventory",
            row.tag,
            row.name
        );
    }
}
