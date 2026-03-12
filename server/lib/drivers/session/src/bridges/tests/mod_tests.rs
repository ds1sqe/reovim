use super::*;

// ========================================================================
// ExtensionScope tests
// ========================================================================

#[test]
fn test_extension_scope_debug() {
    let scope = ExtensionScope::Shared;
    let debug = format!("{scope:?}");
    assert!(debug.contains("Shared"));
}

#[test]
fn test_extension_scope_clone_copy_eq() {
    let a = ExtensionScope::Client;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(a, ExtensionScope::Shared);
}

// ========================================================================
// BridgeRegistry tests
// ========================================================================

/// Minimal bridge for testing.
struct DummyBridge {
    kind_str: &'static str,
}

impl DummyBridge {
    const fn new(kind: &'static str) -> Self {
        Self { kind_str: kind }
    }
}

impl ExtensionStateBridge for DummyBridge {
    fn kind(&self) -> &'static str {
        self.kind_str
    }
    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Shared
    }
    fn snapshot(&self, _extensions: &ExtensionMap) -> Option<serde_json::Value> {
        Some(serde_json::json!({"dummy": true}))
    }
    fn is_active(&self, _extensions: &ExtensionMap) -> bool {
        false
    }
}

#[test]
fn test_registry_new_is_empty() {
    let reg = BridgeRegistry::new();
    assert!(reg.kinds().is_empty());
}

#[test]
fn test_registry_default_is_empty() {
    let reg = BridgeRegistry::default();
    assert!(reg.kinds().is_empty());
}

#[test]
fn test_registry_register_and_get() {
    let mut reg = BridgeRegistry::new();
    reg.register(DummyBridge::new("test"));
    assert!(reg.get("test").is_some());
    assert_eq!(reg.get("test").unwrap().kind(), "test");
}

#[test]
fn test_registry_get_unknown_returns_none() {
    let reg = BridgeRegistry::new();
    assert!(reg.get("nonexistent").is_none());
}

#[test]
fn test_registry_kinds() {
    let mut reg = BridgeRegistry::new();
    reg.register(DummyBridge::new("beta"));
    reg.register(DummyBridge::new("alpha"));
    // Sorted alphabetically
    assert_eq!(reg.kinds(), vec!["alpha", "beta"]);
}

#[test]
fn test_registry_duplicate_overwrites() {
    let mut reg = BridgeRegistry::new();
    reg.register(DummyBridge::new("test"));
    reg.register(DummyBridge::new("test"));
    assert_eq!(reg.kinds().len(), 1);
}

#[test]
fn test_registry_debug() {
    let mut reg = BridgeRegistry::new();
    reg.register(DummyBridge::new("cmdline"));
    let debug = format!("{reg:?}");
    assert!(debug.contains("BridgeRegistry"));
    assert!(debug.contains("cmdline"));
}

// ========================================================================
// Trait object construction test
// ========================================================================

#[test]
fn test_trait_object_construction() {
    let bridge: Box<dyn ExtensionStateBridge> = Box::new(DummyBridge::new("test"));
    assert_eq!(bridge.kind(), "test");
    assert_eq!(bridge.scope(), ExtensionScope::Shared);
    assert!(!bridge.is_active(&ExtensionMap::new()));
}

#[test]
fn test_dummy_bridge_snapshot() {
    let bridge = DummyBridge::new("test");
    let map = ExtensionMap::new();
    let snap = bridge.snapshot(&map);
    assert!(snap.is_some());
    assert_eq!(snap.unwrap(), serde_json::json!({"dummy": true}));
}

#[test]
fn test_on_mode_changed_default_noop() {
    let bridge = DummyBridge::new("test");
    let mut map = ExtensionMap::new();
    // Default impl is a no-op — should not panic.
    bridge.on_mode_changed("vim:insert", "vim:normal", &mut map);
}

#[test]
fn test_tick_default_returns_false() {
    let bridge = DummyBridge::new("test");
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();
    assert!(!bridge.tick(&mut client, &mut shared, &services));
}

#[test]
fn test_registry_values() {
    let mut reg = BridgeRegistry::new();
    reg.register(DummyBridge::new("alpha"));
    reg.register(DummyBridge::new("beta"));
    let mut kinds: Vec<&str> = reg.values().map(ExtensionStateBridge::kind).collect();
    kinds.sort_unstable();
    assert_eq!(kinds, vec!["alpha", "beta"]);
}

// ========================================================================
// register_boxed tests
// ========================================================================

#[test]
fn test_registry_register_boxed() {
    let mut reg = BridgeRegistry::new();
    let boxed: Box<dyn ExtensionStateBridge> = Box::new(DummyBridge::new("boxed"));
    reg.register_boxed(boxed);
    assert!(reg.get("boxed").is_some());
    assert_eq!(reg.get("boxed").unwrap().kind(), "boxed");
}

// ========================================================================
// BridgeProvider tests
// ========================================================================

#[test]
fn test_bridge_provider_default_empty() {
    let provider = BridgeProvider::default();
    let bridges = provider.take_bridges();
    assert!(bridges.is_empty());
}

#[test]
fn test_bridge_provider_register_and_take() {
    let provider = BridgeProvider::default();
    provider.register(DummyBridge::new("alpha"));
    provider.register(DummyBridge::new("beta"));

    let bridges = provider.take_bridges();
    assert_eq!(bridges.len(), 2);
    assert_eq!(bridges[0].kind(), "alpha");
    assert_eq!(bridges[1].kind(), "beta");

    // After take, provider is empty
    let bridges2 = provider.take_bridges();
    assert!(bridges2.is_empty());
}

#[test]
fn test_bridge_provider_service_trait() {
    use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

    let registry = ServiceRegistry::new();
    let provider = registry.get_or_create::<BridgeProvider>();
    provider.register(DummyBridge::new("test"));

    // Retrieve same instance
    let provider2 = registry.get_or_create::<BridgeProvider>();
    assert_eq!(Arc::as_ptr(&provider), Arc::as_ptr(&provider2));

    let bridges = provider2.take_bridges();
    assert_eq!(bridges.len(), 1);
}

#[test]
fn test_bridge_provider_debug() {
    let provider = BridgeProvider::default();
    provider.register(DummyBridge::new("test"));
    let debug = format!("{provider:?}");
    assert!(debug.contains("BridgeProvider"));
    assert!(debug.contains("pending_bridges"));
}

// ========================================================================
// BridgeContext tests (#543)
// ========================================================================

#[test]
fn test_bridge_context_new() {
    let shared = ExtensionMap::new();
    let opp1 = ExtensionMap::new();
    let opponents = vec![(ClientId::new(2), &opp1)];
    let ctx = BridgeContext::new(ClientId::new(1), &shared, &opponents);

    assert_eq!(ctx.client_id, ClientId::new(1));
    assert_eq!(ctx.opponent_count(), 1);
}

#[test]
fn test_bridge_context_for_each_opponent_iterates_all() {
    let shared = ExtensionMap::new();
    let opp1 = ExtensionMap::new();
    let opp2 = ExtensionMap::new();
    let opponents = vec![(ClientId::new(2), &opp1), (ClientId::new(3), &opp2)];
    let ctx = BridgeContext::new(ClientId::new(1), &shared, &opponents);

    let mut ids = Vec::new();
    ctx.for_each_opponent(|id, _ext| ids.push(id));
    assert_eq!(ids, vec![ClientId::new(2), ClientId::new(3)]);
}

#[test]
fn test_bridge_context_for_each_opponent_empty() {
    let shared = ExtensionMap::new();
    let opponents: Vec<(ClientId, &ExtensionMap)> = vec![];
    let ctx = BridgeContext::new(ClientId::new(1), &shared, &opponents);

    let mut count = 0;
    ctx.for_each_opponent(|_, _| count += 1);
    assert_eq!(count, 0);
}

#[test]
fn test_bridge_context_opponent_count() {
    let shared = ExtensionMap::new();
    let opp1 = ExtensionMap::new();
    let opponents = vec![(ClientId::new(5), &opp1)];
    let ctx = BridgeContext::new(ClientId::new(1), &shared, &opponents);
    assert_eq!(ctx.opponent_count(), 1);

    let empty: Vec<(ClientId, &ExtensionMap)> = vec![];
    let ctx2 = BridgeContext::new(ClientId::new(1), &shared, &empty);
    assert_eq!(ctx2.opponent_count(), 0);
}

#[test]
fn test_bridge_context_shared_extensions_access() {
    use crate::SessionExtension;

    #[derive(Debug)]
    struct SharedExt {
        value: i32,
    }
    impl SessionExtension for SharedExt {
        fn create() -> Self {
            Self { value: 42 }
        }
    }

    let mut shared = ExtensionMap::new();
    shared.get_or_insert::<SharedExt>();
    let opponents: Vec<(ClientId, &ExtensionMap)> = vec![];
    let ctx = BridgeContext::new(ClientId::new(1), &shared, &opponents);

    let ext = ctx.shared_extensions.get::<SharedExt>();
    assert!(ext.is_some());
    assert_eq!(ext.unwrap().value, 42);
}

// ========================================================================
// snapshot_with_context tests (#543)
// ========================================================================

#[test]
fn test_snapshot_with_context_default_delegates() {
    let bridge = DummyBridge::new("test");
    let map = ExtensionMap::new();
    let shared = ExtensionMap::new();
    let opponents: Vec<(ClientId, &ExtensionMap)> = vec![];
    let ctx = BridgeContext::new(ClientId::new(1), &shared, &opponents);

    // Default snapshot_with_context delegates to snapshot
    let snap = bridge.snapshot_with_context(&map, &ctx);
    let direct = bridge.snapshot(&map);
    assert_eq!(snap, direct);
}

#[test]
fn test_snapshot_with_context_override_uses_context() {
    use crate::SessionExtension;

    #[derive(Debug)]
    struct OpponentScore {
        score: u32,
    }
    impl SessionExtension for OpponentScore {
        fn create() -> Self {
            Self { score: 0 }
        }
    }

    /// Bridge that reads opponent data from context.
    struct MultiplayerBridge;

    impl ExtensionStateBridge for MultiplayerBridge {
        fn kind(&self) -> &'static str {
            "multiplayer"
        }
        fn scope(&self) -> ExtensionScope {
            ExtensionScope::Client
        }
        fn snapshot(&self, _ext: &ExtensionMap) -> Option<serde_json::Value> {
            Some(serde_json::json!({"score": 0}))
        }
        fn is_active(&self, _ext: &ExtensionMap) -> bool {
            true
        }
        fn snapshot_with_context(
            &self,
            _ext: &ExtensionMap,
            context: &BridgeContext<'_>,
        ) -> Option<serde_json::Value> {
            let mut opponent_scores = Vec::new();
            context.for_each_opponent(|id, ext| {
                if let Some(score) = ext.get::<OpponentScore>() {
                    opponent_scores
                        .push(serde_json::json!({"id": id.as_usize(), "score": score.score}));
                }
            });
            Some(serde_json::json!({"opponents": opponent_scores}))
        }
    }

    // Setup: one opponent with a score
    let mut opp_map = ExtensionMap::new();
    let opp_score = opp_map.get_or_insert::<OpponentScore>();
    opp_score.score = 999;

    let shared = ExtensionMap::new();
    let opponents = vec![(ClientId::new(2), &opp_map)];
    let ctx = BridgeContext::new(ClientId::new(1), &shared, &opponents);

    let bridge = MultiplayerBridge;
    let own_map = ExtensionMap::new();
    let snap = bridge.snapshot_with_context(&own_map, &ctx);

    assert!(snap.is_some());
    let json = snap.unwrap();
    let opponents_arr = json["opponents"].as_array().unwrap();
    assert_eq!(opponents_arr.len(), 1);
    assert_eq!(opponents_arr[0]["id"], 2);
    assert_eq!(opponents_arr[0]["score"], 999);
}
