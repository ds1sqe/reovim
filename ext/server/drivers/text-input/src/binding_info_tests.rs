//! Tests for `BindingInfo`.

use {super::BindingInfo, crate::BindingLayer, reovim_kernel::api::v1::ModuleId};

fn cmd_id(name: &'static str) -> reovim_kernel::api::v1::CommandId {
    reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), name)
}

#[test]
fn binding_info_constructors_preserve_fields() {
    let info = BindingInfo::new(
        cmd_id("cursor-down"),
        "Move cursor down",
        Some("motion"),
        BindingLayer::Policy,
    );
    assert_eq!(info.command, cmd_id("cursor-down"));
    assert_eq!(info.description, "Move cursor down");
    assert_eq!(info.category, Some("motion"));
    assert_eq!(info.layer, BindingLayer::Policy);

    let bare = BindingInfo::from_command(cmd_id("delete"), BindingLayer::User);
    assert_eq!(bare.command, cmd_id("delete"));
    assert_eq!(bare.description, "");
    assert_eq!(bare.category, None);
    assert_eq!(bare.layer, BindingLayer::User);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn binding_info_debug_mentions_type_name() {
    let debug = format!(
        "{:?}",
        BindingInfo::new(
            cmd_id("delete"),
            "Delete",
            Some("operator"),
            BindingLayer::Base
        )
    );
    assert!(debug.contains("BindingInfo"));
}
