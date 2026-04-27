//! Tests for `BindingLayer` and `BindingInfo`.

use {
    super::binding::{BindingInfo, BindingLayer},
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

fn cmd(name: &'static str) -> CommandId {
    CommandId::new(ModuleId::new("test"), name)
}

#[test]
fn test_binding_layer_order() {
    assert!(BindingLayer::Base < BindingLayer::Policy);
    assert!(BindingLayer::Policy < BindingLayer::User);
}

#[test]
fn test_binding_layer_all() {
    let layers = BindingLayer::all();
    assert_eq!(layers[0], BindingLayer::Base);
    assert_eq!(layers[1], BindingLayer::Policy);
    assert_eq!(layers[2], BindingLayer::User);
}

#[test]
fn test_binding_layer_names() {
    assert_eq!(BindingLayer::Base.name(), "base");
    assert_eq!(BindingLayer::Policy.name(), "policy");
    assert_eq!(BindingLayer::User.name(), "user");
}

#[test]
fn test_binding_info_new() {
    let info = BindingInfo::new(cmd("down"), "Move down", Some("motion"), BindingLayer::Policy);
    assert_eq!(info.command, cmd("down"));
    assert_eq!(info.description, "Move down");
    assert_eq!(info.category, Some("motion"));
    assert_eq!(info.layer, BindingLayer::Policy);
}

#[test]
fn test_binding_info_from_command() {
    let info = BindingInfo::from_command(cmd("noop"), BindingLayer::Base);
    assert_eq!(info.command, cmd("noop"));
    assert_eq!(info.description, "");
    assert!(info.category.is_none());
    assert_eq!(info.layer, BindingLayer::Base);
}

#[test]
fn test_binding_info_equality() {
    let a = BindingInfo::from_command(cmd("x"), BindingLayer::User);
    let b = BindingInfo::from_command(cmd("x"), BindingLayer::User);
    assert_eq!(a, b);
}
