//! Tests for register types.

use super::{projection::DomainId, register::*};

#[test]
fn register_name_from_str() {
    let name = RegisterName::from("a");
    assert_eq!(name.as_str(), "a");
}

#[test]
fn register_name_multi_char() {
    let name = RegisterName::new("clip.1");
    assert_eq!(name.as_str(), "clip.1");
}

#[test]
fn register_name_display() {
    let name = RegisterName::new("selection.vertices");
    assert_eq!(name.to_string(), "selection.vertices");
}

#[test]
fn register_key_global() {
    let key = RegisterKey::Global(RegisterName::from("+"));
    assert!(matches!(key, RegisterKey::Global(_)));
}

#[test]
fn register_key_domain_local() {
    let key = RegisterKey::DomainLocal(DomainId(1), RegisterName::from("a"));
    assert!(matches!(key, RegisterKey::DomainLocal(_, _)));
}

#[test]
fn register_key_different_domains_not_equal() {
    let text_a = RegisterKey::DomainLocal(DomainId(1), RegisterName::from("a"));
    let mesh_a = RegisterKey::DomainLocal(DomainId(2), RegisterName::from("a"));
    assert_ne!(text_a, mesh_a);
}

#[test]
fn register_key_global_vs_local_not_equal() {
    let global = RegisterKey::Global(RegisterName::from("a"));
    let local = RegisterKey::DomainLocal(DomainId(1), RegisterName::from("a"));
    assert_ne!(global, local);
}
