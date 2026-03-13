use reovim_driver_statusline::{ComponentDataContext, ComponentDataProvider};

use super::*;

#[test]
fn test_breadcrumb_id() {
    let comp = BreadcrumbComponent;
    assert_eq!(comp.id(), "breadcrumb");
}

#[test]
fn test_breadcrumb_data_no_breadcrumb() {
    let comp = BreadcrumbComponent;
    let ctx = ComponentDataContext::default();
    let data = comp.data(&ctx);
    assert!(!data.visible);
}

#[test]
fn test_breadcrumb_data_empty_breadcrumb() {
    let comp = BreadcrumbComponent;
    let ctx = ComponentDataContext {
        breadcrumb: Some(String::new()),
        ..ComponentDataContext::default()
    };
    let data = comp.data(&ctx);
    assert!(!data.visible);
}

#[test]
fn test_breadcrumb_data_with_text() {
    let comp = BreadcrumbComponent;
    let ctx = ComponentDataContext {
        breadcrumb: Some("fn main > impl Foo".to_string()),
        ..ComponentDataContext::default()
    };
    let data = comp.data(&ctx);
    assert!(data.visible);
    assert_eq!(data.text, " fn main > impl Foo ");
}
