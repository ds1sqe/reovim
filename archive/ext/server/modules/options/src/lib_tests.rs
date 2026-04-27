use super::*;

// ========================================================================
// VirtualEditMode tests
// ========================================================================

#[test]
fn test_virtual_edit_mode_default() {
    let mode = VirtualEditMode::default();
    assert_eq!(mode, VirtualEditMode::None);
}

#[test]
fn test_virtual_edit_mode_all_variants() {
    // Ensure all variants are distinct
    let none = VirtualEditMode::None;
    let all = VirtualEditMode::All;
    let block = VirtualEditMode::Block;
    let insert = VirtualEditMode::Insert;
    let one_more = VirtualEditMode::OneMore;

    assert_ne!(none, all);
    assert_ne!(none, block);
    assert_ne!(none, insert);
    assert_ne!(none, one_more);
    assert_ne!(all, block);
    assert_ne!(all, insert);
    assert_ne!(all, one_more);
    assert_ne!(block, insert);
    assert_ne!(block, one_more);
    assert_ne!(insert, one_more);
}

#[test]
fn test_virtual_edit_mode_clone() {
    let mode = VirtualEditMode::Block;
    let cloned = mode;
    assert_eq!(mode, cloned);
}

#[test]
fn test_virtual_edit_mode_debug() {
    let mode = VirtualEditMode::Insert;
    let debug_str = format!("{mode:?}");
    assert_eq!(debug_str, "Insert");
}

#[test]
fn test_virtual_edit_mode_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(VirtualEditMode::Block);
    set.insert(VirtualEditMode::Insert);
    set.insert(VirtualEditMode::Block); // duplicate

    assert_eq!(set.len(), 2);
    assert!(set.contains(&VirtualEditMode::Block));
    assert!(set.contains(&VirtualEditMode::Insert));
}

#[test]
fn test_virtual_edit_mode_copy() {
    let mode = VirtualEditMode::All;
    let copied = mode;
    // Both still usable after copy (not moved)
    assert_eq!(mode, copied);
}

// ========================================================================
// VirtualEditConfig tests
// ========================================================================

#[test]
fn test_virtual_edit_config_new() {
    let config = VirtualEditConfig::new();
    assert!(config.is_disabled());
    assert!(!config.is_enabled());
}

#[test]
fn test_virtual_edit_config_default() {
    let config = VirtualEditConfig::default();
    assert!(config.is_disabled());
    assert!(!config.is_enabled());
    assert!(config.modes().is_empty());
}

#[test]
fn test_virtual_edit_config_all() {
    let config = VirtualEditConfig::all();
    assert!(config.is_enabled());
    assert!(!config.is_disabled());
    assert!(config.allows_mode(VirtualEditMode::Block));
    assert!(config.allows_mode(VirtualEditMode::Insert));
    assert!(config.allows_mode(VirtualEditMode::OneMore));
    assert!(config.allows_mode(VirtualEditMode::All));
    // "All" mode makes even None return true (since All is in the list)
    assert!(config.allows_mode(VirtualEditMode::None));
}

#[test]
fn test_virtual_edit_config_all_modes_list() {
    let config = VirtualEditConfig::all();
    assert_eq!(config.modes().len(), 1);
    assert_eq!(config.modes()[0], VirtualEditMode::All);
}

#[test]
fn test_virtual_edit_config_block_only() {
    let config = VirtualEditConfig::block_only();
    assert!(config.allows_mode(VirtualEditMode::Block));
    assert!(!config.allows_mode(VirtualEditMode::Insert));
    assert!(!config.allows_mode(VirtualEditMode::OneMore));
    assert!(!config.allows_mode(VirtualEditMode::All));
}

#[test]
fn test_virtual_edit_config_block_only_modes_list() {
    let config = VirtualEditConfig::block_only();
    assert_eq!(config.modes().len(), 1);
    assert_eq!(config.modes()[0], VirtualEditMode::Block);
}

#[test]
fn test_virtual_edit_config_add_mode() {
    let mut config = VirtualEditConfig::new();
    config.add_mode(VirtualEditMode::Insert);
    config.add_mode(VirtualEditMode::Block);

    assert!(config.allows_mode(VirtualEditMode::Insert));
    assert!(config.allows_mode(VirtualEditMode::Block));
    assert!(!config.allows_mode(VirtualEditMode::OneMore));
}

#[test]
fn test_virtual_edit_config_add_mode_none() {
    let mut config = VirtualEditConfig::new();
    config.add_mode(VirtualEditMode::None);
    // is_enabled returns true because modes is not empty
    assert!(config.is_enabled());
    // is_disabled returns true when modes == [None]
    assert!(config.is_disabled());
}

#[test]
fn test_virtual_edit_config_add_all_makes_everything_allowed() {
    let mut config = VirtualEditConfig::new();
    config.add_mode(VirtualEditMode::All);

    assert!(config.allows_mode(VirtualEditMode::Block));
    assert!(config.allows_mode(VirtualEditMode::Insert));
    assert!(config.allows_mode(VirtualEditMode::OneMore));
    assert!(config.allows_mode(VirtualEditMode::None));
}

#[test]
fn test_virtual_edit_config_remove_mode() {
    let mut config = VirtualEditConfig::new();
    config.add_mode(VirtualEditMode::Insert);
    config.add_mode(VirtualEditMode::Block);
    config.remove_mode(VirtualEditMode::Insert);

    assert!(!config.allows_mode(VirtualEditMode::Insert));
    assert!(config.allows_mode(VirtualEditMode::Block));
}

#[test]
fn test_virtual_edit_config_remove_nonexistent_mode() {
    let mut config = VirtualEditConfig::new();
    config.add_mode(VirtualEditMode::Block);
    // Removing a mode that's not there should be safe
    config.remove_mode(VirtualEditMode::Insert);
    assert_eq!(config.modes().len(), 1);
    assert!(config.allows_mode(VirtualEditMode::Block));
}

#[test]
fn test_virtual_edit_config_remove_from_empty() {
    let mut config = VirtualEditConfig::new();
    config.remove_mode(VirtualEditMode::Block);
    assert!(config.modes().is_empty());
}

#[test]
fn test_virtual_edit_config_remove_all_mode() {
    let mut config = VirtualEditConfig::all();
    config.remove_mode(VirtualEditMode::All);
    assert!(config.is_disabled());
    assert!(!config.allows_mode(VirtualEditMode::Block));
}

#[test]
fn test_virtual_edit_config_clear() {
    let mut config = VirtualEditConfig::all();
    config.clear();
    assert!(config.is_disabled());
    assert!(config.modes().is_empty());
}

#[test]
fn test_virtual_edit_config_clear_already_empty() {
    let mut config = VirtualEditConfig::new();
    config.clear();
    assert!(config.is_disabled());
    assert!(config.modes().is_empty());
}

#[test]
fn test_virtual_edit_config_no_duplicates() {
    let mut config = VirtualEditConfig::new();
    config.add_mode(VirtualEditMode::Block);
    config.add_mode(VirtualEditMode::Block);
    config.add_mode(VirtualEditMode::Block);

    assert_eq!(config.modes().len(), 1);
}

#[test]
fn test_virtual_edit_config_allows_mode_empty() {
    let config = VirtualEditConfig::new();
    assert!(!config.allows_mode(VirtualEditMode::Block));
    assert!(!config.allows_mode(VirtualEditMode::Insert));
    assert!(!config.allows_mode(VirtualEditMode::OneMore));
    assert!(!config.allows_mode(VirtualEditMode::All));
    assert!(!config.allows_mode(VirtualEditMode::None));
}

#[test]
fn test_virtual_edit_config_is_enabled_with_modes() {
    let mut config = VirtualEditConfig::new();
    assert!(!config.is_enabled());
    config.add_mode(VirtualEditMode::Block);
    assert!(config.is_enabled());
}

#[test]
fn test_virtual_edit_config_is_disabled_empty() {
    let config = VirtualEditConfig::new();
    assert!(config.is_disabled());
}

#[test]
fn test_virtual_edit_config_is_disabled_with_none() {
    let mut config = VirtualEditConfig::new();
    config.add_mode(VirtualEditMode::None);
    assert!(config.is_disabled());
}

#[test]
fn test_virtual_edit_config_is_disabled_with_real_mode() {
    let mut config = VirtualEditConfig::new();
    config.add_mode(VirtualEditMode::Block);
    assert!(!config.is_disabled());
}

#[test]
fn test_virtual_edit_config_is_disabled_with_none_and_other() {
    let mut config = VirtualEditConfig::new();
    config.add_mode(VirtualEditMode::None);
    config.add_mode(VirtualEditMode::Block);
    // modes is [None, Block] which is not empty and not == [None]
    assert!(!config.is_disabled());
}

#[test]
fn test_virtual_edit_config_modes_returns_all_added() {
    let mut config = VirtualEditConfig::new();
    config.add_mode(VirtualEditMode::Insert);
    config.add_mode(VirtualEditMode::Block);
    config.add_mode(VirtualEditMode::OneMore);

    let modes = config.modes();
    assert_eq!(modes.len(), 3);
    assert!(modes.contains(&VirtualEditMode::Insert));
    assert!(modes.contains(&VirtualEditMode::Block));
    assert!(modes.contains(&VirtualEditMode::OneMore));
}

#[test]
fn test_set_modes() {
    let mut config = VirtualEditConfig::new();
    config.set_modes(vec![VirtualEditMode::Insert, VirtualEditMode::OneMore]);

    assert!(config.allows_mode(VirtualEditMode::Insert));
    assert!(config.allows_mode(VirtualEditMode::OneMore));
    assert!(!config.allows_mode(VirtualEditMode::Block));
}

#[test]
fn test_set_modes_replaces_existing() {
    let mut config = VirtualEditConfig::new();
    config.add_mode(VirtualEditMode::Block);
    config.add_mode(VirtualEditMode::Insert);
    assert_eq!(config.modes().len(), 2);

    config.set_modes(vec![VirtualEditMode::OneMore]);
    assert_eq!(config.modes().len(), 1);
    assert!(config.allows_mode(VirtualEditMode::OneMore));
    assert!(!config.allows_mode(VirtualEditMode::Block));
    assert!(!config.allows_mode(VirtualEditMode::Insert));
}

#[test]
fn test_set_modes_empty() {
    let mut config = VirtualEditConfig::all();
    config.set_modes(vec![]);
    assert!(config.is_disabled());
    assert!(config.modes().is_empty());
}

#[test]
fn test_set_modes_with_all() {
    let mut config = VirtualEditConfig::new();
    config.set_modes(vec![VirtualEditMode::All]);
    assert!(config.allows_mode(VirtualEditMode::Block));
    assert!(config.allows_mode(VirtualEditMode::Insert));
}

#[test]
fn test_virtual_edit_config_clone() {
    let mut config = VirtualEditConfig::new();
    config.add_mode(VirtualEditMode::Block);
    config.add_mode(VirtualEditMode::Insert);

    let cloned = config.clone();
    assert_eq!(cloned.modes().len(), 2);
    assert!(cloned.allows_mode(VirtualEditMode::Block));
    assert!(cloned.allows_mode(VirtualEditMode::Insert));
}

#[test]
fn test_virtual_edit_config_debug() {
    let config = VirtualEditConfig::new();
    let debug_str = format!("{config:?}");
    assert!(debug_str.contains("VirtualEditConfig"));
}

// ========================================================================
// EditorSettings tests
// ========================================================================

#[test]
fn test_editor_settings_default() {
    let settings = EditorSettings::default();
    assert!(settings.virtual_edit.is_disabled());
}

#[test]
fn test_editor_settings_new() {
    let settings = EditorSettings::new();
    assert!(settings.virtual_edit.is_disabled());
}

#[test]
fn test_editor_settings_modify_virtual_edit() {
    let mut settings = EditorSettings::new();
    settings.virtual_edit.add_mode(VirtualEditMode::Block);
    assert!(settings.virtual_edit.allows_mode(VirtualEditMode::Block));
}

#[test]
fn test_editor_settings_clone() {
    let mut settings = EditorSettings::new();
    settings.virtual_edit.add_mode(VirtualEditMode::Insert);

    let cloned = settings.clone();
    assert!(cloned.virtual_edit.allows_mode(VirtualEditMode::Insert));
}

#[test]
fn test_editor_settings_debug() {
    let settings = EditorSettings::new();
    let debug_str = format!("{settings:?}");
    assert!(debug_str.contains("EditorSettings"));
}

// ========================================================================
// OptionsModule tests
// ========================================================================

#[test]
fn test_options_module_id() {
    let module = OptionsModule::new();
    assert_eq!(module.id().as_str(), "options");
}

#[test]
fn test_options_module_name() {
    let module = OptionsModule::new();
    assert_eq!(module.name(), "Editor Options");
}

#[test]
fn test_options_module_version() {
    let module = OptionsModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 9);
    assert_eq!(version.patch, 0);
}

#[test]
fn test_options_module_version_display() {
    let module = OptionsModule::new();
    assert_eq!(module.version().to_string(), "0.9.0");
}

#[test]
fn test_options_module_default() {
    let module = OptionsModule::default();
    assert_eq!(module.id().as_str(), "options");
    assert_eq!(module.name(), "Editor Options");
}

#[test]
fn test_options_module_settings_access() {
    let mut module = OptionsModule::new();

    // Modify settings
    module
        .settings_mut()
        .virtual_edit
        .add_mode(VirtualEditMode::Block);

    // Read settings
    assert!(
        module
            .settings()
            .virtual_edit
            .allows_mode(VirtualEditMode::Block)
    );
}

#[test]
fn test_options_module_settings_ref() {
    let module = OptionsModule::new();
    let settings = module.settings();
    assert!(settings.virtual_edit.is_disabled());
}

#[test]
fn test_options_module_settings_mut() {
    let mut module = OptionsModule::new();
    let settings = module.settings_mut();
    settings.virtual_edit.add_mode(VirtualEditMode::OneMore);
    assert!(
        module
            .settings()
            .virtual_edit
            .allows_mode(VirtualEditMode::OneMore)
    );
}

// ========== Module trait implementation ==========

#[test]
fn test_options_module_init() {
    let mut module = OptionsModule::new();
    let ctx = ModuleContext::default();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);
}

#[test]
fn test_options_module_exit() {
    let mut module = OptionsModule::new();
    let result = module.exit();
    assert!(result.is_ok());
}

#[test]
fn test_options_module_init_then_exit() {
    let mut module = OptionsModule::new();
    let ctx = ModuleContext::default();

    assert_eq!(module.init(&ctx), ProbeResult::Success);
    assert!(module.exit().is_ok());
}

// ========== Module trait defaults ==========

#[test]
fn test_options_module_api_version() {
    let module = OptionsModule::new();
    let api_v = module.api_version();
    assert_eq!(api_v.major, 0);
    assert_eq!(api_v.minor, 2);
    assert_eq!(api_v.patch, 0);
}

#[test]
fn test_options_module_dependencies_empty() {
    let module = OptionsModule::new();
    assert!(module.dependencies().is_empty());
}

#[test]
fn test_options_module_optional_dependencies_empty() {
    let module = OptionsModule::new();
    assert!(module.optional_dependencies().is_empty());
}

#[test]
fn test_options_module_commands_empty() {
    let module = OptionsModule::new();
    assert!(module.commands().is_empty());
}

#[test]
fn test_options_module_keybindings_empty() {
    let module = OptionsModule::new();
    assert!(module.keybindings().is_empty());
}

#[test]
fn test_options_module_event_handlers_empty() {
    let module = OptionsModule::new();
    assert!(module.event_handlers().is_empty());
}

#[test]
fn test_options_module_no_hot_reload() {
    let module = OptionsModule::new();
    assert!(!module.supports_hot_reload());
}

#[test]
fn test_options_module_save_state_none() {
    let module = OptionsModule::new();
    assert!(module.save_state().is_none());
}

#[test]
fn test_options_module_restore_state_error() {
    let mut module = OptionsModule::new();
    let result = module.restore_state(&[]);
    assert!(result.is_err());
}

// ========== Lifecycle hooks (default no-ops) ==========

#[test]
fn test_options_module_on_all_loaded() {
    let mut module = OptionsModule::new();
    let ctx = ModuleContext::default();
    // Should not panic
    module.on_all_loaded(&ctx);
}

#[test]
fn test_options_module_on_buffer_focus() {
    use reovim_kernel::api::v1::BufferId;
    let mut module = OptionsModule::new();
    let ctx = ModuleContext::default();
    let buf_id = BufferId::from_raw(42);
    // Should not panic
    module.on_buffer_focus(buf_id, &ctx);
}

#[test]
fn test_options_module_on_unload() {
    let mut module = OptionsModule::new();
    let result = module.on_unload();
    assert!(result.is_ok());
}

// ========== Thread safety ==========

#[test]
fn test_options_module_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<OptionsModule>();
}

// ========== Full lifecycle ==========

#[test]
fn test_options_module_full_lifecycle() {
    use reovim_kernel::api::v1::BufferId;

    let mut module = OptionsModule::new();
    let ctx = ModuleContext::default();

    // Init
    assert_eq!(module.init(&ctx), ProbeResult::Success);

    // on_all_loaded
    module.on_all_loaded(&ctx);

    // Modify settings
    module
        .settings_mut()
        .virtual_edit
        .add_mode(VirtualEditMode::Block);
    assert!(
        module
            .settings()
            .virtual_edit
            .allows_mode(VirtualEditMode::Block)
    );

    // on_buffer_focus
    module.on_buffer_focus(BufferId::from_raw(1), &ctx);

    // Exit
    assert!(module.exit().is_ok());

    // on_unload
    assert!(module.on_unload().is_ok());
}

// ========== Module identity consistency ==========

#[test]
fn test_options_module_id_is_static() {
    let module = OptionsModule::new();
    let id = module.id();
    assert!(id.is_static());
}

#[test]
fn test_options_module_id_consistent() {
    let module = OptionsModule::new();
    let id1 = module.id();
    let id2 = module.id();
    assert_eq!(id1, id2);
}

// ========== Complex VirtualEditConfig scenarios ==========

#[test]
fn test_add_remove_add_cycle() {
    let mut config = VirtualEditConfig::new();
    config.add_mode(VirtualEditMode::Block);
    assert!(config.allows_mode(VirtualEditMode::Block));

    config.remove_mode(VirtualEditMode::Block);
    assert!(!config.allows_mode(VirtualEditMode::Block));

    config.add_mode(VirtualEditMode::Block);
    assert!(config.allows_mode(VirtualEditMode::Block));
    assert_eq!(config.modes().len(), 1);
}

#[test]
fn test_set_modes_then_add() {
    let mut config = VirtualEditConfig::new();
    config.set_modes(vec![VirtualEditMode::Block]);
    config.add_mode(VirtualEditMode::Insert);
    assert_eq!(config.modes().len(), 2);
    assert!(config.allows_mode(VirtualEditMode::Block));
    assert!(config.allows_mode(VirtualEditMode::Insert));
}

#[test]
fn test_clear_then_add() {
    let mut config = VirtualEditConfig::all();
    config.clear();
    assert!(config.is_disabled());
    config.add_mode(VirtualEditMode::OneMore);
    assert!(config.is_enabled());
    assert!(config.allows_mode(VirtualEditMode::OneMore));
    assert!(!config.allows_mode(VirtualEditMode::Block));
}

#[test]
fn test_remove_only_mode_makes_disabled() {
    let mut config = VirtualEditConfig::block_only();
    assert!(!config.is_disabled());
    config.remove_mode(VirtualEditMode::Block);
    assert!(config.is_disabled());
}

#[test]
fn test_multiple_modes_allows_each_individually() {
    let mut config = VirtualEditConfig::new();
    config.add_mode(VirtualEditMode::Block);
    config.add_mode(VirtualEditMode::Insert);
    config.add_mode(VirtualEditMode::OneMore);

    assert!(config.allows_mode(VirtualEditMode::Block));
    assert!(config.allows_mode(VirtualEditMode::Insert));
    assert!(config.allows_mode(VirtualEditMode::OneMore));
    assert!(!config.allows_mode(VirtualEditMode::All));
    assert!(!config.allows_mode(VirtualEditMode::None));
}
