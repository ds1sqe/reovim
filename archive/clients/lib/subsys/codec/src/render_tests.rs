use {
    crate::{
        FrameTarget,
        render::{
            DefaultRenderHandlerRegistry, RenderHandler, RenderHandlerError, RenderHandlerRegistry,
        },
    },
    std::sync::Arc,
};

#[derive(Debug, Default, PartialEq, Eq)]
struct StringSlot(String);

struct AppendCharsHandler {
    kind: u16,
    min_len: usize,
}

impl RenderHandler for AppendCharsHandler {
    fn kind(&self) -> u16 {
        self.kind
    }
    fn render(&self, body: &[u8], target: &mut FrameTarget) -> Result<(), RenderHandlerError> {
        if body.len() < self.min_len {
            return Err(RenderHandlerError::TooShort {
                got: body.len(),
                min: self.min_len,
            });
        }
        let Some(slot) = target.get_mut::<StringSlot>() else {
            return Err(RenderHandlerError::CapabilityMissing {
                expected: "StringSlot",
            });
        };
        for &b in body {
            slot.0.push(b as char);
        }
        Ok(())
    }
}

struct AlwaysInvalid;
impl RenderHandler for AlwaysInvalid {
    fn kind(&self) -> u16 {
        0xBAD
    }
    fn render(&self, _body: &[u8], _target: &mut FrameTarget) -> Result<(), RenderHandlerError> {
        Err(RenderHandlerError::InvalidData {
            reason: "always invalid for test",
        })
    }
}

fn registry() -> DefaultRenderHandlerRegistry {
    DefaultRenderHandlerRegistry::new()
}

#[test]
fn new_registry_has_no_handlers() {
    let reg = registry();
    assert!(reg.get(0).is_none());
    assert!(reg.get(0xFFFF).is_none());
}

#[test]
fn register_and_get_round_trip_by_kind() {
    let reg = registry();
    reg.register(Arc::new(AppendCharsHandler {
        kind: 1,
        min_len: 0,
    }));
    let handler = reg.get(1).expect("registered");
    assert_eq!(handler.kind(), 1);
}

#[test]
fn register_replaces_existing_handler_for_same_kind() {
    let reg = registry();
    reg.register(Arc::new(AppendCharsHandler {
        kind: 1,
        min_len: 0,
    }));
    reg.register(Arc::new(AppendCharsHandler {
        kind: 1,
        min_len: 10,
    }));
    let handler = reg.get(1).expect("registered");
    let mut target = FrameTarget::new();
    target.insert(StringSlot::default());
    let err = handler.render(b"short", &mut target).unwrap_err();
    assert_eq!(err, RenderHandlerError::TooShort { got: 5, min: 10 });
}

#[test]
fn unregister_removes_handler() {
    let reg = registry();
    reg.register(Arc::new(AppendCharsHandler {
        kind: 7,
        min_len: 0,
    }));
    assert!(reg.get(7).is_some());
    reg.unregister(7);
    assert!(reg.get(7).is_none());
}

#[test]
fn unregister_missing_kind_is_noop() {
    let reg = registry();
    reg.unregister(42); // should not panic
    assert!(reg.get(42).is_none());
}

#[test]
fn registered_handler_decodes_into_target_slot() {
    let reg = registry();
    reg.register(Arc::new(AppendCharsHandler {
        kind: 1,
        min_len: 0,
    }));
    let handler = reg.get(1).unwrap();
    let mut target = FrameTarget::new();
    target.insert(StringSlot::default());
    handler.render(b"hi", &mut target).expect("ok");
    assert_eq!(target.get::<StringSlot>(), Some(&StringSlot("hi".to_owned())));
}

#[test]
fn handler_errors_when_target_slot_missing() {
    let reg = registry();
    reg.register(Arc::new(AppendCharsHandler {
        kind: 1,
        min_len: 0,
    }));
    let handler = reg.get(1).unwrap();
    let mut target = FrameTarget::new();
    let err = handler.render(b"x", &mut target).unwrap_err();
    assert_eq!(
        err,
        RenderHandlerError::CapabilityMissing {
            expected: "StringSlot"
        }
    );
}

#[test]
fn too_short_error_formats_with_got_and_min() {
    let err = RenderHandlerError::TooShort { got: 2, min: 5 };
    let rendered = format!("{err}");
    assert!(rendered.contains("got 2"));
    assert!(rendered.contains("at least 5"));
}

#[test]
fn invalid_data_error_formats_reason() {
    let err = RenderHandlerError::InvalidData { reason: "oops" };
    assert_eq!(format!("{err}"), "invalid payload: oops");
}

#[test]
fn capability_missing_error_formats_expected() {
    let err = RenderHandlerError::CapabilityMissing {
        expected: "CellCapability",
    };
    assert_eq!(format!("{err}"), "capability missing on target: expected CellCapability");
}

#[test]
fn error_is_std_error_with_no_source() {
    let err = RenderHandlerError::InvalidData { reason: "x" };
    let e: &dyn std::error::Error = &err;
    assert!(e.source().is_none());
}

#[test]
fn error_implements_clone_and_eq() {
    let a = RenderHandlerError::TooShort { got: 1, min: 2 };
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn default_registry_equals_new() {
    let a = DefaultRenderHandlerRegistry::default();
    let b = DefaultRenderHandlerRegistry::new();
    assert!(a.get(0).is_none() && b.get(0).is_none());
}

#[test]
fn debug_impl_reports_registered_kinds() {
    let reg = registry();
    reg.register(Arc::new(AppendCharsHandler {
        kind: 3,
        min_len: 0,
    }));
    reg.register(Arc::new(AlwaysInvalid));
    let rendered = format!("{reg:?}");
    assert!(rendered.contains("DefaultRenderHandlerRegistry"));
    assert!(rendered.contains("registered_kinds"));
}

#[test]
fn registry_supports_multiple_distinct_kinds_concurrently() {
    let reg = registry();
    reg.register(Arc::new(AppendCharsHandler {
        kind: 1,
        min_len: 0,
    }));
    reg.register(Arc::new(AlwaysInvalid)); // kind 0xBAD
    assert!(reg.get(1).is_some());
    assert!(reg.get(0xBAD).is_some());
    assert!(reg.get(2).is_none());
}
