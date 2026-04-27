use {
    crate::surface_descriptor::{
        DefaultSurfaceDescriptorHandlerRegistry, SurfaceApplyContext, SurfaceDescriptorApplyError,
        SurfaceDescriptorHandler, SurfaceDescriptorHandlerError, SurfaceDescriptorHandlerRegistry,
    },
    std::{any::Any, sync::Arc},
};

#[derive(Debug, Default, PartialEq, Eq)]
struct CaptureCtx {
    last_size: Option<(u16, u16)>,
}

impl SurfaceApplyContext for CaptureCtx {
    fn set_surface_size(&mut self, width: u16, height: u16) {
        self.last_size = Some((width, height));
    }
}

struct ApplyingHandler;
impl SurfaceDescriptorHandler for ApplyingHandler {
    fn kind(&self) -> u16 {
        0x1111
    }
    fn decode(&self, body: &[u8]) -> Result<Box<dyn Any + Send>, SurfaceDescriptorHandlerError> {
        if body.len() < 2 {
            return Err(SurfaceDescriptorHandlerError::TooShort {
                got: body.len(),
                min: 2,
            });
        }
        Ok(Box::new((u16::from(body[0]), u16::from(body[1]))))
    }
    fn decode_and_apply(
        &self,
        body: &[u8],
        ctx: &mut dyn SurfaceApplyContext,
    ) -> Result<(), SurfaceDescriptorApplyError> {
        let boxed = self.decode(body)?;
        let (w, h) = *boxed.downcast::<(u16, u16)>().expect("own type");
        if w == 0 || h == 0 {
            return Err(SurfaceDescriptorApplyError::StateApplyRejected {
                reason: "zero dimension",
            });
        }
        ctx.set_surface_size(w, h);
        Ok(())
    }
}

struct RejectingHandler;
impl SurfaceDescriptorHandler for RejectingHandler {
    fn kind(&self) -> u16 {
        0x2222
    }
    fn decode(&self, _body: &[u8]) -> Result<Box<dyn Any + Send>, SurfaceDescriptorHandlerError> {
        Ok(Box::new(()))
    }
    fn decode_and_apply(
        &self,
        _body: &[u8],
        _ctx: &mut dyn SurfaceApplyContext,
    ) -> Result<(), SurfaceDescriptorApplyError> {
        Err(SurfaceDescriptorApplyError::Contextual {
            reason: "unavailable",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestInfo {
    a: u16,
    b: u16,
}

struct TestHandler {
    kind: u16,
    min_len: usize,
}

impl SurfaceDescriptorHandler for TestHandler {
    fn kind(&self) -> u16 {
        self.kind
    }
    fn decode(&self, body: &[u8]) -> Result<Box<dyn Any + Send>, SurfaceDescriptorHandlerError> {
        if body.len() < self.min_len {
            return Err(SurfaceDescriptorHandlerError::TooShort {
                got: body.len(),
                min: self.min_len,
            });
        }
        Ok(Box::new(TestInfo {
            a: u16::from(body[0]),
            b: u16::from(body[1]),
        }))
    }
}

struct AlwaysInvalidHandler;
impl SurfaceDescriptorHandler for AlwaysInvalidHandler {
    fn kind(&self) -> u16 {
        0xDEAD
    }
    fn decode(&self, _body: &[u8]) -> Result<Box<dyn Any + Send>, SurfaceDescriptorHandlerError> {
        Err(SurfaceDescriptorHandlerError::InvalidData {
            reason: "always invalid",
        })
    }
}

struct AlwaysOutOfRangeHandler;
impl SurfaceDescriptorHandler for AlwaysOutOfRangeHandler {
    fn kind(&self) -> u16 {
        0xBEEF
    }
    fn decode(&self, _body: &[u8]) -> Result<Box<dyn Any + Send>, SurfaceDescriptorHandlerError> {
        Err(SurfaceDescriptorHandlerError::OutOfRange {
            reason: "always out of range",
        })
    }
}

fn registry() -> DefaultSurfaceDescriptorHandlerRegistry {
    DefaultSurfaceDescriptorHandlerRegistry::new()
}

#[test]
fn new_registry_has_no_handlers() {
    let reg = registry();
    assert!(reg.get(0).is_none());
    assert!(reg.get(0xFFFF).is_none());
}

#[test]
fn default_registry_equals_new() {
    let a = DefaultSurfaceDescriptorHandlerRegistry::default();
    let b = DefaultSurfaceDescriptorHandlerRegistry::new();
    assert!(a.get(0).is_none() && b.get(0).is_none());
}

#[test]
fn register_and_get_round_trip_by_kind() {
    let reg = registry();
    reg.register(Arc::new(TestHandler {
        kind: 7,
        min_len: 2,
    }));
    assert_eq!(reg.get(7).map(|h| h.kind()), Some(7));
}

#[test]
fn register_replaces_existing_handler_for_same_kind() {
    let reg = registry();
    reg.register(Arc::new(TestHandler {
        kind: 1,
        min_len: 0,
    }));
    reg.register(Arc::new(TestHandler {
        kind: 1,
        min_len: 10,
    }));
    let handler = reg.get(1).expect("registered");
    let err = handler.decode(b"short").unwrap_err();
    assert_eq!(err, SurfaceDescriptorHandlerError::TooShort { got: 5, min: 10 });
}

#[test]
fn unregister_removes_handler() {
    let reg = registry();
    reg.register(Arc::new(TestHandler {
        kind: 9,
        min_len: 0,
    }));
    assert!(reg.get(9).is_some());
    reg.unregister(9);
    assert!(reg.get(9).is_none());
}

#[test]
fn unregister_missing_kind_is_noop() {
    let reg = registry();
    reg.unregister(42);
    assert!(reg.get(42).is_none());
}

#[test]
fn get_returns_none_for_unknown_kind_on_populated_registry() {
    let reg = registry();
    reg.register(Arc::new(TestHandler {
        kind: 1,
        min_len: 0,
    }));
    reg.register(Arc::new(AlwaysInvalidHandler));
    assert!(reg.get(99).is_none());
    assert!(reg.get(0).is_none());
}

#[test]
fn registered_handler_decodes_to_typed_descriptor_via_downcast() {
    let reg = registry();
    reg.register(Arc::new(TestHandler {
        kind: 1,
        min_len: 2,
    }));
    let handler = reg.get(1).unwrap();
    let boxed = handler.decode(&[3, 4, 5]).expect("ok");
    let info = boxed
        .downcast::<TestInfo>()
        .expect("downcast to TestInfo succeeds");
    assert_eq!(*info, TestInfo { a: 3, b: 4 });
}

#[test]
fn too_short_error_round_trips_get_and_min() {
    let err = SurfaceDescriptorHandlerError::TooShort { got: 2, min: 8 };
    let rendered = format!("{err}");
    assert!(rendered.contains("got 2"));
    assert!(rendered.contains("at least 8"));
}

#[test]
fn invalid_data_error_formats_reason() {
    let err = SurfaceDescriptorHandlerError::InvalidData { reason: "oops" };
    assert_eq!(format!("{err}"), "invalid payload: oops");
}

#[test]
fn out_of_range_error_formats_reason() {
    let err = SurfaceDescriptorHandlerError::OutOfRange {
        reason: "width exceeds u16::MAX",
    };
    assert_eq!(format!("{err}"), "value out of range: width exceeds u16::MAX");
}

#[test]
fn error_is_std_error_with_no_source() {
    let err = SurfaceDescriptorHandlerError::InvalidData { reason: "x" };
    let e: &dyn std::error::Error = &err;
    assert!(e.source().is_none());
}

#[test]
fn error_implements_clone_and_eq() {
    let a = SurfaceDescriptorHandlerError::TooShort { got: 1, min: 2 };
    let b = a.clone();
    assert_eq!(a, b);
    let c = SurfaceDescriptorHandlerError::OutOfRange { reason: "x" };
    let d = c.clone();
    assert_eq!(c, d);
}

#[test]
fn debug_impl_reports_registered_kinds() {
    let reg = registry();
    reg.register(Arc::new(TestHandler {
        kind: 3,
        min_len: 0,
    }));
    reg.register(Arc::new(AlwaysOutOfRangeHandler));
    let rendered = format!("{reg:?}");
    assert!(rendered.contains("DefaultSurfaceDescriptorHandlerRegistry"));
    assert!(rendered.contains("registered_kinds"));
}

#[test]
fn registry_supports_multiple_distinct_kinds_concurrently() {
    let reg = registry();
    reg.register(Arc::new(TestHandler {
        kind: 1,
        min_len: 0,
    }));
    reg.register(Arc::new(AlwaysInvalidHandler));
    reg.register(Arc::new(AlwaysOutOfRangeHandler));
    assert!(reg.get(1).is_some());
    assert!(reg.get(0xDEAD).is_some());
    assert!(reg.get(0xBEEF).is_some());
    assert!(reg.get(2).is_none());
}

#[test]
fn handler_decode_invalid_data_surfaces_through_registry() {
    let reg = registry();
    reg.register(Arc::new(AlwaysInvalidHandler));
    let handler = reg.get(0xDEAD).unwrap();
    let err = handler.decode(b"ignored").unwrap_err();
    assert_eq!(
        err,
        SurfaceDescriptorHandlerError::InvalidData {
            reason: "always invalid"
        }
    );
}

// =========================================================================
// Plan 17-β.2a: decode_and_apply + SurfaceApplyContext tests
// =========================================================================

#[test]
fn decode_and_apply_default_impl_runs_decode_without_side_effect() {
    let handler = TestHandler {
        kind: 7,
        min_len: 2,
    };
    let mut ctx = CaptureCtx::default();
    handler.decode_and_apply(&[3, 4], &mut ctx).expect("ok");
    assert_eq!(ctx.last_size, None, "default impl has no side-effect");
}

#[test]
fn decode_and_apply_default_impl_propagates_decode_error() {
    let handler = TestHandler {
        kind: 7,
        min_len: 10,
    };
    let mut ctx = CaptureCtx::default();
    let err = handler.decode_and_apply(b"short", &mut ctx).unwrap_err();
    assert!(matches!(err, SurfaceDescriptorApplyError::Decode(_)));
}

#[test]
fn overriding_handler_applies_to_context() {
    let handler = ApplyingHandler;
    let mut ctx = CaptureCtx::default();
    handler.decode_and_apply(&[80, 24], &mut ctx).expect("ok");
    assert_eq!(ctx.last_size, Some((80, 24)));
}

#[test]
fn overriding_handler_rejects_zero_dim_with_state_apply_rejected() {
    let handler = ApplyingHandler;
    let mut ctx = CaptureCtx::default();
    let err = handler.decode_and_apply(&[0, 24], &mut ctx).unwrap_err();
    assert_eq!(
        err,
        SurfaceDescriptorApplyError::StateApplyRejected {
            reason: "zero dimension"
        }
    );
    assert_eq!(ctx.last_size, None);
}

#[test]
fn contextual_failure_variant_surfaces_from_handler() {
    let handler = RejectingHandler;
    let mut ctx = CaptureCtx::default();
    let err = handler.decode_and_apply(b"any", &mut ctx).unwrap_err();
    assert_eq!(
        err,
        SurfaceDescriptorApplyError::Contextual {
            reason: "unavailable"
        }
    );
}

#[test]
fn apply_error_decode_from_blanket_impl_wraps_decode_error() {
    let e = SurfaceDescriptorHandlerError::TooShort { got: 1, min: 2 };
    let wrapped: SurfaceDescriptorApplyError = e.clone().into();
    assert_eq!(wrapped, SurfaceDescriptorApplyError::Decode(e));
}

#[test]
fn apply_error_displays_each_variant_distinctly() {
    let a = SurfaceDescriptorApplyError::Decode(SurfaceDescriptorHandlerError::InvalidData {
        reason: "x",
    });
    let b = SurfaceDescriptorApplyError::StateApplyRejected { reason: "y" };
    let c = SurfaceDescriptorApplyError::Contextual { reason: "z" };
    assert!(format!("{a}").contains("decode failed"));
    assert!(format!("{b}").contains("rejected"));
    assert!(format!("{c}").contains("apply failure"));
}

#[test]
fn apply_error_decode_variant_exposes_source() {
    let inner = SurfaceDescriptorHandlerError::InvalidData { reason: "x" };
    let wrapped = SurfaceDescriptorApplyError::Decode(inner);
    let e: &dyn std::error::Error = &wrapped;
    assert!(e.source().is_some());
}

#[test]
fn apply_error_other_variants_have_no_source() {
    let a = SurfaceDescriptorApplyError::StateApplyRejected { reason: "x" };
    let b = SurfaceDescriptorApplyError::Contextual { reason: "y" };
    let ae: &dyn std::error::Error = &a;
    let be: &dyn std::error::Error = &b;
    assert!(ae.source().is_none());
    assert!(be.source().is_none());
}

#[test]
fn apply_error_implements_clone_and_eq() {
    let a = SurfaceDescriptorApplyError::StateApplyRejected { reason: "x" };
    let b = a.clone();
    assert_eq!(a, b);
}
