
use super::*;


// Test extension type
#[derive(Debug, Default, PartialEq)]
struct TestExtension {
    value: i32,
}

impl SessionExtension for TestExtension {
    fn create() -> Self {
        Self { value: 42 }
    }
}

// Another test extension
#[derive(Debug, Default)]
struct AnotherExtension {
    name: String,
}

impl SessionExtension for AnotherExtension {
    fn create() -> Self {
        Self {
            name: "default".to_string(),
        }
    }
}

#[test]
fn test_extension_map_new() {
    let map = ExtensionMap::new();
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
}

#[test]
fn test_get_nonexistent() {
    let map = ExtensionMap::new();
    assert!(map.get::<TestExtension>().is_none());
}

#[test]
fn test_get_or_insert_creates() {
    let mut map = ExtensionMap::new();
    let ext = map.get_or_insert::<TestExtension>();
    assert_eq!(ext.value, 42); // Default from create()
}

#[test]
fn test_get_or_insert_returns_existing() {
    let mut map = ExtensionMap::new();

    // First access creates with default
    map.get_or_insert::<TestExtension>().value = 100;

    // Second access returns existing
    let ext = map.get_or_insert::<TestExtension>();
    assert_eq!(ext.value, 100);
}

#[test]
fn test_get_after_insert() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<TestExtension>().value = 99;

    let ext = map.get::<TestExtension>();
    assert!(ext.is_some());
    assert_eq!(ext.unwrap().value, 99);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_get_mut() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<TestExtension>();

    if let Some(ext) = map.get_mut::<TestExtension>() {
        ext.value = 200;
    }

    assert_eq!(map.get::<TestExtension>().unwrap().value, 200);
}

#[test]
fn test_multiple_extensions() {
    let mut map = ExtensionMap::new();

    map.get_or_insert::<TestExtension>().value = 1;
    map.get_or_insert::<AnotherExtension>().name = "hello".to_string();

    assert_eq!(map.len(), 2);
    assert_eq!(map.get::<TestExtension>().unwrap().value, 1);
    assert_eq!(map.get::<AnotherExtension>().unwrap().name, "hello");
}

#[test]
fn test_contains() {
    let mut map = ExtensionMap::new();
    assert!(!map.contains::<TestExtension>());

    map.get_or_insert::<TestExtension>();
    assert!(map.contains::<TestExtension>());
}

#[test]
fn test_remove() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<TestExtension>();

    assert!(map.remove::<TestExtension>());
    assert!(!map.contains::<TestExtension>());
    assert!(!map.remove::<TestExtension>()); // Already removed
}

#[test]
fn test_clear() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<TestExtension>();
    map.get_or_insert::<AnotherExtension>();

    assert_eq!(map.len(), 2);
    map.clear();
    assert!(map.is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_debug_impl() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<TestExtension>();

    let debug = format!("{map:?}");
    assert!(debug.contains("ExtensionMap"));
    assert!(debug.contains("count"));
}

// ========================================================================
// TextInputSink tests (#482)
// ========================================================================

#[test]
fn test_get_text_input_sink_by_id_nonexistent() {
    // Test that get_text_input_sink_by_id returns None for non-existent extension
    let mut map = ExtensionMap::new();
    let fake_type_id = std::any::TypeId::of::<String>();

    let result = map.get_text_input_sink_by_id(fake_type_id);
    assert!(result.is_none());
}

#[test]
fn test_get_text_input_sink_by_id_no_sink_impl() {
    // Test that get_text_input_sink_by_id returns None for extension
    // that exists but doesn't implement TextInputSink
    let mut map = ExtensionMap::new();
    map.get_or_insert::<TestExtension>(); // TestExtension doesn't impl TextInputSink

    let type_id = std::any::TypeId::of::<TestExtension>();
    let result = map.get_text_input_sink_by_id(type_id);
    assert!(result.is_none()); // Should return None (default as_text_input_sink returns None)
}

// ========================================================================
// TextInputSink full flow tests (#482)
// ========================================================================

/// Extension that implements `TextInputSink`
#[derive(Debug, Default)]
struct SinkExtension {
    buffer: String,
}

impl SessionExtension for SinkExtension {
    fn create() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
        Some(self)
    }
}

impl TextInputSink for SinkExtension {
    fn insert_char(&mut self, ch: char) {
        self.buffer.push(ch);
    }
}

#[test]
fn test_text_input_sink_via_extension_map() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<SinkExtension>();

    let type_id = std::any::TypeId::of::<SinkExtension>();
    let sink = map.get_text_input_sink_by_id(type_id);
    assert!(sink.is_some());

    // Insert characters via sink
    let sink = sink.unwrap();
    sink.insert_char('h');
    sink.insert_char('i');

    // Verify via get
    let ext = map.get::<SinkExtension>().unwrap();
    assert_eq!(ext.buffer, "hi");
}

#[test]
fn test_extension_default_as_text_input_sink_returns_none() {
    let mut ext = TestExtension { value: 42 };
    assert!(SessionExtension::as_text_input_sink(&mut ext).is_none());
}

#[test]
fn test_extension_dyn_as_any() {
    let ext = TestExtension { value: 42 };
    let boxed: Box<dyn SessionExtensionDyn> = Box::new(ext);

    // Should be able to downcast
    let any = boxed.as_any();
    assert!(any.downcast_ref::<TestExtension>().is_some());
}

#[test]
fn test_extension_dyn_as_any_mut() {
    let ext = TestExtension { value: 42 };
    let mut boxed: Box<dyn SessionExtensionDyn> = Box::new(ext);

    let any = boxed.as_any_mut();
    let concrete = any.downcast_mut::<TestExtension>().unwrap();
    concrete.value = 100;
    assert_eq!(concrete.value, 100);
}

#[test]
fn test_extension_dyn_as_text_input_sink_none() {
    let ext = TestExtension { value: 42 };
    let mut boxed: Box<dyn SessionExtensionDyn> = Box::new(ext);

    assert!(boxed.as_text_input_sink().is_none());
}

#[test]
fn test_extension_dyn_as_text_input_sink_some() {
    let ext = SinkExtension::create();
    let mut boxed: Box<dyn SessionExtensionDyn> = Box::new(ext);

    let sink = boxed.as_text_input_sink();
    assert!(sink.is_some());
}

#[test]
fn test_extension_map_default() {
    let map = ExtensionMap::default();
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
}

#[test]
fn test_get_mut_nonexistent() {
    let mut map = ExtensionMap::new();
    assert!(map.get_mut::<TestExtension>().is_none());
}

#[test]
fn test_remove_nonexistent() {
    let mut map = ExtensionMap::new();
    assert!(!map.remove::<TestExtension>());
}

#[test]
fn test_extension_independence() {
    // Verify different extension types don't interfere
    let mut map = ExtensionMap::new();

    map.get_or_insert::<TestExtension>().value = 10;
    map.get_or_insert::<AnotherExtension>().name = "test".to_string();

    // Removing one doesn't affect the other
    assert!(map.remove::<TestExtension>());
    assert!(map.contains::<AnotherExtension>());
    assert_eq!(map.get::<AnotherExtension>().unwrap().name, "test");
    assert_eq!(map.len(), 1);
}

#[test]
fn test_clear_then_reinsert() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<TestExtension>().value = 99;
    map.clear();

    // After clear, get_or_insert should create fresh
    let ext = map.get_or_insert::<TestExtension>();
    assert_eq!(ext.value, 42); // Default from create()
}