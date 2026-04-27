use {
    crate::{
        FrameTarget,
        surface::{
            DefaultSurfaceEncoderRegistry, SurfaceEncoder, SurfaceEncoderError,
            SurfaceEncoderRegistry,
        },
    },
    std::sync::Arc,
};

#[derive(Debug, Default, PartialEq, Eq)]
struct ByteSlot(Vec<u8>);

struct ByteSlotEncoder {
    kind: u16,
}

impl SurfaceEncoder for ByteSlotEncoder {
    fn kind(&self) -> u16 {
        self.kind
    }
    fn encode_from(&self, target: &FrameTarget) -> Result<Vec<u8>, SurfaceEncoderError> {
        let slot = target
            .get::<ByteSlot>()
            .ok_or(SurfaceEncoderError::CapabilityMissing {
                expected: "ByteSlot",
            })?;
        if slot.0.is_empty() {
            return Err(SurfaceEncoderError::InvalidState {
                reason: "empty byte slot",
            });
        }
        Ok(slot.0.clone())
    }
}

fn registry() -> DefaultSurfaceEncoderRegistry {
    DefaultSurfaceEncoderRegistry::new()
}

#[test]
fn new_registry_has_no_encoders() {
    let reg = registry();
    assert!(reg.get(0).is_none());
    assert!(reg.get(0xFFFF).is_none());
}

#[test]
fn register_and_get_round_trip_by_kind() {
    let reg = registry();
    reg.register(Arc::new(ByteSlotEncoder { kind: 5 }));
    assert_eq!(reg.get(5).map(|e| e.kind()), Some(5));
}

#[test]
fn register_replaces_existing_encoder_for_same_kind() {
    let reg = registry();
    reg.register(Arc::new(ByteSlotEncoder { kind: 5 }));
    reg.register(Arc::new(ByteSlotEncoder { kind: 5 }));
    assert!(reg.get(5).is_some());
}

#[test]
fn unregister_removes_encoder() {
    let reg = registry();
    reg.register(Arc::new(ByteSlotEncoder { kind: 9 }));
    assert!(reg.get(9).is_some());
    reg.unregister(9);
    assert!(reg.get(9).is_none());
}

#[test]
fn unregister_missing_kind_is_noop() {
    let reg = registry();
    reg.unregister(123); // should not panic
    assert!(reg.get(123).is_none());
}

#[test]
fn encoder_reads_slot_and_returns_bytes() {
    let reg = registry();
    reg.register(Arc::new(ByteSlotEncoder { kind: 1 }));
    let enc = reg.get(1).unwrap();
    let mut target = FrameTarget::new();
    target.insert(ByteSlot(vec![1, 2, 3]));
    let bytes = enc.encode_from(&target).unwrap();
    assert_eq!(bytes, vec![1, 2, 3]);
}

#[test]
fn encoder_errors_when_slot_missing() {
    let reg = registry();
    reg.register(Arc::new(ByteSlotEncoder { kind: 1 }));
    let enc = reg.get(1).unwrap();
    let target = FrameTarget::new();
    let err = enc.encode_from(&target).unwrap_err();
    assert_eq!(
        err,
        SurfaceEncoderError::CapabilityMissing {
            expected: "ByteSlot"
        }
    );
}

#[test]
fn encoder_errors_when_slot_state_invalid() {
    let reg = registry();
    reg.register(Arc::new(ByteSlotEncoder { kind: 1 }));
    let enc = reg.get(1).unwrap();
    let mut target = FrameTarget::new();
    target.insert(ByteSlot::default());
    let err = enc.encode_from(&target).unwrap_err();
    assert_eq!(
        err,
        SurfaceEncoderError::InvalidState {
            reason: "empty byte slot"
        }
    );
}

#[test]
fn capability_missing_error_formats_expected() {
    let err = SurfaceEncoderError::CapabilityMissing {
        expected: "CellCapability",
    };
    assert_eq!(format!("{err}"), "capability missing on target: expected CellCapability");
}

#[test]
fn invalid_state_error_formats_reason() {
    let err = SurfaceEncoderError::InvalidState {
        reason: "zero-size",
    };
    assert_eq!(format!("{err}"), "invalid capability state: zero-size");
}

#[test]
fn error_is_std_error_with_no_source() {
    let err = SurfaceEncoderError::InvalidState { reason: "x" };
    let e: &dyn std::error::Error = &err;
    assert!(e.source().is_none());
}

#[test]
fn error_implements_clone_and_eq() {
    let a = SurfaceEncoderError::CapabilityMissing { expected: "X" };
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn default_registry_equals_new() {
    let a = DefaultSurfaceEncoderRegistry::default();
    let b = DefaultSurfaceEncoderRegistry::new();
    assert!(a.get(0).is_none() && b.get(0).is_none());
}

#[test]
fn debug_impl_reports_registered_kinds() {
    let reg = registry();
    reg.register(Arc::new(ByteSlotEncoder { kind: 4 }));
    let rendered = format!("{reg:?}");
    assert!(rendered.contains("DefaultSurfaceEncoderRegistry"));
    assert!(rendered.contains("registered_kinds"));
}
