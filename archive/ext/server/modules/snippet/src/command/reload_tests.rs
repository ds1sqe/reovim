use crate::provider::SnippetRegistry;

use super::*;

#[test]
fn test_reload_command_id() {
    let handle = SnippetRegistryHandle::new(SnippetRegistry::new());
    let cmd = ReloadSnippets::new(handle, PathBuf::from("/tmp"));
    assert_eq!(cmd.id(), ids::RELOAD);
}

#[test]
fn test_reload_description() {
    let handle = SnippetRegistryHandle::new(SnippetRegistry::new());
    let cmd = ReloadSnippets::new(handle, PathBuf::from("/tmp"));
    assert!(!cmd.description().is_empty());
}
