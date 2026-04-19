use super::SurfaceDescriptor;

#[test]
fn test_surface_descriptor_new_and_accessors() {
    let body = vec![0x01u8, 0x02, 0x03];
    let sd = SurfaceDescriptor::new(SurfaceDescriptor::KIND_CELL_GRID, body.clone());
    assert_eq!(sd.kind(), SurfaceDescriptor::KIND_CELL_GRID);
    assert_eq!(sd.body(), body.as_slice());
}

#[test]
fn test_surface_descriptor_empty_body() {
    let sd = SurfaceDescriptor::new(SurfaceDescriptor::KIND_PIXEL_BUFFER, vec![]);
    assert_eq!(sd.kind(), SurfaceDescriptor::KIND_PIXEL_BUFFER);
    assert!(sd.body().is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_surface_descriptor_debug() {
    let sd = SurfaceDescriptor::new(SurfaceDescriptor::KIND_VR_SCENE, vec![0xAB]);
    let s = format!("{sd:?}");
    assert!(s.contains("SurfaceDescriptor"));
}

#[test]
fn test_surface_descriptor_clone_and_eq() {
    let a = SurfaceDescriptor::new(SurfaceDescriptor::KIND_VOLUMETRIC, vec![1, 2, 3]);
    let b = a.clone();
    assert_eq!(a, b);

    let c = SurfaceDescriptor::new(SurfaceDescriptor::KIND_VOLUMETRIC, vec![1, 2, 4]);
    assert_ne!(a, c);
}

#[test]
fn test_surface_descriptor_kind_constants() {
    assert_eq!(SurfaceDescriptor::KIND_CELL_GRID, 0x0001);
    assert_eq!(SurfaceDescriptor::KIND_PIXEL_BUFFER, 0x0002);
    assert_eq!(SurfaceDescriptor::KIND_VR_SCENE, 0x0003);
    assert_eq!(SurfaceDescriptor::KIND_VOLUMETRIC, 0x0004);
    assert_eq!(SurfaceDescriptor::KIND_NEURAL, 0x0005);
}
