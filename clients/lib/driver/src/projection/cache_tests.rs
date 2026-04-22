use super::*;

fn make_projection(tag: &str, domain_id: u32, display: &str, version: u64) -> DomainProjection {
    DomainProjection {
        tag: ProjectionTag::new(tag),
        domain_id: DomainId(domain_id),
        window_id: None,
        content: vec![],
        display: display.to_string(),
        transient: false,
        version,
        client_id: 0,
    }
}

fn make_transient(tag: &str, domain_id: u32, display: &str) -> DomainProjection {
    DomainProjection {
        tag: ProjectionTag::new(tag),
        domain_id: DomainId(domain_id),
        window_id: None,
        content: vec![],
        display: display.to_string(),
        transient: true,
        version: 0,
        client_id: 0,
    }
}

#[test]
fn cache_miss_returns_none() {
    let cache = ProjectionDisplayCache::new();
    assert!(
        cache
            .get(DomainId(1), &ProjectionTag::new("text.mode"))
            .is_none()
    );
}

#[test]
fn cache_hit_after_update() {
    let mut cache = ProjectionDisplayCache::new();
    let proj = make_projection("text.mode", 1, "NORMAL", 1);
    cache.update(&proj);

    assert_eq!(cache.get(DomainId(1), &ProjectionTag::new("text.mode")), Some("NORMAL"));
}

#[test]
fn newer_version_overwrites() {
    let mut cache = ProjectionDisplayCache::new();
    cache.update(&make_projection("text.mode", 1, "NORMAL", 1));
    cache.update(&make_projection("text.mode", 1, "INSERT", 2));

    assert_eq!(cache.get(DomainId(1), &ProjectionTag::new("text.mode")), Some("INSERT"));
}

#[test]
fn older_version_does_not_overwrite() {
    let mut cache = ProjectionDisplayCache::new();
    cache.update(&make_projection("text.mode", 1, "INSERT", 5));
    cache.update(&make_projection("text.mode", 1, "NORMAL", 3));

    assert_eq!(cache.get(DomainId(1), &ProjectionTag::new("text.mode")), Some("INSERT"));
}

#[test]
fn transient_projections_skip_cache() {
    let mut cache = ProjectionDisplayCache::new();
    cache.update(&make_projection("text.mode", 1, "NORMAL", 1));
    cache.update(&make_transient("text.mode", 1, "FLASH"));

    assert_eq!(cache.get(DomainId(1), &ProjectionTag::new("text.mode")), Some("NORMAL"));
}

#[test]
fn transient_does_not_create_entry() {
    let mut cache = ProjectionDisplayCache::new();
    cache.update(&make_transient("text.flash", 1, "highlight"));

    assert!(cache.is_empty());
}

#[test]
fn evict_domain() {
    let mut cache = ProjectionDisplayCache::new();
    cache.update(&make_projection("text.mode", 1, "NORMAL", 1));
    cache.update(&make_projection("text.cursor", 1, "1:0", 1));
    cache.update(&make_projection("3d.transform", 2, "xyz", 1));

    assert_eq!(cache.len(), 3);
    cache.evict_domain(DomainId(1));
    assert_eq!(cache.len(), 1);
    assert!(
        cache
            .get(DomainId(1), &ProjectionTag::new("text.mode"))
            .is_none()
    );
    assert_eq!(cache.get(DomainId(2), &ProjectionTag::new("3d.transform")), Some("xyz"));
}

#[test]
fn clear_removes_all() {
    let mut cache = ProjectionDisplayCache::new();
    cache.update(&make_projection("text.mode", 1, "NORMAL", 1));
    cache.update(&make_projection("3d.mesh", 2, "cube", 1));

    assert_eq!(cache.len(), 2);
    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn entries_for_domain_filters() {
    let mut cache = ProjectionDisplayCache::new();
    cache.update(&make_projection("text.mode", 1, "NORMAL", 1));
    cache.update(&make_projection("text.cursor", 1, "1:0", 1));
    cache.update(&make_projection("3d.transform", 2, "xyz", 1));

    assert_eq!(cache.entries_for_domain(DomainId(1)).count(), 2);
    assert_eq!(cache.entries_for_domain(DomainId(2)).count(), 1);
}

#[test]
fn different_domains_same_tag_independent() {
    let mut cache = ProjectionDisplayCache::new();
    cache.update(&make_projection("mode", 1, "TEXT-NORMAL", 1));
    cache.update(&make_projection("mode", 2, "3D-ORBIT", 1));

    assert_eq!(cache.get(DomainId(1), &ProjectionTag::new("mode")), Some("TEXT-NORMAL"));
    assert_eq!(cache.get(DomainId(2), &ProjectionTag::new("mode")), Some("3D-ORBIT"));
}
