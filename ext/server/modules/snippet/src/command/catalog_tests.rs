use crate::provider::SnippetRegistry;

use super::*;

#[test]
fn test_catalog_command_id() {
    let cmd = SnippetCatalog::new(SnippetRegistryHandle::new(SnippetRegistry::new()));
    assert_eq!(cmd.id(), ids::CATALOG);
}

#[test]
fn test_catalog_description() {
    let cmd = SnippetCatalog::new(SnippetRegistryHandle::new(SnippetRegistry::new()));
    assert!(!cmd.description().is_empty());
}
