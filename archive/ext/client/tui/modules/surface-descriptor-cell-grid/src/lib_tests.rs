use {
    crate::{CELL_GRID_BODY_LEN, CellGridSurfaceHandler, CellGridSurfaceInfo, KIND_CELL_GRID},
    reovim_client_subsys_codec::{
        SurfaceApplyContext, SurfaceDescriptorApplyError, SurfaceDescriptorHandler,
        SurfaceDescriptorHandlerError,
    },
};

#[derive(Debug, Default, PartialEq, Eq)]
struct RecordingCtx {
    last: Option<(u16, u16)>,
}
impl SurfaceApplyContext for RecordingCtx {
    fn set_surface_size(&mut self, width: u16, height: u16) {
        self.last = Some((width, height));
    }
}

fn handler() -> CellGridSurfaceHandler {
    CellGridSurfaceHandler::new()
}

fn be_bytes(width: u32, height: u32) -> [u8; 8] {
    let mut body = [0u8; 8];
    body[..4].copy_from_slice(&width.to_be_bytes());
    body[4..].copy_from_slice(&height.to_be_bytes());
    body
}

#[test]
fn kind_is_cell_grid_constant() {
    assert_eq!(handler().kind(), KIND_CELL_GRID);
    assert_eq!(KIND_CELL_GRID, 0x0001);
}

#[test]
fn body_len_constant_is_8() {
    assert_eq!(CELL_GRID_BODY_LEN, 8);
}

#[test]
fn valid_body_decodes_to_width_and_height() {
    let body = be_bytes(80, 24);
    let boxed = handler().decode(&body).expect("ok");
    let info = boxed
        .downcast::<CellGridSurfaceInfo>()
        .expect("downcast to CellGridSurfaceInfo");
    assert_eq!(
        *info,
        CellGridSurfaceInfo {
            width: 80,
            height: 24
        }
    );
}

#[test]
fn valid_body_with_zero_dims_decodes_without_error() {
    // Zero dimensions are a valid codec output. The consuming layer
    // (notification handler) is responsible for deciding whether to
    // act on them — not the codec.
    let body = be_bytes(0, 0);
    let boxed = handler().decode(&body).expect("ok");
    let info = boxed.downcast::<CellGridSurfaceInfo>().unwrap();
    assert_eq!(
        *info,
        CellGridSurfaceInfo {
            width: 0,
            height: 0
        }
    );
}

#[test]
fn zero_width_nonzero_height_decodes() {
    let body = be_bytes(0, 10);
    let info = handler()
        .decode(&body)
        .unwrap()
        .downcast::<CellGridSurfaceInfo>()
        .unwrap();
    assert_eq!(
        *info,
        CellGridSurfaceInfo {
            width: 0,
            height: 10
        }
    );
}

#[test]
fn body_too_short_returns_too_short_error() {
    let err = handler().decode(&[1, 2, 3]).unwrap_err();
    assert_eq!(err, SurfaceDescriptorHandlerError::TooShort { got: 3, min: 8 });
}

#[test]
fn empty_body_returns_too_short_error() {
    let err = handler().decode(&[]).unwrap_err();
    assert_eq!(err, SurfaceDescriptorHandlerError::TooShort { got: 0, min: 8 });
}

#[test]
fn width_u32_max_is_out_of_range() {
    let body = be_bytes(u32::MAX, 10);
    let err = handler().decode(&body).unwrap_err();
    assert_eq!(
        err,
        SurfaceDescriptorHandlerError::OutOfRange {
            reason: "width exceeds u16::MAX"
        }
    );
}

#[test]
fn height_u32_max_is_out_of_range() {
    let body = be_bytes(10, u32::MAX);
    let err = handler().decode(&body).unwrap_err();
    assert_eq!(
        err,
        SurfaceDescriptorHandlerError::OutOfRange {
            reason: "height exceeds u16::MAX"
        }
    );
}

#[test]
fn width_just_over_u16_max_is_out_of_range() {
    let body = be_bytes(u32::from(u16::MAX) + 1, 10);
    let err = handler().decode(&body).unwrap_err();
    assert!(matches!(err, SurfaceDescriptorHandlerError::OutOfRange { .. }));
}

#[test]
fn width_at_u16_max_is_accepted() {
    let body = be_bytes(u32::from(u16::MAX), 10);
    let info = handler()
        .decode(&body)
        .unwrap()
        .downcast::<CellGridSurfaceInfo>()
        .unwrap();
    assert_eq!(info.width, u16::MAX);
    assert_eq!(info.height, 10);
}

#[test]
fn body_with_trailing_bytes_is_accepted_but_ignored() {
    // The codec reads the first 8 bytes; trailing bytes are forward-
    // compatible extension surface (no strict-length policy).
    let mut body = be_bytes(80, 24).to_vec();
    body.extend_from_slice(&[0xFF, 0xFF]);
    let info = handler()
        .decode(&body)
        .unwrap()
        .downcast::<CellGridSurfaceInfo>()
        .unwrap();
    assert_eq!(
        *info,
        CellGridSurfaceInfo {
            width: 80,
            height: 24
        }
    );
}

#[test]
fn handler_constructors_are_equivalent() {
    let a = CellGridSurfaceHandler::new();
    let b = CellGridSurfaceHandler;
    assert_eq!(a.kind(), b.kind());
}

#[test]
fn handler_is_copy() {
    let a = CellGridSurfaceHandler::new();
    let b = a;
    let c = a;
    assert_eq!(a.kind(), b.kind());
    assert_eq!(a.kind(), c.kind());
}

#[test]
fn cell_grid_surface_info_implements_copy_and_eq() {
    let a = CellGridSurfaceInfo {
        width: 1,
        height: 2,
    };
    let b = a;
    let c = a;
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_ne!(
        a,
        CellGridSurfaceInfo {
            width: 2,
            height: 1
        }
    );
}

// =========================================================================
// Plan 17-β.2a: decode_and_apply override tests
// =========================================================================

#[test]
fn decode_and_apply_valid_body_calls_set_surface_size() {
    let mut ctx = RecordingCtx::default();
    handler()
        .decode_and_apply(&be_bytes(80, 24), &mut ctx)
        .expect("ok");
    assert_eq!(ctx.last, Some((80, 24)));
}

#[test]
fn decode_and_apply_zero_width_rejects_without_calling_ctx() {
    let mut ctx = RecordingCtx::default();
    let err = handler()
        .decode_and_apply(&be_bytes(0, 24), &mut ctx)
        .unwrap_err();
    assert_eq!(
        err,
        SurfaceDescriptorApplyError::StateApplyRejected {
            reason: "zero surface dimension"
        }
    );
    assert_eq!(ctx.last, None);
}

#[test]
fn decode_and_apply_zero_height_rejects_without_calling_ctx() {
    let mut ctx = RecordingCtx::default();
    let err = handler()
        .decode_and_apply(&be_bytes(80, 0), &mut ctx)
        .unwrap_err();
    assert_eq!(
        err,
        SurfaceDescriptorApplyError::StateApplyRejected {
            reason: "zero surface dimension"
        }
    );
    assert_eq!(ctx.last, None);
}

#[test]
fn decode_and_apply_too_short_body_propagates_decode_error() {
    let mut ctx = RecordingCtx::default();
    let err = handler()
        .decode_and_apply(&[1, 2, 3], &mut ctx)
        .unwrap_err();
    assert!(matches!(
        err,
        SurfaceDescriptorApplyError::Decode(SurfaceDescriptorHandlerError::TooShort {
            got: 3,
            min: 8
        })
    ));
    assert_eq!(ctx.last, None);
}

#[test]
fn decode_and_apply_u32_overflow_propagates_decode_error() {
    let mut ctx = RecordingCtx::default();
    let err = handler()
        .decode_and_apply(&be_bytes(u32::MAX, 24), &mut ctx)
        .unwrap_err();
    assert!(matches!(
        err,
        SurfaceDescriptorApplyError::Decode(SurfaceDescriptorHandlerError::OutOfRange { .. })
    ));
    assert_eq!(ctx.last, None);
}

// =========================================================================
// Plan 17-β.2b Phase A: decode_info direct-helper tests (F1 fold)
// =========================================================================

#[test]
fn decode_info_valid_body_returns_typed_info() {
    let info = super::decode_info(&be_bytes(80, 24)).expect("ok");
    assert_eq!(
        info,
        CellGridSurfaceInfo {
            width: 80,
            height: 24
        }
    );
}

#[test]
fn decode_info_too_short_body_returns_too_short_error() {
    let err = super::decode_info(&[1, 2, 3]).unwrap_err();
    assert_eq!(err, SurfaceDescriptorHandlerError::TooShort { got: 3, min: 8 });
}

#[test]
fn decode_info_width_overflow_returns_out_of_range() {
    let err = super::decode_info(&be_bytes(u32::MAX, 24)).unwrap_err();
    assert_eq!(
        err,
        SurfaceDescriptorHandlerError::OutOfRange {
            reason: "width exceeds u16::MAX"
        }
    );
}
