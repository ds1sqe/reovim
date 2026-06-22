//! Tests for `carrier.rs` — classify_tag, reject_reason_to_error, and
//! structural invariants of the carrier.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (arch_test! + testrt::run). The server-selftest bin in
//! tests/fixtures/ runs these.
//!
//! Full end-to-end smoke (Hello→Attach→SendInput over a real UDS) lives in
//! the server-selftest fixture bin.

use reovim_arch::arch_test;

use reovim_uapi::{
    abi::ErrorCode,
    protocol::{
        messages::{Direction, Hello, HelloAck, Message},
        state::TagInfo,
    },
};

use {
    crate::carrier::{classify_tag, reject_reason_to_error},
    reovim_uapi::protocol::state::RejectReason,
};

arch_test!(classify_hello_tag_is_handshake, {
    assert_eq!(classify_tag(Hello::TAG), TagInfo::Known(Direction::Handshake));
});

arch_test!(classify_hello_ack_tag_is_handshake, {
    assert_eq!(classify_tag(HelloAck::TAG), TagInfo::Known(Direction::Handshake));
});

arch_test!(classify_attach_tag_is_request, {
    // Attach::TAG = 0x0100
    use reovim_uapi::protocol::messages::Attach;
    assert_eq!(classify_tag(Attach::TAG), TagInfo::Known(Direction::Request));
});

arch_test!(classify_send_input_tag_is_request, {
    use reovim_uapi::protocol::messages::SendInput;
    assert_eq!(classify_tag(SendInput::TAG), TagInfo::Known(Direction::Request));
});

arch_test!(classify_attach_ack_tag_is_response, {
    use reovim_uapi::protocol::messages::AttachAck;
    assert_eq!(classify_tag(AttachAck::TAG), TagInfo::Known(Direction::Response));
});

arch_test!(classify_reject_tag_is_error, {
    use reovim_uapi::protocol::messages::Reject;
    assert_eq!(classify_tag(Reject::TAG), TagInfo::Known(Direction::Error));
});

arch_test!(classify_unknown_tag_is_unknown, {
    assert_eq!(classify_tag(0x7000), TagInfo::Unknown);
});

arch_test!(reject_reason_handshake_expected_maps_to_protocol_violation, {
    assert_eq!(
        reject_reason_to_error(RejectReason::HandshakeExpected),
        ErrorCode::ProtocolViolation
    );
});

arch_test!(reject_reason_unexpected_handshake_maps_to_protocol_violation, {
    assert_eq!(
        reject_reason_to_error(RejectReason::UnexpectedHandshake),
        ErrorCode::ProtocolViolation
    );
});

arch_test!(reject_reason_unknown_request_maps_to_protocol_violation, {
    assert_eq!(
        reject_reason_to_error(RejectReason::UnknownRequest),
        ErrorCode::ProtocolViolation
    );
});

arch_test!(reject_reason_correlation_violation_maps_to_protocol_violation, {
    assert_eq!(
        reject_reason_to_error(RejectReason::CorrelationViolation),
        ErrorCode::ProtocolViolation
    );
});

arch_test!(reject_reason_bad_flags_maps_to_protocol_violation, {
    assert_eq!(reject_reason_to_error(RejectReason::BadFlags), ErrorCode::ProtocolViolation);
});
