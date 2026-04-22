//! Tests for the process-global surface-descriptor registry.
//!
//! Because `REGISTRY` is a `static OnceLock`, these tests share the
//! same cell. We rely on the idempotence contract:
//! `register_builtin_surface_handlers` is a no-op on subsequent calls
//! and always returns the same `Arc`. Tests below only read, they
//! never assume the registry starts empty.

use {
    super::{global_registry, register_builtin_surface_handlers},
    reovim_client_subsys_codec::SurfaceDescriptorHandlerRegistry,
    reovim_tui_mod_surface_descriptor_cell_grid::{CellGridSurfaceInfo, KIND_CELL_GRID},
};

#[test]
fn register_builtin_returns_populated_registry() {
    let reg = register_builtin_surface_handlers();
    assert!(reg.get(KIND_CELL_GRID).is_some());
}

#[test]
fn global_registry_mirrors_initialised_state() {
    register_builtin_surface_handlers();
    let reg = global_registry().expect("registry initialised");
    assert!(reg.get(KIND_CELL_GRID).is_some());
}

#[test]
fn unknown_kind_returns_none_from_global_registry() {
    let reg = register_builtin_surface_handlers();
    assert!(reg.get(0xDEAD).is_none());
    assert!(reg.get(0xFFFF).is_none());
}

#[test]
fn registered_handler_decodes_cell_grid_body() {
    let reg = register_builtin_surface_handlers();
    let handler = reg.get(KIND_CELL_GRID).unwrap();
    let body = {
        let mut b = [0u8; 8];
        b[..4].copy_from_slice(&80u32.to_be_bytes());
        b[4..].copy_from_slice(&24u32.to_be_bytes());
        b
    };
    let boxed = handler.decode(&body).expect("ok");
    let info = boxed.downcast::<CellGridSurfaceInfo>().unwrap();
    assert_eq!(
        *info,
        CellGridSurfaceInfo {
            width: 80,
            height: 24
        }
    );
}

#[test]
fn register_is_idempotent_returns_same_registry() {
    let first = register_builtin_surface_handlers();
    let second = register_builtin_surface_handlers();
    assert!(Arc::ptr_eq(&first, &second));
}

use std::sync::Arc;
