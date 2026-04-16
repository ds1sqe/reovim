use super::*;

#[test]
fn surface_kind_round_trip() {
    assert_eq!(SurfaceKind::from_u8(1), Some(SurfaceKind::CellGrid));
    assert_eq!(SurfaceKind::from_u8(2), Some(SurfaceKind::Pixel));
    assert_eq!(SurfaceKind::from_u8(3), Some(SurfaceKind::Vr));
    assert_eq!(SurfaceKind::from_u8(4), Some(SurfaceKind::Volumetric));
    assert_eq!(SurfaceKind::from_u8(0), None);
    assert_eq!(SurfaceKind::from_u8(255), None);
}

#[test]
fn cell_grid_command_buffer_new() {
    let buf = cell_grid::CommandBuffer::new(80, 24);
    assert_eq!(buf.width(), 80);
    assert_eq!(buf.height(), 24);
    assert!(buf.is_empty());
    assert_eq!(buf.len(), 0);
}

#[test]
fn cell_grid_command_buffer_from_raw() {
    let data = vec![1, 2, 3, 4];
    let buf = cell_grid::CommandBuffer::from_raw(data.clone(), 120, 40);
    assert_eq!(buf.data(), &[1, 2, 3, 4]);
    assert_eq!(buf.width(), 120);
    assert_eq!(buf.height(), 40);
    assert!(!buf.is_empty());
    assert_eq!(buf.len(), 4);
    assert_eq!(buf.into_data(), data);
}

#[test]
fn pixel_command_buffer_new() {
    let buf = pixel::CommandBuffer::new(1920, 1080);
    assert_eq!(buf.width(), 1920);
    assert_eq!(buf.height(), 1080);
    assert!(buf.is_empty());
}

#[test]
fn pixel_command_buffer_from_raw() {
    let data = vec![0xFF; 16];
    let buf = pixel::CommandBuffer::from_raw(data.clone(), 800, 600);
    assert_eq!(buf.data(), data.as_slice());
    assert_eq!(buf.len(), 16);
    assert_eq!(buf.into_data(), data);
}

#[test]
fn vr_command_buffer_default() {
    let buf = vr::CommandBuffer::default();
    assert!(buf.is_empty());
    assert_eq!(buf.len(), 0);
}

#[test]
fn vr_command_buffer_from_raw() {
    let data = vec![10, 20, 30];
    let buf = vr::CommandBuffer::from_raw(data.clone());
    assert_eq!(buf.data(), &[10, 20, 30]);
    assert_eq!(buf.into_data(), data);
}

#[test]
fn volumetric_command_buffer_default() {
    let buf = volumetric::CommandBuffer::default();
    assert!(buf.is_empty());
    assert_eq!(buf.len(), 0);
}

#[test]
fn volumetric_command_buffer_from_raw() {
    let data = vec![0xDE, 0xAD, 0xBE, 0xEF];
    let buf = volumetric::CommandBuffer::from_raw(data.clone());
    assert_eq!(buf.data(), &[0xDE, 0xAD, 0xBE, 0xEF]);
    assert!(!buf.is_empty());
    assert_eq!(buf.len(), 4);
    assert_eq!(buf.into_data(), data);
}
