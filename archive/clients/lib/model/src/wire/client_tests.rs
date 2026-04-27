use super::*;

#[test]
fn test_client_relation_following() {
    let relation = ClientRelation::following(42);
    assert!(relation.is_following());
    assert!(!relation.is_sharing());
    assert_eq!(relation.target_id(), 42);
}

#[test]
fn test_client_relation_sharing() {
    let relation = ClientRelation::sharing(42);
    assert!(relation.is_sharing());
    assert!(!relation.is_following());
    assert_eq!(relation.target_id(), 42);
}

#[test]
fn test_client_view_state_default() {
    let view = ClientViewState::default();
    assert!(view.mode.is_empty());
    assert_eq!(view.cursor, (0, 0));
    assert!(view.buffer_id.is_none());
    assert!(view.selection.is_none());
}

#[test]
fn test_client_view_state_builder() {
    let view = ClientViewState::new("NORMAL")
        .with_cursor(10, 5)
        .with_buffer(1)
        .with_selection((5, 0), (10, 5));

    assert_eq!(view.mode, "NORMAL");
    assert_eq!(view.cursor, (10, 5));
    assert_eq!(view.buffer_id, Some(1));
    assert_eq!(view.selection, Some(((5, 0), (10, 5))));
}

#[test]
fn test_client_metadata_new() {
    let metadata = ClientMetadata::new("tui", "my-laptop");
    assert_eq!(metadata.client_type, "tui");
    assert_eq!(metadata.display_name, "my-laptop");
    assert_eq!(metadata.joined_at_ms, 0);
}

#[test]
fn test_client_info_new_independent() {
    let info = ClientInfo::new(1, ClientMetadata::new("tui", "laptop"));
    assert_eq!(info.id, 1);
    assert!(info.is_independent());
    assert!(!info.is_following());
    assert!(!info.is_sharing());
    assert!(info.target_id().is_none());
}

#[test]
fn test_client_info_following() {
    let mut info = ClientInfo::new(1, ClientMetadata::new("tui", "laptop"));
    info.relation = Some(ClientRelation::Following { target: 2 });

    assert!(info.is_following());
    assert!(!info.is_independent());
    assert!(!info.is_sharing());
    assert_eq!(info.target_id(), Some(2));
}

#[test]
fn test_client_metadata_default() {
    let metadata = ClientMetadata::default();
    assert_eq!(metadata.client_type, "unknown");
    assert_eq!(metadata.display_name, "unknown");
}

#[test]
fn test_client_metadata_with_joined_at() {
    let metadata = ClientMetadata::new("tui", "laptop").with_joined_at(1_234_567_890);
    assert_eq!(metadata.joined_at_ms, 1_234_567_890);
}

#[test]
fn test_client_view_state_new() {
    let view = ClientViewState::new("INSERT");
    assert_eq!(view.mode, "INSERT");
    assert_eq!(view.cursor, (0, 0));
}

#[test]
fn test_client_info_sharing() {
    let mut info = ClientInfo::new(1, ClientMetadata::new("tui", "laptop"));
    info.relation = Some(ClientRelation::Sharing { with: 2 });

    assert!(info.is_sharing());
    assert!(!info.is_independent());
    assert!(!info.is_following());
    assert_eq!(info.target_id(), Some(2));
}
