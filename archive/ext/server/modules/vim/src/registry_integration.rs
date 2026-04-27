//! Integration tests for `ResolverRegistry` with Vim resolvers.
//!
//! These tests verify that the Vim resolvers integrate correctly
//! with the editor's `ResolverRegistry` mechanism.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use {reovim_driver_text_input::ModeKeyResolver, reovim_module_editor::ResolverRegistry};

    use crate::{
        VimChangeResolver, VimDeleteResolver, VimInsertResolver, VimMode, VimNormalResolver,
        VimYankResolver,
    };

    #[test]
    fn test_register_vim_normal_resolver() {
        let registry = ResolverRegistry::new();

        registry.register(VimNormalResolver::new());

        assert_eq!(registry.len(), 1);
        assert!(registry.has(&VimMode::NORMAL_ID));
    }

    #[test]
    fn test_register_all_vim_resolvers() {
        let registry = ResolverRegistry::new();

        registry.register(VimNormalResolver::new());
        registry.register(VimInsertResolver::new());
        registry.register(VimDeleteResolver::new());
        registry.register(VimYankResolver::new());
        registry.register(VimChangeResolver::new());

        assert_eq!(registry.len(), 5);
        assert!(registry.has(&VimMode::NORMAL_ID));
        assert!(registry.has(&VimMode::INSERT_ID));
        assert!(registry.has(&VimMode::DELETE_ID));
        assert!(registry.has(&VimMode::YANK_ID));
        assert!(registry.has(&VimMode::CHANGE_ID));
    }

    #[test]
    fn test_get_vim_resolver() {
        let registry = ResolverRegistry::new();
        registry.register(VimNormalResolver::new());

        let resolver = registry.get(&VimMode::NORMAL_ID);
        assert!(resolver.is_some());
        assert_eq!(resolver.unwrap().mode_id(), &VimMode::NORMAL_ID);
    }

    #[test]
    fn test_remove_vim_resolver() {
        let registry = ResolverRegistry::new();
        registry.register(VimNormalResolver::new());

        let removed = registry.remove(&VimMode::NORMAL_ID);
        assert!(removed.is_some());
        assert!(!registry.has(&VimMode::NORMAL_ID));
        assert!(registry.is_empty());
    }

    #[test]
    fn test_vim_resolvers_iterator() {
        let registry = ResolverRegistry::new();
        registry.register(VimNormalResolver::new());
        registry.register(VimInsertResolver::new());

        assert_eq!(registry.modes().len(), 2);
    }

    #[test]
    fn test_vim_resolver_replacement() {
        let registry = ResolverRegistry::new();

        registry.register(VimNormalResolver::new());
        assert_eq!(registry.len(), 1);

        // Register another normal mode resolver - should replace
        registry.register(VimNormalResolver::new());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_register_vim_resolver_arc() {
        let registry = ResolverRegistry::new();
        let resolver: Arc<dyn ModeKeyResolver> = Arc::new(VimNormalResolver::new());

        registry.register_arc(resolver);

        assert!(registry.has(&VimMode::NORMAL_ID));
    }

    #[test]
    fn test_debug_with_vim_resolvers() {
        let registry = ResolverRegistry::new();
        registry.register(VimNormalResolver::new());

        let debug = format!("{registry:?}");
        assert!(debug.contains("ResolverRegistry"));
        assert!(debug.contains("count: 1"));
    }

    // ========================================================================
    // Visual resolver registration tests
    // ========================================================================

    #[test]
    fn test_register_visual_character_resolver() {
        use crate::resolvers::VimVisualResolver;
        let registry = ResolverRegistry::new();
        registry.register(VimVisualResolver::character_wise());

        assert_eq!(registry.len(), 1);
        assert!(registry.has(&VimMode::VISUAL_ID));
    }

    #[test]
    fn test_register_visual_line_resolver() {
        use crate::resolvers::VimVisualResolver;
        let registry = ResolverRegistry::new();
        registry.register(VimVisualResolver::line_wise());

        assert_eq!(registry.len(), 1);
        assert!(registry.has(&VimMode::VISUAL_LINE_ID));
    }

    #[test]
    fn test_register_visual_block_resolver() {
        use crate::resolvers::VimVisualResolver;
        let registry = ResolverRegistry::new();
        registry.register(VimVisualResolver::block_wise());

        assert_eq!(registry.len(), 1);
        assert!(registry.has(&VimMode::VISUAL_BLOCK_ID));
    }

    #[test]
    fn test_register_all_visual_resolvers() {
        use crate::resolvers::VimVisualResolver;
        let registry = ResolverRegistry::new();
        registry.register(VimVisualResolver::character_wise());
        registry.register(VimVisualResolver::line_wise());
        registry.register(VimVisualResolver::block_wise());

        assert_eq!(registry.len(), 3);
        assert!(registry.has(&VimMode::VISUAL_ID));
        assert!(registry.has(&VimMode::VISUAL_LINE_ID));
        assert!(registry.has(&VimMode::VISUAL_BLOCK_ID));
    }

    // ========================================================================
    // Commandline resolver registration tests
    // ========================================================================

    #[test]
    fn test_register_commandline_resolver() {
        use crate::resolvers::VimCommandLineResolver;
        let registry = ResolverRegistry::new();
        registry.register(VimCommandLineResolver::new());

        assert_eq!(registry.len(), 1);
        assert!(registry.has(&VimMode::COMMANDLINE_ID));
    }

    #[test]
    fn test_get_commandline_resolver() {
        use crate::resolvers::VimCommandLineResolver;
        let registry = ResolverRegistry::new();
        registry.register(VimCommandLineResolver::new());

        let resolver = registry.get(&VimMode::COMMANDLINE_ID);
        assert!(resolver.is_some());
        assert_eq!(resolver.unwrap().mode_id(), &VimMode::COMMANDLINE_ID);
    }

    // ========================================================================
    // Window resolver registration tests
    // ========================================================================

    #[test]
    fn test_register_window_resolver() {
        use crate::resolvers::VimWindowResolver;
        let registry = ResolverRegistry::new();
        registry.register(VimWindowResolver::new());

        assert_eq!(registry.len(), 1);
        assert!(registry.has(&VimMode::WINDOW_ID));
    }

    #[test]
    fn test_get_window_resolver() {
        use crate::resolvers::VimWindowResolver;
        let registry = ResolverRegistry::new();
        registry.register(VimWindowResolver::new());

        let resolver = registry.get(&VimMode::WINDOW_ID);
        assert!(resolver.is_some());
        assert_eq!(resolver.unwrap().mode_id(), &VimMode::WINDOW_ID);
    }

    // ========================================================================
    // Full resolver registration tests (all 10)
    // ========================================================================

    #[test]
    fn test_register_all_ten_vim_resolvers() {
        use crate::resolvers::{VimCommandLineResolver, VimVisualResolver, VimWindowResolver};
        let registry = ResolverRegistry::new();

        registry.register(VimNormalResolver::new());
        registry.register(VimInsertResolver::new());
        registry.register(VimDeleteResolver::new());
        registry.register(VimYankResolver::new());
        registry.register(VimChangeResolver::new());
        registry.register(VimCommandLineResolver::new());
        registry.register(VimWindowResolver::new());
        registry.register(VimVisualResolver::character_wise());
        registry.register(VimVisualResolver::line_wise());
        registry.register(VimVisualResolver::block_wise());

        assert_eq!(registry.len(), 10);
    }

    #[test]
    fn test_all_ten_resolvers_have_correct_modes() {
        use crate::resolvers::{VimCommandLineResolver, VimVisualResolver, VimWindowResolver};
        let registry = ResolverRegistry::new();

        registry.register(VimNormalResolver::new());
        registry.register(VimInsertResolver::new());
        registry.register(VimDeleteResolver::new());
        registry.register(VimYankResolver::new());
        registry.register(VimChangeResolver::new());
        registry.register(VimCommandLineResolver::new());
        registry.register(VimWindowResolver::new());
        registry.register(VimVisualResolver::character_wise());
        registry.register(VimVisualResolver::line_wise());
        registry.register(VimVisualResolver::block_wise());

        assert!(registry.has(&VimMode::NORMAL_ID));
        assert!(registry.has(&VimMode::INSERT_ID));
        assert!(registry.has(&VimMode::DELETE_ID));
        assert!(registry.has(&VimMode::YANK_ID));
        assert!(registry.has(&VimMode::CHANGE_ID));
        assert!(registry.has(&VimMode::COMMANDLINE_ID));
        assert!(registry.has(&VimMode::WINDOW_ID));
        assert!(registry.has(&VimMode::VISUAL_ID));
        assert!(registry.has(&VimMode::VISUAL_LINE_ID));
        assert!(registry.has(&VimMode::VISUAL_BLOCK_ID));
    }

    #[test]
    fn test_modes_list_contains_all() {
        use crate::resolvers::{VimCommandLineResolver, VimVisualResolver, VimWindowResolver};
        let registry = ResolverRegistry::new();

        registry.register(VimNormalResolver::new());
        registry.register(VimInsertResolver::new());
        registry.register(VimDeleteResolver::new());
        registry.register(VimYankResolver::new());
        registry.register(VimChangeResolver::new());
        registry.register(VimCommandLineResolver::new());
        registry.register(VimWindowResolver::new());
        registry.register(VimVisualResolver::character_wise());
        registry.register(VimVisualResolver::line_wise());
        registry.register(VimVisualResolver::block_wise());

        assert_eq!(registry.modes().len(), 10);
    }

    #[test]
    fn test_remove_visual_resolver() {
        use crate::resolvers::VimVisualResolver;
        let registry = ResolverRegistry::new();
        registry.register(VimVisualResolver::character_wise());

        let removed = registry.remove(&VimMode::VISUAL_ID);
        assert!(removed.is_some());
        assert!(!registry.has(&VimMode::VISUAL_ID));
        assert!(registry.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_resolver() {
        let registry = ResolverRegistry::new();
        let removed = registry.remove(&VimMode::NORMAL_ID);
        assert!(removed.is_none());
    }

    #[test]
    fn test_visual_resolver_replacement() {
        use crate::resolvers::VimVisualResolver;
        let registry = ResolverRegistry::new();

        registry.register(VimVisualResolver::character_wise());
        assert_eq!(registry.len(), 1);

        // Replace with another character-wise resolver
        registry.register(VimVisualResolver::character_wise());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_registry_empty_initially() {
        let registry = ResolverRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_debug_with_all_resolvers() {
        use crate::resolvers::{VimCommandLineResolver, VimVisualResolver, VimWindowResolver};
        let registry = ResolverRegistry::new();

        registry.register(VimNormalResolver::new());
        registry.register(VimInsertResolver::new());
        registry.register(VimCommandLineResolver::new());
        registry.register(VimWindowResolver::new());
        registry.register(VimVisualResolver::character_wise());

        let debug = format!("{registry:?}");
        assert!(debug.contains("ResolverRegistry"));
        assert!(debug.contains("count: 5"));
    }

    #[test]
    fn test_register_arc_visual_resolver() {
        use crate::resolvers::VimVisualResolver;
        let registry = ResolverRegistry::new();
        let resolver: Arc<dyn ModeKeyResolver> = Arc::new(VimVisualResolver::character_wise());

        registry.register_arc(resolver);

        assert!(registry.has(&VimMode::VISUAL_ID));
    }

    #[test]
    fn test_register_arc_commandline_resolver() {
        use crate::resolvers::VimCommandLineResolver;
        let registry = ResolverRegistry::new();
        let resolver: Arc<dyn ModeKeyResolver> = Arc::new(VimCommandLineResolver::new());

        registry.register_arc(resolver);

        assert!(registry.has(&VimMode::COMMANDLINE_ID));
    }

    #[test]
    fn test_register_arc_window_resolver() {
        use crate::resolvers::VimWindowResolver;
        let registry = ResolverRegistry::new();
        let resolver: Arc<dyn ModeKeyResolver> = Arc::new(VimWindowResolver::new());

        registry.register_arc(resolver);

        assert!(registry.has(&VimMode::WINDOW_ID));
    }
}
