use std::{path::Path, sync::Arc};

use {
    reovim_kernel::api::v1::{
        BufferId, OptionRegistry, OptionScopeId, OptionSpec, OptionValue, RwLock, ServiceRegistry,
        events::kernel::BufferWillSave,
    },
    reovim_provider_text::{Buffer, TextBufferRegistry},
    reovim_driver_formatter::{FormatError, FormatterProvider, FormatterRegistry},
};

use super::*;

struct UpperFormatter;

impl FormatterProvider for UpperFormatter {
    fn format(&self, content: &str, _path: &Path) -> Result<String, FormatError> {
        Ok(content.to_uppercase())
    }

    #[allow(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        "upper"
    }
}

/// Create a `TextBufferRegistry` with a buffer containing the given content.
/// Returns the registry, the `BufferId`, and the u64 for the event.
fn setup_buffer(content: &str) -> (Arc<TextBufferRegistry>, BufferId, u64) {
    let registry = Arc::new(TextBufferRegistry::new());
    let mut buf = Buffer::new();
    buf.set_content(content);
    buf.set_file_path(Some("test.rs".to_string()));
    let id = registry.register(Arc::new(RwLock::new(buf)));
    // Buffer IDs in tests are small, safe to cast.
    #[allow(clippy::cast_possible_truncation)]
    let event_id = id.as_usize() as u64;
    (registry, id, event_id)
}

fn setup_options(autoformat: bool) -> Arc<OptionRegistry> {
    let options = Arc::new(OptionRegistry::new());
    options
        .register(OptionSpec::new(
            "autoformat",
            "Enable format-on-save",
            OptionValue::bool(autoformat),
        ))
        .unwrap();
    options
}

fn setup_services_with_formatter(text_buffers: Arc<TextBufferRegistry>) -> Arc<ServiceRegistry> {
    let services = Arc::new(ServiceRegistry::new());
    let mut registry = FormatterRegistry::new();
    registry.register("rust", Box::new(UpperFormatter));
    services.register(Arc::new(registry));
    services.register(text_buffers);
    services
}

/// Build a `ServiceRegistry` with only a `TextBufferRegistry` (no formatter).
fn setup_services_no_formatter(text_buffers: Arc<TextBufferRegistry>) -> Arc<ServiceRegistry> {
    let services = Arc::new(ServiceRegistry::new());
    services.register(text_buffers);
    services
}

#[test]
fn test_format_on_save_applies_formatter() {
    let (text_buffers, bid, eid) = setup_buffer("hello world");
    let options = setup_options(true);
    let services = setup_services_with_formatter(Arc::clone(&text_buffers));

    let event = BufferWillSave {
        buffer_id: eid,
        path: "test.rs".to_string(),
    };

    format_on_save(&event, &services, &options);

    let buf = text_buffers.get(bid).unwrap();
    assert_eq!(buf.read().content(), "HELLO WORLD");
}

#[test]
fn test_format_on_save_skipped_when_disabled() {
    let (text_buffers, bid, eid) = setup_buffer("hello world");
    let options = setup_options(true);
    options
        .set("autoformat", OptionValue::bool(false), OptionScopeId::Global)
        .unwrap();
    let services = setup_services_with_formatter(Arc::clone(&text_buffers));

    let event = BufferWillSave {
        buffer_id: eid,
        path: "test.rs".to_string(),
    };

    format_on_save(&event, &services, &options);

    let buf = text_buffers.get(bid).unwrap();
    assert_eq!(buf.read().content(), "hello world");
}

#[test]
fn test_format_on_save_no_formatter_available() {
    let (text_buffers, bid, eid) = setup_buffer("hello world");
    let options = setup_options(true);
    let services = setup_services_no_formatter(Arc::clone(&text_buffers));

    let event = BufferWillSave {
        buffer_id: eid,
        path: "test.rs".to_string(),
    };

    format_on_save(&event, &services, &options);

    let buf = text_buffers.get(bid).unwrap();
    assert_eq!(buf.read().content(), "hello world");
}

#[test]
fn test_format_on_save_buffer_not_found() {
    let text_buffers = Arc::new(TextBufferRegistry::new());
    let options = setup_options(true);
    let services = setup_services_with_formatter(text_buffers);

    let event = BufferWillSave {
        buffer_id: 999,
        path: "test.rs".to_string(),
    };

    format_on_save(&event, &services, &options);
}

#[test]
fn test_format_on_save_empty_content() {
    let (text_buffers, _bid, eid) = setup_buffer("");
    let options = setup_options(true);
    let services = setup_services_with_formatter(text_buffers);

    let event = BufferWillSave {
        buffer_id: eid,
        path: "test.rs".to_string(),
    };

    format_on_save(&event, &services, &options);
}

#[test]
fn test_format_on_save_unknown_filetype() {
    let (text_buffers, bid, eid) = setup_buffer("hello");
    let options = setup_options(true);
    let services = setup_services_with_formatter(Arc::clone(&text_buffers));

    let event = BufferWillSave {
        buffer_id: eid,
        path: "Makefile".to_string(),
    };

    format_on_save(&event, &services, &options);

    let buf = text_buffers.get(bid).unwrap();
    assert_eq!(buf.read().content(), "hello");
}

#[test]
fn test_format_on_save_no_change_needed() {
    let (text_buffers, bid, eid) = setup_buffer("ALREADY UPPERCASE");
    let options = setup_options(true);
    let services = setup_services_with_formatter(Arc::clone(&text_buffers));

    let event = BufferWillSave {
        buffer_id: eid,
        path: "test.rs".to_string(),
    };

    format_on_save(&event, &services, &options);

    let buf = text_buffers.get(bid).unwrap();
    assert_eq!(buf.read().content(), "ALREADY UPPERCASE");
}

#[test]
fn test_format_on_save_no_text_buffer_registry() {
    // MC/DC: services.get::<TextBufferRegistry>() returns None
    let options = setup_options(true);
    let services = Arc::new(ServiceRegistry::new());

    let event = BufferWillSave {
        buffer_id: 1,
        path: "test.rs".to_string(),
    };

    // Should not panic — logs warning and returns early.
    format_on_save(&event, &services, &options);
}

#[test]
fn test_format_on_save_default_autoformat_enabled() {
    let (text_buffers, bid, eid) = setup_buffer("hello");
    let options = setup_options(true);
    let services = setup_services_with_formatter(Arc::clone(&text_buffers));

    let event = BufferWillSave {
        buffer_id: eid,
        path: "test.rs".to_string(),
    };

    format_on_save(&event, &services, &options);

    let buf = text_buffers.get(bid).unwrap();
    assert_eq!(buf.read().content(), "HELLO");
}
