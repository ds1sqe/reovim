use reovim_kernel::api::v1::ModuleId;

use {super::*, crate::provider::SnippetRegistry};

fn test_return_mode() -> ModeId {
    ModeId::new(ModuleId::new("test"), "return")
}

#[test]
fn test_all_commands_count() {
    let handle = SnippetRegistryHandle::new(SnippetRegistry::new());
    let commands = all_commands(handle, test_return_mode(), PathBuf::from("/tmp"));
    assert_eq!(commands.len(), 6);
}

#[test]
fn test_all_commands_unique_ids() {
    let handle = SnippetRegistryHandle::new(SnippetRegistry::new());
    let commands = all_commands(handle, test_return_mode(), PathBuf::from("/tmp"));
    let ids: Vec<_> = commands.iter().map(|c| c.id()).collect();
    for (i, a) in ids.iter().enumerate() {
        for (j, b) in ids.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "command ids {i} and {j} should be unique");
            }
        }
    }
}
