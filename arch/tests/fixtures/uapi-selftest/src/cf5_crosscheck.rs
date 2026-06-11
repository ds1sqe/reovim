//! CF5 inventory cross-check migrated to the no_std selftest runner (#786 Phase 5).
//!
//! Mirrors `uapi/protocol/tests/cf5_crosscheck.rs`.  The only std dependency
//! removed is `std::collections::BTreeSet` in `inventory_tags_are_unique`,
//! replaced with an O(n²) linear scan (37 items — negligible).
//!
//! The libtest originals remain as the bootstrap-state-1 mirror; these
//! selftest registrations are the flight-environment authority.

use reovim_arch::arch_test;
use reovim_uapi_protocol::messages::{
    Attach,
    AttachAck,
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
    Direction,
    Hello,
    HelloAck,
    InputAck,
    Message,
    PkgSync,
    PkgSyncDone,
    PkgSyncProgress,
    PkgVerify,
    PkgVerifyAck,
    Reject,
    RenameSession,
    RenameSessionAck,
    SendInput,
    SwitchSession,
    SwitchSessionAck,
};

// The §7 golden table (tag, direction) — row order matches the spec top-to-bottom.
// 37 rows: 2 handshake + 12 req + 13 resp + 9 notify + 1 error.
const INVENTORY: [(u16, Direction); 37] = [
    // Handshake
    (0x0001, Direction::Handshake), // Hello
    (0x0002, Direction::Handshake), // HelloAck
    // Requests
    (0x0100, Direction::Request),   // Attach
    (0x0101, Direction::Request),   // Detach
    (0x0102, Direction::Request),   // SendInput
    (0x0103, Direction::Request),   // SwitchSession
    (0x0104, Direction::Request),   // DestroySession
    (0x0105, Direction::Request),   // RenameSession
    (0x0106, Direction::Request),   // DebugRead
    (0x0107, Direction::Request),   // DebugDrive
    (0x0108, Direction::Request),   // PkgSync
    (0x0109, Direction::Request),   // PkgVerify
    (0x010A, Direction::Request),   // ConfigDump
    (0x010B, Direction::Request),   // ConfigValidate
    // Responses
    (0x0200, Direction::Response),  // AttachAck
    (0x0201, Direction::Response),  // DetachAck
    (0x0202, Direction::Response),  // InputAck
    (0x0203, Direction::Response),  // SwitchSessionAck
    (0x0204, Direction::Response),  // DestroySessionAck
    (0x0205, Direction::Response),  // RenameSessionAck
    (0x0206, Direction::Response),  // DebugReadEvent
    (0x0207, Direction::Response),  // DebugDriveAck
    (0x0208, Direction::Response),  // PkgSyncProgress
    (0x0209, Direction::Response),  // PkgSyncDone
    (0x020A, Direction::Response),  // PkgVerifyAck
    (0x020B, Direction::Response),  // ConfigDumpResponse
    (0x020C, Direction::Response),  // ConfigValidateResponse
    // Notifications
    (0x0301, Direction::Notify),    // AttachEventFrame
    (0x0302, Direction::Notify),    // AttachEventDiff
    (0x0303, Direction::Notify),    // AttachEventCursor
    (0x0304, Direction::Notify),    // AttachEventProjection
    (0x0305, Direction::Notify),    // AttachEventDomainTableDelta
    (0x0306, Direction::Notify),    // AttachEventSessionPivot
    (0x0307, Direction::Notify),    // AttachEventSessionDestroyed
    (0x0308, Direction::Notify),    // AttachEventClientLeft
    (0x0309, Direction::Notify),    // AttachEventServerDraining
    // Error
    (0xFF00, Direction::Error),     // Reject
];

arch_test!(cf5_inventory_count_is_37, {
    reovim_arch::testrt::check_eq(INVENTORY.len(), 37);
});

arch_test!(cf5_inventory_direction_subcounts, {
    let mut handshake = 0usize;
    let mut req = 0usize;
    let mut resp = 0usize;
    let mut notify = 0usize;
    let mut error = 0usize;
    for (_, dir) in &INVENTORY {
        match dir {
            Direction::Handshake => handshake += 1,
            Direction::Request   => req += 1,
            Direction::Response  => resp += 1,
            Direction::Notify    => notify += 1,
            Direction::Error     => error += 1,
        }
    }
    reovim_arch::testrt::check_eq(handshake, 2);
    reovim_arch::testrt::check_eq(req, 12);
    reovim_arch::testrt::check_eq(resp, 13);
    reovim_arch::testrt::check_eq(notify, 9);
    reovim_arch::testrt::check_eq(error, 1);
});

// Per-struct TAG + DIRECTION assertions

arch_test!(cf5_hello,                 { reovim_arch::testrt::check_eq((Hello::TAG,                 Hello::DIRECTION),                 INVENTORY[0]);  });
arch_test!(cf5_hello_ack,             { reovim_arch::testrt::check_eq((HelloAck::TAG,               HelloAck::DIRECTION),               INVENTORY[1]);  });
arch_test!(cf5_attach,                { reovim_arch::testrt::check_eq((Attach::TAG,                 Attach::DIRECTION),                 INVENTORY[2]);  });
arch_test!(cf5_detach,                { reovim_arch::testrt::check_eq((Detach::TAG,                 Detach::DIRECTION),                 INVENTORY[3]);  });
arch_test!(cf5_send_input,            { reovim_arch::testrt::check_eq((SendInput::TAG,               SendInput::DIRECTION),               INVENTORY[4]);  });
arch_test!(cf5_switch_session,        { reovim_arch::testrt::check_eq((SwitchSession::TAG,           SwitchSession::DIRECTION),           INVENTORY[5]);  });
arch_test!(cf5_destroy_session,       { reovim_arch::testrt::check_eq((DestroySession::TAG,          DestroySession::DIRECTION),          INVENTORY[6]);  });
arch_test!(cf5_rename_session,        { reovim_arch::testrt::check_eq((RenameSession::TAG,           RenameSession::DIRECTION),           INVENTORY[7]);  });
arch_test!(cf5_debug_read,            { reovim_arch::testrt::check_eq((DebugRead::TAG,               DebugRead::DIRECTION),               INVENTORY[8]);  });
arch_test!(cf5_debug_drive,           { reovim_arch::testrt::check_eq((DebugDrive::TAG,              DebugDrive::DIRECTION),              INVENTORY[9]);  });
arch_test!(cf5_pkg_sync,              { reovim_arch::testrt::check_eq((PkgSync::TAG,                 PkgSync::DIRECTION),                 INVENTORY[10]); });
arch_test!(cf5_pkg_verify,            { reovim_arch::testrt::check_eq((PkgVerify::TAG,               PkgVerify::DIRECTION),               INVENTORY[11]); });
arch_test!(cf5_config_dump,           { reovim_arch::testrt::check_eq((ConfigDump::TAG,              ConfigDump::DIRECTION),              INVENTORY[12]); });
arch_test!(cf5_config_validate,       { reovim_arch::testrt::check_eq((ConfigValidate::TAG,          ConfigValidate::DIRECTION),          INVENTORY[13]); });
arch_test!(cf5_attach_ack,            { reovim_arch::testrt::check_eq((AttachAck::TAG,               AttachAck::DIRECTION),               INVENTORY[14]); });
arch_test!(cf5_detach_ack,            { reovim_arch::testrt::check_eq((DetachAck::TAG,               DetachAck::DIRECTION),               INVENTORY[15]); });
arch_test!(cf5_input_ack,             { reovim_arch::testrt::check_eq((InputAck::TAG,                InputAck::DIRECTION),                INVENTORY[16]); });
arch_test!(cf5_switch_session_ack,    { reovim_arch::testrt::check_eq((SwitchSessionAck::TAG,        SwitchSessionAck::DIRECTION),        INVENTORY[17]); });
arch_test!(cf5_destroy_session_ack,   { reovim_arch::testrt::check_eq((DestroySessionAck::TAG,       DestroySessionAck::DIRECTION),       INVENTORY[18]); });
arch_test!(cf5_rename_session_ack,    { reovim_arch::testrt::check_eq((RenameSessionAck::TAG,        RenameSessionAck::DIRECTION),        INVENTORY[19]); });
arch_test!(cf5_debug_read_event,      { reovim_arch::testrt::check_eq((DebugReadEvent::TAG,          DebugReadEvent::DIRECTION),          INVENTORY[20]); });
arch_test!(cf5_debug_drive_ack,       { reovim_arch::testrt::check_eq((DebugDriveAck::TAG,           DebugDriveAck::DIRECTION),           INVENTORY[21]); });
arch_test!(cf5_pkg_sync_progress,     { reovim_arch::testrt::check_eq((PkgSyncProgress::TAG,         PkgSyncProgress::DIRECTION),         INVENTORY[22]); });
arch_test!(cf5_pkg_sync_done,         { reovim_arch::testrt::check_eq((PkgSyncDone::TAG,             PkgSyncDone::DIRECTION),             INVENTORY[23]); });
arch_test!(cf5_pkg_verify_ack,        { reovim_arch::testrt::check_eq((PkgVerifyAck::TAG,            PkgVerifyAck::DIRECTION),            INVENTORY[24]); });
arch_test!(cf5_config_dump_response,  { reovim_arch::testrt::check_eq((ConfigDumpResponse::TAG,      ConfigDumpResponse::DIRECTION),      INVENTORY[25]); });
arch_test!(cf5_config_validate_response, { reovim_arch::testrt::check_eq((ConfigValidateResponse::TAG, ConfigValidateResponse::DIRECTION), INVENTORY[26]); });
arch_test!(cf5_attach_event_frame,    { reovim_arch::testrt::check_eq((AttachEventFrame::TAG,        AttachEventFrame::DIRECTION),        INVENTORY[27]); });
arch_test!(cf5_attach_event_diff,     { reovim_arch::testrt::check_eq((AttachEventDiff::TAG,         AttachEventDiff::DIRECTION),         INVENTORY[28]); });
arch_test!(cf5_attach_event_cursor,   { reovim_arch::testrt::check_eq((AttachEventCursor::TAG,       AttachEventCursor::DIRECTION),       INVENTORY[29]); });
arch_test!(cf5_attach_event_projection, { reovim_arch::testrt::check_eq((AttachEventProjection::TAG, AttachEventProjection::DIRECTION),   INVENTORY[30]); });
arch_test!(cf5_attach_event_domain_table_delta, {
    reovim_arch::testrt::check_eq(
        (AttachEventDomainTableDelta::TAG, AttachEventDomainTableDelta::DIRECTION),
        INVENTORY[31],
    );
});
arch_test!(cf5_attach_event_session_pivot,    { reovim_arch::testrt::check_eq((AttachEventSessionPivot::TAG,    AttachEventSessionPivot::DIRECTION),    INVENTORY[32]); });
arch_test!(cf5_attach_event_session_destroyed, { reovim_arch::testrt::check_eq((AttachEventSessionDestroyed::TAG, AttachEventSessionDestroyed::DIRECTION), INVENTORY[33]); });
arch_test!(cf5_attach_event_client_left,      { reovim_arch::testrt::check_eq((AttachEventClientLeft::TAG,      AttachEventClientLeft::DIRECTION),      INVENTORY[34]); });
arch_test!(cf5_attach_event_server_draining,  { reovim_arch::testrt::check_eq((AttachEventServerDraining::TAG,  AttachEventServerDraining::DIRECTION),  INVENTORY[35]); });
arch_test!(cf5_reject,                        { reovim_arch::testrt::check_eq((Reject::TAG,                     Reject::DIRECTION),                     INVENTORY[36]); });

arch_test!(cf5_deliberately_wrong_tag_detected, {
    // Fail-closed proof: Hello::TAG must not be 0x0002.
    reovim_arch::testrt::check(Hello::TAG != 0x0002, "CF5 fail-closed: Hello::TAG != 0x0002");
    reovim_arch::testrt::check(
        Hello::DIRECTION != Direction::Request,
        "CF5 fail-closed: Hello::DIRECTION != Request",
    );
});

arch_test!(cf5_inventory_tags_are_unique, {
    // O(n²) linear uniqueness check (37 items; no alloc needed).
    let n = INVENTORY.len();
    for i in 0..n {
        for j in (i + 1)..n {
            reovim_arch::testrt::check(
                INVENTORY[i].0 != INVENTORY[j].0,
                "CF5: duplicate tag in §7 inventory",
            );
        }
    }
});
