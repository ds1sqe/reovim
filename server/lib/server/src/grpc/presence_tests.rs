use {super::*, crate::session::SessionToken};

fn test_registry() -> Arc<SessionRegistry> {
    let registry = Arc::new(SessionRegistry::new());
    let session = Arc::new(Session::new(SessionId::new("test")));
    registry.insert(&session);
    registry
}

fn test_tokens() -> Arc<TokenRegistry> {
    Arc::new(TokenRegistry::new())
}

/// Helper: build a request with token-authenticated `ClientId` in extensions.
fn authed_request<T>(body: T, client_id: ClientId) -> Request<T> {
    let mut request = Request::new(body);
    request.extensions_mut().insert(client_id);
    request
}

#[tokio::test]
async fn test_join_no_session() {
    let registry = Arc::new(SessionRegistry::new());
    let service = PresenceServiceImpl::new(registry, SessionId::new("nonexistent"), test_tokens());

    let request = Request::new(JoinRequest {
        client_type: "tui".to_string(),
        display_name: "laptop".to_string(),
    });
    let response = service.join(request).await;

    assert!(response.is_err());
    let err = response.err().unwrap();
    assert_eq!(err.code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_join_success() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let request = Request::new(JoinRequest {
        client_type: "tui".to_string(),
        display_name: "laptop".to_string(),
    });
    let response = service.join(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.client_id > 0);
    assert!(resp.peers.is_empty()); // First client has no peers
    // #483: Join now returns a session token
    assert!(!resp.session_token.is_empty());
    assert_eq!(resp.session_token.len(), 32); // 128-bit hex
}

#[tokio::test]
async fn test_join_returns_peers() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    // First client joins
    let request1 = Request::new(JoinRequest {
        client_type: "tui".to_string(),
        display_name: "laptop".to_string(),
    });
    let _ = service.join(request1).await.unwrap();

    // Second client joins
    let request2 = Request::new(JoinRequest {
        client_type: "android".to_string(),
        display_name: "phone".to_string(),
    });
    let response = service.join(request2).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.peers.len(), 1);
    assert_eq!(resp.peers[0].display_name, "laptop");
}

#[tokio::test]
async fn test_leave_success() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    // Join first
    let join_req = Request::new(JoinRequest {
        client_type: "tui".to_string(),
        display_name: "laptop".to_string(),
    });
    let join_resp = service.join(join_req).await.unwrap().into_inner();
    let cid = ClientId::new(join_resp.client_id as usize);

    // Then leave (token identifies client)
    let leave_req = authed_request(LeaveRequest {}, cid);
    let response = service.leave(leave_req).await;

    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().ok);
}

#[tokio::test]
async fn test_leave_unknown_client() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    // Authed as client 999, which doesn't exist
    let request = authed_request(LeaveRequest {}, ClientId::new(999));
    let response = service.leave(request).await;

    assert!(response.is_ok());
    assert!(!response.unwrap().into_inner().ok);
}

#[tokio::test]
async fn test_update_presence_success() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    // Join first
    let join_req = Request::new(JoinRequest {
        client_type: "tui".to_string(),
        display_name: "laptop".to_string(),
    });
    let join_resp = service.join(join_req).await.unwrap().into_inner();
    let cid = ClientId::new(join_resp.client_id as usize);

    // Update presence (token identifies client)
    let update_req = authed_request(
        UpdatePresenceRequest {
            buffer_id: Some(42),
            viewport_state: None,
        },
        cid,
    );
    let response = service.update_presence(update_req).await;

    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().ok);
}

#[tokio::test]
async fn test_update_presence_unknown_client() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let request = authed_request(
        UpdatePresenceRequest {
            buffer_id: Some(42),
            viewport_state: None,
        },
        ClientId::new(999),
    );
    let response = service.update_presence(request).await;

    assert!(response.is_err());
    assert_eq!(response.err().unwrap().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_set_sync_mode_independent() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let join_resp = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let cid = ClientId::new(join_resp.client_id as usize);

    let request = authed_request(
        SetSyncModeRequest {
            mode: ProtoSyncMode::Independent as i32,
            follow_target: None,
        },
        cid,
    );
    let response = service.set_sync_mode(request).await;

    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().ok);
}

#[tokio::test]
async fn test_set_sync_mode_present() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let join_resp = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let cid = ClientId::new(join_resp.client_id as usize);

    let request = authed_request(
        SetSyncModeRequest {
            mode: ProtoSyncMode::Present as i32,
            follow_target: None,
        },
        cid,
    );
    let response = service.set_sync_mode(request).await;

    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().ok);
}

#[tokio::test]
async fn test_set_sync_mode_follow_success() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let resp1 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "presenter".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let presenter_id = resp1.client_id;

    let resp2 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "follower".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let follower_cid = ClientId::new(resp2.client_id as usize);

    let request = authed_request(
        SetSyncModeRequest {
            mode: ProtoSyncMode::Follow as i32,
            follow_target: Some(presenter_id),
        },
        follower_cid,
    );
    let response = service.set_sync_mode(request).await;

    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().ok);
}

#[tokio::test]
async fn test_set_sync_mode_follow_missing_target() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let join_resp = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let cid = ClientId::new(join_resp.client_id as usize);

    let request = authed_request(
        SetSyncModeRequest {
            mode: ProtoSyncMode::Follow as i32,
            follow_target: None,
        },
        cid,
    );
    let response = service.set_sync_mode(request).await;

    assert!(response.is_err());
    assert_eq!(response.err().unwrap().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_set_sync_mode_follow_invalid_target() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let join_resp = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let cid = ClientId::new(join_resp.client_id as usize);

    let request = authed_request(
        SetSyncModeRequest {
            mode: ProtoSyncMode::Follow as i32,
            follow_target: Some(9999),
        },
        cid,
    );
    let response = service.set_sync_mode(request).await;

    assert!(response.is_err());
    assert_eq!(response.err().unwrap().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_list_clients() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    // Initially empty
    let request = Request::new(ListClientsRequest {});
    let response = service.list_clients(request).await;
    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().clients.is_empty());

    // Join two clients
    let _ = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        }))
        .await
        .unwrap();
    let _ = service
        .join(Request::new(JoinRequest {
            client_type: "android".to_string(),
            display_name: "phone".to_string(),
        }))
        .await
        .unwrap();

    // Now should have two clients
    let request = Request::new(ListClientsRequest {});
    let response = service.list_clients(request).await;
    assert!(response.is_ok());
    let clients = response.unwrap().into_inner().clients;
    assert_eq!(clients.len(), 2);
}

#[tokio::test]
async fn test_stream_presence_returns_stream() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let request = Request::new(StreamPresenceRequest {});
    let response = service.stream_presence(request).await;

    // Should successfully return a stream
    assert!(response.is_ok());
}

// ── Token lifecycle tests (#483) ──────────────────────────────────

#[tokio::test]
async fn test_join_returns_session_token() {
    let tokens = test_tokens();
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), Arc::clone(&tokens));

    let resp = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "test".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();

    // Token is present and valid
    assert_eq!(resp.session_token.len(), 32);
    // Token resolves in the registry
    let token = SessionToken::from(resp.session_token.as_str());
    assert_eq!(tokens.resolve(&token), Some(ClientId::new(resp.client_id as usize)));
}

#[tokio::test]
async fn test_leave_revokes_token() {
    let tokens = test_tokens();
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), Arc::clone(&tokens));

    // Join
    let join_resp = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "test".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let token = SessionToken::from(join_resp.session_token.as_str());
    assert!(tokens.resolve(&token).is_some());

    // Leave (token identifies client)
    let cid = ClientId::new(join_resp.client_id as usize);
    service
        .leave(authed_request(LeaveRequest {}, cid))
        .await
        .unwrap();

    // Token is revoked
    assert!(tokens.resolve(&token).is_none());
    assert!(tokens.is_empty());
}

#[test]
fn test_to_proto_client_info_independent() {
    use {
        crate::session::ClientMetadata,
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
    };

    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let mode_stack = ModeStack::new(mode);
    let metadata = ClientMetadata::default();
    let client = Client::with_mode_stack(ClientId::new(42), metadata, mode_stack);

    let info = to_proto_client_info(&client);
    assert_eq!(info.id, 42);
    assert!(info.relation.is_none()); // Independent
    assert!(info.view.is_some());
    // (#753) ClientViewState no longer has a mode field — mode is domain-neutral.
    assert!(info.metadata.is_some());
}

#[test]
fn test_to_proto_client_info_following() {
    use {
        crate::session::{ClientMetadata, ClientRelation},
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
    };

    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let mode_stack = ModeStack::new(mode);
    let metadata = ClientMetadata::default();
    // windows are domain-owned (#753 E3)
    let mut client = Client::with_mode_stack(ClientId::new(5), metadata, mode_stack);
    client.set_relation_unchecked(Some(ClientRelation::Following {
        target: ClientId::new(1),
    }));

    let info = to_proto_client_info(&client);
    assert_eq!(info.id, 5);
    let rel = info.relation.unwrap();
    assert_eq!(rel.r#type, ProtoRelationType::RelationTypeFollowing as i32);
    assert_eq!(rel.target_id, 1);
    // buffer_id is domain-owned (#753 E3) — None without domain driver
    assert!(info.view.as_ref().unwrap().buffer_id.is_none());
}

#[test]
fn test_to_proto_client_info_sharing() {
    use {
        crate::session::{ClientMetadata, ClientRelation},
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
    };

    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let mode_stack = ModeStack::new(mode);
    let metadata = ClientMetadata::default();
    let mut client = Client::with_mode_stack(ClientId::new(3), metadata, mode_stack);
    client.set_relation_unchecked(Some(ClientRelation::Sharing {
        with: ClientId::new(2),
    }));

    let info = to_proto_client_info(&client);
    let rel = info.relation.unwrap();
    assert_eq!(rel.r#type, ProtoRelationType::RelationTypeSharing as i32);
    assert_eq!(rel.target_id, 2);
}

#[test]
fn test_to_proto_presence_independent() {
    let presence = ClientPresence::new(ClientId::new(1), "tui", "laptop");
    let proto = to_proto_presence(&presence);

    assert_eq!(proto.client_id, 1);
    assert_eq!(proto.client_type, "tui");
    assert_eq!(proto.display_name, "laptop");
    assert_eq!(proto.sync_mode, ProtoSyncMode::Independent as i32);
    assert!(proto.follow_target.is_none());
}

#[test]
fn test_to_proto_presence_follow() {
    let mut presence = ClientPresence::new(ClientId::new(2), "cli", "terminal");
    presence.sync_mode = SyncMode::Follow {
        target: ClientId::new(1),
    };

    let proto = to_proto_presence(&presence);
    assert_eq!(proto.sync_mode, ProtoSyncMode::Follow as i32);
    assert_eq!(proto.follow_target, Some(1));
}

#[test]
fn test_to_proto_presence_present() {
    let mut presence = ClientPresence::new(ClientId::new(3), "web", "browser");
    presence.sync_mode = SyncMode::Present;

    let proto = to_proto_presence(&presence);
    assert_eq!(proto.sync_mode, ProtoSyncMode::Present as i32);
    assert!(proto.follow_target.is_none());
}

#[test]
fn test_notification_to_presence_update_joined() {
    let presence = ClientPresence::new(ClientId::new(1), "tui", "laptop");
    let notification = build_presence_joined_notification(&presence);

    let update = notification_to_presence_update(&notification);
    assert!(update.is_some());
    let u = update.unwrap();
    assert!(matches!(u.update, Some(Update::Joined(_))));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_notification_to_presence_update_left() {
    let notification = build_presence_left_notification(ClientId::new(5), "laptop");
    let update = notification_to_presence_update(&notification);
    assert!(update.is_some());
    let u = update.unwrap();
    if let Some(Update::Left(id)) = u.update {
        assert_eq!(id, 5);
    } else {
        panic!("Expected Left update");
    }
}

#[test]
fn test_notification_to_presence_update_updated() {
    let presence = ClientPresence::new(ClientId::new(3), "cli", "term");
    let notification = build_presence_updated_notification(&presence);
    let update = notification_to_presence_update(&notification);
    assert!(update.is_some());
    assert!(matches!(update.unwrap().update, Some(Update::Updated(_))));
}

#[test]
fn test_notification_to_presence_update_non_presence() {
    let notification = Notification {
        event_type: "mode_changed".to_string(),
        timestamp_ms: 0,
        payload: None,
    };
    let update = notification_to_presence_update(&notification);
    assert!(update.is_none());
}

#[tokio::test]
async fn test_set_role_owner() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let join_resp = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let cid = ClientId::new(join_resp.client_id as usize);

    let request = authed_request(
        SetRoleRequest {
            role: ProtoRole::Owner as i32,
            target_id: None,
        },
        cid,
    );
    let response = service.set_role(request).await;
    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().ok);
}

#[tokio::test]
async fn test_set_role_follow() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let r1 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "owner".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();

    let r2 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "follower".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let follower_cid = ClientId::new(r2.client_id as usize);

    let request = authed_request(
        SetRoleRequest {
            role: ProtoRole::Follow as i32,
            target_id: Some(r1.client_id),
        },
        follower_cid,
    );
    let response = service.set_role(request).await;
    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().ok);
}

#[tokio::test]
async fn test_set_role_share() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let r1 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "owner".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();

    let r2 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "sharer".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let sharer_cid = ClientId::new(r2.client_id as usize);

    let request = authed_request(
        SetRoleRequest {
            role: ProtoRole::Share as i32,
            target_id: Some(r1.client_id),
        },
        sharer_cid,
    );
    let response = service.set_role(request).await;
    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().ok);
}

#[tokio::test]
async fn test_set_role_follow_missing_target_id() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let join_resp = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "test".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let cid = ClientId::new(join_resp.client_id as usize);

    let request = authed_request(
        SetRoleRequest {
            role: ProtoRole::Follow as i32,
            target_id: None, // Missing target!
        },
        cid,
    );
    let response = service.set_role(request).await;
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_set_role_client_not_found() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let request = authed_request(
        SetRoleRequest {
            role: ProtoRole::Owner as i32,
            target_id: None,
        },
        ClientId::new(999),
    );
    let response = service.set_role(request).await;
    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(!resp.ok);
    assert!(resp.error.is_some());
}

#[tokio::test]
async fn test_set_role_follow_target_not_found() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let join_resp = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "test".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let cid = ClientId::new(join_resp.client_id as usize);

    let request = authed_request(
        SetRoleRequest {
            role: ProtoRole::Follow as i32,
            target_id: Some(9999), // Non-existent target
        },
        cid,
    );
    let response = service.set_role(request).await;
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::FailedPrecondition);
}

#[tokio::test]
async fn test_set_relation_following() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let r1 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "target".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();

    let r2 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "follower".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let follower_cid = ClientId::new(r2.client_id as usize);

    let request = authed_request(
        SetRelationRequest {
            relation: Some(ProtoClientRelation {
                r#type: ProtoRelationType::RelationTypeFollowing as i32,
                target_id: r1.client_id,
            }),
        },
        follower_cid,
    );
    let response = service.set_relation(request).await;
    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().ok);
}

#[tokio::test]
async fn test_set_relation_sharing() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let r1 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "owner".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();

    let r2 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "sharer".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let sharer_cid = ClientId::new(r2.client_id as usize);

    let request = authed_request(
        SetRelationRequest {
            relation: Some(ProtoClientRelation {
                r#type: ProtoRelationType::RelationTypeSharing as i32,
                target_id: r1.client_id,
            }),
        },
        sharer_cid,
    );
    let response = service.set_relation(request).await;
    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().ok);
}

#[tokio::test]
async fn test_set_relation_independent() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let r1 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "test".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let cid = ClientId::new(r1.client_id as usize);

    // Set to independent (None relation)
    let request = authed_request(SetRelationRequest { relation: None }, cid);
    let response = service.set_relation(request).await;
    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().ok);
}

#[tokio::test]
async fn test_set_relation_target_not_found() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let r1 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "test".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let cid = ClientId::new(r1.client_id as usize);

    let request = authed_request(
        SetRelationRequest {
            relation: Some(ProtoClientRelation {
                r#type: ProtoRelationType::RelationTypeFollowing as i32,
                target_id: 99999,
            }),
        },
        cid,
    );
    let response = service.set_relation(request).await;
    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(!resp.ok);
    assert!(resp.error.is_some());
}

#[tokio::test]
async fn test_set_relation_self_target() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let r1 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "test".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let cid = ClientId::new(r1.client_id as usize);

    // Try to follow self
    let request = authed_request(
        SetRelationRequest {
            relation: Some(ProtoClientRelation {
                r#type: ProtoRelationType::RelationTypeFollowing as i32,
                target_id: r1.client_id,
            }),
        },
        cid,
    );
    let response = service.set_relation(request).await;
    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(!resp.ok);
    // Should have CannotTargetSelf error
    assert!(resp.error.is_some());
}

#[tokio::test]
async fn test_set_sync_mode_client_not_in_presence_map() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    // Client 42 never joined - not in presence map
    let request = authed_request(
        SetSyncModeRequest {
            mode: ProtoSyncMode::Independent as i32,
            follow_target: None,
        },
        ClientId::new(42),
    );
    let response = service.set_sync_mode(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_set_role_share_missing_target_id() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let join_resp = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "test".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let cid = ClientId::new(join_resp.client_id as usize);

    let request = authed_request(
        SetRoleRequest {
            role: ProtoRole::Share as i32,
            target_id: None, // Missing target!
        },
        cid,
    );
    let response = service.set_role(request).await;
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_set_relation_invalid_type_defaults_to_following() {
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let r1 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "target".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();

    let r2 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "follower".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let follower_cid = ClientId::new(r2.client_id as usize);

    // Use invalid relation type (999) - should default to Following
    let request = authed_request(
        SetRelationRequest {
            relation: Some(ProtoClientRelation {
                r#type: 999,
                target_id: r1.client_id,
            }),
        },
        follower_cid,
    );
    let response = service.set_relation(request).await;
    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().ok);
}

#[tokio::test]
async fn test_each_client_gets_unique_token() {
    let tokens = test_tokens();
    let registry = test_registry();
    let service = PresenceServiceImpl::new(registry, SessionId::new("test"), Arc::clone(&tokens));

    let r1 = service
        .join(Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "a".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    let r2 = service
        .join(Request::new(JoinRequest {
            client_type: "cli".to_string(),
            display_name: "b".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();

    assert_ne!(r1.session_token, r2.session_token);
    assert_eq!(tokens.len(), 2);
}

// ── Illuminate tick coverage (#664) ──────────────────────────────────────────

/// Ensure `join()` calls `tick_handle.start()` when a `TickSchedulerHandle` is
/// registered in `state.app.services`.  Without a real scheduler wired in the
/// `start()` call is a no-op, but the three lines that form the `if let` branch
/// are counted by the coverage tool.
#[tokio::test]
async fn test_join_starts_illuminate_tick_when_handle_present() {
    use reovim_driver_text_session::TickSchedulerHandle;

    let state = crate::session::SessionState::default();
    state
        .app
        .services
        .register(Arc::new(TickSchedulerHandle::default()));
    let session = Arc::new(crate::session::Session::from_state(SessionId::new("tick-test"), state));
    let registry = Arc::new(SessionRegistry::new());
    registry.insert(&session);
    let service = PresenceServiceImpl::new(registry, SessionId::new("tick-test"), test_tokens());

    let request = Request::new(JoinRequest {
        client_type: "tui".to_string(),
        display_name: "test-tick".to_string(),
    });
    let response = service.join(request).await;
    // Join should succeed; tick_handle.start() is a no-op without an inner
    // scheduler, but the code path (lines 302-308) is exercised.
    assert!(response.is_ok());
}
