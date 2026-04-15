//! Tests for ProjectionStore.

use super::projection_store::*;
use reovim_subsys_coordination::{
    DomainId, Projection, ProjectionDelivery, ProjectionTag,
};
use reovim_subsys_session::ClientId;

fn make_persistent(tag: &str, payload: &[u8]) -> Projection {
    Projection {
        tag: ProjectionTag::from(tag),
        domain_id: DomainId(1),
        window_id: None,
        payload: payload.to_vec(),
        display: None,
        delivery: ProjectionDelivery::Persistent,
    }
}

fn make_transient(tag: &str, payload: &[u8]) -> Projection {
    Projection {
        tag: ProjectionTag::from(tag),
        domain_id: DomainId(1),
        window_id: None,
        payload: payload.to_vec(),
        display: None,
        delivery: ProjectionDelivery::Transient,
    }
}

fn cid(n: usize) -> ClientId {
    ClientId::new(n)
}

#[test]
fn seed_stores_persistent() {
    let mut store = ProjectionStore::new();
    store.seed(cid(1), vec![make_persistent("text.mode", b"NORMAL")]);
    let all = store.get_all(cid(1));
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].projection.tag.as_str(), "text.mode");
}

#[test]
#[should_panic(expected = "initial_projections must return only Persistent")]
fn seed_asserts_on_transient_in_debug() {
    let mut store = ProjectionStore::new();
    // Transient in initial_projections triggers debug_assert (D21)
    store.seed(cid(1), vec![make_transient("platform.haptic", &[0xFF])]);
}

#[test]
fn update_returns_changed() {
    let mut store = ProjectionStore::new();
    let result = store.update(cid(1), vec![make_persistent("text.mode", b"NORMAL")]);
    assert_eq!(result.changed.len(), 1);
    assert!(result.transient.is_empty());
}

#[test]
fn update_deduplicates_same_payload() {
    let mut store = ProjectionStore::new();
    store.update(cid(1), vec![make_persistent("text.mode", b"NORMAL")]);

    // Same payload — should not produce a change
    let result = store.update(cid(1), vec![make_persistent("text.mode", b"NORMAL")]);
    assert!(result.changed.is_empty());
}

#[test]
fn update_detects_payload_change() {
    let mut store = ProjectionStore::new();
    store.update(cid(1), vec![make_persistent("text.mode", b"NORMAL")]);

    // Different payload — should produce a change
    let result = store.update(cid(1), vec![make_persistent("text.mode", b"INSERT")]);
    assert_eq!(result.changed.len(), 1);
    assert_eq!(result.changed[0].projection.payload, b"INSERT");
}

#[test]
fn update_forwards_transient() {
    let mut store = ProjectionStore::new();
    let result = store.update(
        cid(1),
        vec![make_transient("platform.haptic", &[0xFF])],
    );
    assert!(result.changed.is_empty());
    assert_eq!(result.transient.len(), 1);
}

#[test]
fn versions_monotonically_increase() {
    let mut store = ProjectionStore::new();
    let r1 = store.update(cid(1), vec![make_persistent("text.mode", b"NORMAL")]);
    let r2 = store.update(cid(1), vec![make_persistent("text.mode", b"INSERT")]);
    assert!(r2.changed[0].version > r1.changed[0].version);
}

#[test]
fn remove_client() {
    let mut store = ProjectionStore::new();
    store.update(cid(1), vec![make_persistent("text.mode", b"NORMAL")]);
    store.update(cid(2), vec![make_persistent("text.mode", b"INSERT")]);
    store.remove_client(cid(1));
    assert!(store.get_all(cid(1)).is_empty());
    assert_eq!(store.get_all(cid(2)).len(), 1);
}

#[test]
fn remove_domain() {
    let mut store = ProjectionStore::new();
    store.update(cid(1), vec![make_persistent("text.mode", b"NORMAL")]);
    store.remove_domain(DomainId(1));
    assert!(store.get_all(cid(1)).is_empty());
}

#[test]
fn multiple_tags_per_client() {
    let mut store = ProjectionStore::new();
    store.update(
        cid(1),
        vec![
            make_persistent("text.mode", b"NORMAL"),
            make_persistent("text.cursor", b"\x00\x01"),
        ],
    );
    assert_eq!(store.get_all(cid(1)).len(), 2);
}

#[test]
fn session_has_projection_store() {
    use crate::session::{Session, SessionId};
    let session = Session::new(SessionId::new("proj-test"));
    let store = session.projection_store().read();
    assert!(store.get_all(cid(1)).is_empty());
}
