//! Integration tests for ResolverRegistry with Vim resolvers.
//!
//! These tests verify that the Vim resolvers integrate correctly
//! with the editor's ResolverRegistry mechanism.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use {reovim_driver_input::ModeKeyResolver, reovim_module_editor::ResolverRegistry};

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
}
