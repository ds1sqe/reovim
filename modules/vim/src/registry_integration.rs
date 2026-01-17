//! Integration tests for ResolverRegistry with Vim resolvers.
//!
//! These tests verify that the Vim resolvers integrate correctly
//! with the editor's ResolverRegistry mechanism.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use {
        reovim_driver_input::ModeKeyResolver,
        reovim_module_editor::{EditorMode, ResolverRegistry},
    };

    use crate::{VimInsertResolver, VimNormalResolver, VimOperatorPendingResolver};

    #[test]
    fn test_register_vim_normal_resolver() {
        let mut registry = ResolverRegistry::new();

        registry.register(VimNormalResolver::new());

        assert_eq!(registry.len(), 1);
        assert!(registry.has(&EditorMode::NORMAL_ID));
    }

    #[test]
    fn test_register_all_vim_resolvers() {
        let mut registry = ResolverRegistry::new();

        registry.register(VimNormalResolver::new());
        registry.register(VimInsertResolver::new());
        registry.register(VimOperatorPendingResolver::new());

        assert_eq!(registry.len(), 3);
        assert!(registry.has(&EditorMode::NORMAL_ID));
        assert!(registry.has(&EditorMode::INSERT_ID));
        assert!(registry.has(&EditorMode::OPERATOR_PENDING_ID));
    }

    #[test]
    fn test_get_vim_resolver() {
        let mut registry = ResolverRegistry::new();
        registry.register(VimNormalResolver::new());

        let resolver = registry.get(&EditorMode::NORMAL_ID);
        assert!(resolver.is_some());
        assert_eq!(resolver.unwrap().mode_id(), &EditorMode::NORMAL_ID);
    }

    #[test]
    fn test_remove_vim_resolver() {
        let mut registry = ResolverRegistry::new();
        registry.register(VimNormalResolver::new());

        let removed = registry.remove(&EditorMode::NORMAL_ID);
        assert!(removed.is_some());
        assert!(!registry.has(&EditorMode::NORMAL_ID));
        assert!(registry.is_empty());
    }

    #[test]
    fn test_vim_resolvers_iterator() {
        let mut registry = ResolverRegistry::new();
        registry.register(VimNormalResolver::new());
        registry.register(VimInsertResolver::new());

        assert_eq!(registry.modes().count(), 2);
    }

    #[test]
    fn test_vim_resolver_replacement() {
        let mut registry = ResolverRegistry::new();

        registry.register(VimNormalResolver::new());
        assert_eq!(registry.len(), 1);

        // Register another normal mode resolver - should replace
        registry.register(VimNormalResolver::new());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_register_vim_resolver_arc() {
        let mut registry = ResolverRegistry::new();
        let resolver: Arc<dyn ModeKeyResolver> = Arc::new(VimNormalResolver::new());

        registry.register_arc(resolver);

        assert!(registry.has(&EditorMode::NORMAL_ID));
    }

    #[test]
    fn test_debug_with_vim_resolvers() {
        let mut registry = ResolverRegistry::new();
        registry.register(VimNormalResolver::new());

        let debug = format!("{registry:?}");
        assert!(debug.contains("ResolverRegistry"));
        assert!(debug.contains("count: 1"));
    }
}
