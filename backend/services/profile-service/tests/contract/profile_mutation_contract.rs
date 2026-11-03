use platform::grpc_auth::{VerifiedCaller, Workload};
use profile_service::adapters::r#in::grpc::server::proto;
use profile_service::adapters::r#in::grpc::server::proto::profile_service_server::ProfileService;
use profile_service::application::dto::profile_dto::UpdateProfileDto;
use tonic::{Code, Request};
use uuid::Uuid;

const USER_ID: &str = "11111111-1111-1111-1111-111111111111";

fn gateway<T>(mut request: Request<T>) -> Request<T> {
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    request
}

async fn operation_request<T>(message: T, operation_id: Uuid, actor_id: Uuid) -> Request<T> {
    operation_request_with_key(message, operation_id, actor_id, "privacy-update-1").await
}

async fn operation_request_with_key<T>(
    message: T,
    operation_id: Uuid,
    actor_id: Uuid,
    key: &str,
) -> Request<T> {
    let context = platform::mutation_context::MutationContext::new(
        operation_id,
        actor_id,
        Some(key.into()),
        false,
    );
    platform::mutation_context::scope(Some(context), async {
        gateway(platform::mutation_context::attach(message).expect("operation metadata"))
    })
    .await
}

#[tokio::test]
async fn privacy_mutation_receipt_is_atomic_and_replay_rpc_is_lookup_only() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let user_id = Uuid::parse_str(USER_ID).expect("user UUID");
    app.state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: USER_ID.into(),
            username: "privacy_owner".into(),
            nickname: "Privacy Owner".into(),
            avatar_url: "/avatars/privacy.svg".into(),
            bio: "Owner privacy receipt test".into(),
        })
        .await
        .expect("create profile");
    let grpc = crate::support::grpc_server(app.state.clone());
    let operation_id = Uuid::new_v4();
    let command = proto::UpdateProfilePrivacyRequest {
        user_id: USER_ID.into(),
        actor_id: USER_ID.into(),
        account_visibility: "PRIVATE".into(),
    };

    let unauthenticated = grpc
        .update_profile_privacy(Request::new(command.clone()))
        .await
        .expect_err("privacy mutation must require Gateway identity");
    assert_eq!(unauthenticated.code(), Code::Unauthenticated);

    let first = grpc
        .update_profile_privacy(operation_request(command.clone(), operation_id, user_id).await)
        .await
        .expect("privacy update")
        .into_inner();
    assert_eq!(
        first.profile.as_ref().expect("profile").account_visibility,
        "PRIVATE"
    );

    let replay = grpc
        .replay_mutation(
            operation_request(
                proto::ReplayMutationRequest {
                    command: Some(
                        proto::replay_mutation_request::Command::UpdateProfilePrivacy(command),
                    ),
                },
                operation_id,
                user_id,
            )
            .await,
        )
        .await
        .expect("lookup-only replay")
        .into_inner();
    let Some(proto::replay_mutation_response::Outcome::UpdateProfilePrivacy(replayed)) =
        replay.outcome
    else {
        panic!("lookup-only RPC must return the stored privacy outcome");
    };
    assert_eq!(first.profile, replayed.profile);

    let receipt = grpc
        .get_mutation_receipt_status(gateway(Request::new(
            proto::GetMutationReceiptStatusRequest {
                operation_id: operation_id.to_string(),
            },
        )))
        .await
        .expect("Gateway may inspect minimized receipt status")
        .into_inner();
    assert_eq!(receipt.actor_id, USER_ID);
    assert_eq!(receipt.scope, "profile.v1.UpdateProfilePrivacy");
    assert_eq!(
        proto::get_mutation_receipt_status_response::State::try_from(receipt.state).unwrap(),
        proto::get_mutation_receipt_status_response::State::Replayable
    );

    let command_conflict = grpc
        .update_profile_privacy(
            operation_request(
                proto::UpdateProfilePrivacyRequest {
                    user_id: USER_ID.into(),
                    actor_id: USER_ID.into(),
                    account_visibility: "PUBLIC".into(),
                },
                operation_id,
                user_id,
            )
            .await,
        )
        .await
        .expect_err("operation ID cannot be reused for a different command");
    assert_eq!(command_conflict.code(), Code::AlreadyExists);
    assert_eq!(
        command_conflict
            .metadata()
            .get(platform::mutation_context::ERROR_CODE)
            .and_then(|value| value.to_str().ok()),
        Some("conflict")
    );

    let key_conflict = grpc
        .update_profile_privacy(
            operation_request(
                proto::UpdateProfilePrivacyRequest {
                    user_id: USER_ID.into(),
                    actor_id: USER_ID.into(),
                    account_visibility: "PRIVATE".into(),
                },
                Uuid::new_v4(),
                user_id,
            )
            .await,
        )
        .await
        .expect_err("actor-scoped idempotency key cannot change operation ID");
    assert_eq!(key_conflict.code(), Code::AlreadyExists);
    assert_eq!(
        key_conflict
            .metadata()
            .get(platform::mutation_context::ERROR_CODE)
            .and_then(|value| value.to_str().ok()),
        Some("conflict")
    );

    let actor_mismatch = grpc
        .update_profile_privacy(
            operation_request(
                proto::UpdateProfilePrivacyRequest {
                    user_id: USER_ID.into(),
                    actor_id: USER_ID.into(),
                    account_visibility: "PUBLIC".into(),
                },
                Uuid::new_v4(),
                Uuid::new_v4(),
            )
            .await,
        )
        .await
        .expect_err("owner operation actor must match its profile command");
    assert_eq!(actor_mismatch.code(), Code::PermissionDenied);

    let unauthenticated_status = grpc
        .get_mutation_receipt_status(Request::new(proto::GetMutationReceiptStatusRequest {
            operation_id: operation_id.to_string(),
        }))
        .await
        .expect_err("receipt metadata is not public");
    assert_eq!(unauthenticated_status.code(), Code::Unauthenticated);
    let mut post_caller = Request::new(proto::GetMutationReceiptStatusRequest {
        operation_id: operation_id.to_string(),
    });
    post_caller
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Post));
    assert_eq!(
        grpc.get_mutation_receipt_status(post_caller)
            .await
            .expect_err("Post must not inspect Profile receipts")
            .code(),
        Code::PermissionDenied
    );

    let missing_operation_id = Uuid::new_v4();
    let missing_replay = grpc
        .replay_mutation(
            operation_request_with_key(
                proto::ReplayMutationRequest {
                    command: Some(
                        proto::replay_mutation_request::Command::UpdateProfilePrivacy(
                            proto::UpdateProfilePrivacyRequest {
                                user_id: USER_ID.into(),
                                actor_id: USER_ID.into(),
                                account_visibility: "PUBLIC".into(),
                            },
                        ),
                    ),
                },
                missing_operation_id,
                user_id,
                "privacy-missing-1",
            )
            .await,
        )
        .await
        .expect_err("missing owner receipt must never redispatch the write");
    assert_eq!(missing_replay.code(), Code::AlreadyExists);
    assert_eq!(
        missing_replay
            .metadata()
            .get(platform::mutation_context::ERROR_CODE)
            .and_then(|value| value.to_str().ok()),
        Some("unknown")
    );

    let visibility: String =
        sqlx::query_scalar("select account_visibility from profiles where user_id = $1")
            .bind(user_id)
            .fetch_one(&app.pool)
            .await
            .expect("profile visibility");
    let receipt_count: i64 = sqlx::query_scalar(
        "select count(*) from profile_mutation_receipts where operation_id = $1",
    )
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await
    .expect("receipt count");
    let audit_count: i64 = sqlx::query_scalar(
        "select count(*) from profile_audit_events where action = 'UPDATE_PRIVACY' and profile_user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .expect("audit count");
    assert_eq!(visibility, "PRIVATE", "missing replay did not write PUBLIC");
    assert_eq!(
        receipt_count, 1,
        "only the committed operation has a receipt"
    );
    assert_eq!(
        audit_count, 1,
        "replay did not write a duplicate audit event"
    );
}

#[tokio::test]
async fn profile_edit_receipt_is_atomic_and_replay_rpc_is_lookup_only() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let user_id = Uuid::parse_str(USER_ID).expect("user UUID");
    let reserved_owner = Uuid::parse_str("33333333-3333-3333-3333-333333333333")
        .expect("reserved username owner UUID");
    app.state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: reserved_owner.to_string(),
            username: "reserved_owner".into(),
            nickname: "Reserved Owner".into(),
            avatar_url: "/avatars/reserved.svg".into(),
            bio: "Username remains reserved".into(),
        })
        .await
        .expect("create reserved username owner");
    let grpc = crate::support::grpc_server(app.state.clone());
    let operation_id = Uuid::new_v4();
    let command = proto::UpdateProfileRequest {
        user_id: USER_ID.into(),
        username: "edit_owner_updated".into(),
        nickname: "Edit Owner Updated".into(),
        avatar_url: "/avatars/edit-owner-updated.svg".into(),
        bio: "Updated profile from owner receipt".into(),
    };

    let unauthenticated = grpc
        .update_profile(Request::new(command.clone()))
        .await
        .expect_err("profile edits must require Gateway identity");
    assert_eq!(unauthenticated.code(), Code::Unauthenticated);

    let first = grpc
        .update_profile(
            operation_request_with_key(command.clone(), operation_id, user_id, "profile-edit-1")
                .await,
        )
        .await
        .expect("profile edit")
        .into_inner();
    assert_eq!(
        first.profile.as_ref().expect("profile").username,
        "edit_owner_updated"
    );

    let actor_mismatch = grpc
        .update_profile(
            operation_request_with_key(
                proto::UpdateProfileRequest {
                    user_id: USER_ID.into(),
                    username: "not_committed".into(),
                    nickname: "Wrong Actor".into(),
                    avatar_url: "/avatars/wrong-actor.svg".into(),
                    bio: "Actor id does not own this profile".into(),
                },
                Uuid::new_v4(),
                Uuid::new_v4(),
                "profile-edit-wrong-actor",
            )
            .await,
        )
        .await
        .expect_err("mutation actor must match profile owner");
    assert_eq!(actor_mismatch.code(), Code::PermissionDenied);

    let replay = grpc
        .replay_mutation(
            operation_request_with_key(
                proto::ReplayMutationRequest {
                    command: Some(proto::replay_mutation_request::Command::UpdateProfile(
                        command,
                    )),
                },
                operation_id,
                user_id,
                "profile-edit-1",
            )
            .await,
        )
        .await
        .expect("lookup-only profile edit replay")
        .into_inner();
    let Some(proto::replay_mutation_response::Outcome::UpdateProfile(replayed)) = replay.outcome
    else {
        panic!("lookup-only RPC must return the stored profile edit outcome");
    };
    assert_eq!(first.profile, replayed.profile);

    let status = grpc
        .get_mutation_receipt_status(gateway(Request::new(
            proto::GetMutationReceiptStatusRequest {
                operation_id: operation_id.to_string(),
            },
        )))
        .await
        .expect("Gateway may inspect minimized edit receipt status")
        .into_inner();
    assert_eq!(status.actor_id, USER_ID);
    assert_eq!(status.scope, "profile.v1.UpdateProfile");

    let conflict = grpc
        .update_profile(
            operation_request_with_key(
                proto::UpdateProfileRequest {
                    user_id: USER_ID.into(),
                    username: "edit_owner_updated".into(),
                    nickname: "Edit Owner Updated".into(),
                    avatar_url: "/avatars/edit-owner-updated.svg".into(),
                    bio: "Different canonical command".into(),
                },
                operation_id,
                user_id,
                "profile-edit-1",
            )
            .await,
        )
        .await
        .expect_err("the original command identity cannot be changed");
    assert_eq!(conflict.code(), Code::AlreadyExists);
    assert_eq!(
        conflict
            .metadata()
            .get(platform::mutation_context::ERROR_CODE)
            .and_then(|value| value.to_str().ok()),
        Some("conflict")
    );

    let username_conflict = grpc
        .update_profile(
            operation_request_with_key(
                proto::UpdateProfileRequest {
                    user_id: USER_ID.into(),
                    username: "reserved_owner".into(),
                    nickname: "Should Not Commit".into(),
                    avatar_url: "/avatars/conflict.svg".into(),
                    bio: "Name belongs to another profile".into(),
                },
                Uuid::new_v4(),
                user_id,
                "profile-edit-reserved-name",
            )
            .await,
        )
        .await
        .expect_err("username conflict must not commit an owner receipt");
    assert_eq!(username_conflict.code(), Code::InvalidArgument);

    let cross_scope_key = "profile-edit-cross-scope";
    grpc.update_profile_privacy(
        operation_request_with_key(
            proto::UpdateProfilePrivacyRequest {
                user_id: USER_ID.into(),
                actor_id: USER_ID.into(),
                account_visibility: "PRIVATE".into(),
            },
            Uuid::new_v4(),
            user_id,
            cross_scope_key,
        )
        .await,
    )
    .await
    .expect("commit privacy receipt for cross-scope key check");
    let cross_scope_conflict = grpc
        .update_profile(
            operation_request_with_key(
                proto::UpdateProfileRequest {
                    user_id: USER_ID.into(),
                    username: "edit_owner_updated".into(),
                    nickname: "Edit Owner Updated".into(),
                    avatar_url: "/avatars/edit-owner-updated.svg".into(),
                    bio: "A profile key cannot cross RPC scopes".into(),
                },
                Uuid::new_v4(),
                user_id,
                cross_scope_key,
            )
            .await,
        )
        .await
        .expect_err("actor-scoped keys cannot cross profile RPC scopes");
    assert_eq!(cross_scope_conflict.code(), Code::AlreadyExists);

    let missing_user =
        Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("missing user UUID");
    let missing = grpc
        .replay_mutation(
            operation_request_with_key(
                proto::ReplayMutationRequest {
                    command: Some(proto::replay_mutation_request::Command::UpdateProfile(
                        proto::UpdateProfileRequest {
                            user_id: missing_user.to_string(),
                            username: "missing_edit_owner".into(),
                            nickname: "Missing Owner".into(),
                            avatar_url: "/avatars/missing.svg".into(),
                            bio: "Must remain uncreated".into(),
                        },
                    )),
                },
                Uuid::new_v4(),
                missing_user,
                "profile-edit-missing-1",
            )
            .await,
        )
        .await
        .expect_err("lookup-only edit must not create a missing profile");
    assert_eq!(missing.code(), Code::AlreadyExists);
    assert_eq!(
        missing
            .metadata()
            .get(platform::mutation_context::ERROR_CODE)
            .and_then(|value| value.to_str().ok()),
        Some("unknown")
    );

    let profile: (String, String, String) =
        sqlx::query_as("select username, nickname, bio from profiles where user_id = $1")
            .bind(user_id)
            .fetch_one(&app.pool)
            .await
            .expect("committed profile fields");
    assert_eq!(
        profile,
        (
            "edit_owner_updated".into(),
            "Edit Owner Updated".into(),
            "Updated profile from owner receipt".into(),
        )
    );
    let missing_profile_count: i64 =
        sqlx::query_scalar("select count(*) from profiles where user_id = $1")
            .bind(missing_user)
            .fetch_one(&app.pool)
            .await
            .expect("missing profile count");
    assert_eq!(missing_profile_count, 0);
    let receipt_count: i64 = sqlx::query_scalar(
        "select count(*) from profile_mutation_receipts where scope = 'profile.v1.UpdateProfile'",
    )
    .fetch_one(&app.pool)
    .await
    .expect("profile edit receipt count");
    assert_eq!(receipt_count, 1);
    let rejected_receipt_count: i64 = sqlx::query_scalar(
        "select count(*) from profile_mutation_receipts where idempotency_key = 'profile-edit-reserved-name'",
    )
    .fetch_one(&app.pool)
    .await
    .expect("rejected command receipt count");
    assert_eq!(rejected_receipt_count, 0);
}

/// Forces the interleaving: one contender has inserted its receipt and is
/// parked on the profile row, and the other is observed waiting on that
/// uncommitted receipt before the row is released. Both return the winner's
/// outcome, with one receipt and one UPDATE_PRIVACY audit event.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_duplicate_privacy_update_waits_for_the_winner_and_commits_one_effect() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let user_id = Uuid::parse_str(USER_ID).expect("user UUID");
    app.state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: USER_ID.into(),
            username: "privacy_race".into(),
            nickname: "Privacy Race".into(),
            avatar_url: "/avatars/privacy.svg".into(),
            bio: "Concurrent owner receipt test".into(),
        })
        .await
        .expect("create profile");
    let operation_id = Uuid::new_v4();
    let command = proto::UpdateProfilePrivacyRequest {
        user_id: USER_ID.into(),
        actor_id: USER_ID.into(),
        account_visibility: "PRIVATE".into(),
    };
    let first_request = operation_request(command.clone(), operation_id, user_id).await;
    let second_request = operation_request(command, operation_id, user_id).await;
    warm_pool(&app.pool, 4).await;

    let mut gate = app.pool.begin().await.expect("controller transaction");
    // The receipt claim precedes the profile update, so this row lock parks
    // the first claimant after its receipt insert and before any effect.
    sqlx::query("select 1 from profiles where user_id = $1 for update")
        .bind(user_id)
        .execute(&mut *gate)
        .await
        .expect("hold the profile row");
    let contender = |request| {
        let grpc = crate::support::grpc_server(app.state.clone());
        tokio::spawn(async move { grpc.update_profile_privacy(request).await })
    };
    let first = contender(first_request);
    let second = contender(second_request);
    wait_for_parked_contenders(&mut gate, "update profiles").await;
    gate.commit().await.expect("release the winner");

    let first = first.await.unwrap().expect("first contender").into_inner();
    let second = second
        .await
        .unwrap()
        .expect("second contender")
        .into_inner();
    assert_eq!(first.profile, second.profile);
    assert_eq!(
        first.profile.as_ref().expect("profile").account_visibility,
        "PRIVATE"
    );

    let effects: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from profile_mutation_receipts where actor_id = $1),
                (select count(outcome_json) from profile_mutation_receipts where operation_id = $2),
                (select count(*) from profile_audit_events
                  where action = 'UPDATE_PRIVACY' and profile_user_id = $1)",
    )
    .bind(user_id)
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await
    .expect("effect counts");
    assert_eq!(effects, (1, 1, 1));
}

/// Same forced interleaving for UpdateProfile: the first claimant parks on the
/// profile upsert and the duplicate waits on its uncommitted receipt.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_duplicate_profile_edit_waits_for_the_winner_and_commits_one_effect() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let user_id = Uuid::parse_str(USER_ID).expect("user UUID");
    app.state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: USER_ID.into(),
            username: "edit_race".into(),
            nickname: "Edit Race".into(),
            avatar_url: "/avatars/edit.svg".into(),
            bio: "Before the race".into(),
        })
        .await
        .expect("create profile");
    let operation_id = Uuid::new_v4();
    let command = proto::UpdateProfileRequest {
        user_id: USER_ID.into(),
        username: "edit_race_after".into(),
        nickname: "Edit Race After".into(),
        avatar_url: "/avatars/edit-after.svg".into(),
        bio: "After the race".into(),
    };
    let first_request =
        operation_request_with_key(command.clone(), operation_id, user_id, "edit-race").await;
    let second_request =
        operation_request_with_key(command, operation_id, user_id, "edit-race").await;
    warm_pool(&app.pool, 4).await;

    let mut gate = app.pool.begin().await.expect("controller transaction");
    sqlx::query("select 1 from profiles where user_id = $1 for update")
        .bind(user_id)
        .execute(&mut *gate)
        .await
        .expect("hold the profile row");
    let contender = |request| {
        let grpc = crate::support::grpc_server(app.state.clone());
        tokio::spawn(async move { grpc.update_profile(request).await })
    };
    let first = contender(first_request);
    let second = contender(second_request);
    wait_for_parked_contenders(&mut gate, "insert into profiles").await;
    gate.commit().await.expect("release the winner");

    let first = first.await.unwrap().expect("first contender").into_inner();
    let second = second
        .await
        .unwrap()
        .expect("second contender")
        .into_inner();
    assert_eq!(first, second);

    let effects: (i64, i64, String) = sqlx::query_as(
        "select (select count(*) from profile_mutation_receipts where actor_id = $1),
                (select count(outcome_json) from profile_mutation_receipts where operation_id = $2),
                (select username from profiles where user_id = $1)",
    )
    .bind(user_id)
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await
    .expect("effect counts");
    assert_eq!(effects, (1, 1, "edit_race_after".to_owned()));
}

/// Privacy and profile edits complete their receipt last. A completion failure
/// must leave the profile, its audit trail and the receipt table unchanged; the
/// same operation then commits once.
#[tokio::test]
async fn failed_receipt_completion_rolls_back_privacy_and_edit_and_allows_safe_retry() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let user_id = Uuid::parse_str(USER_ID).expect("user UUID");
    app.state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: USER_ID.into(),
            username: "fault_owner".into(),
            nickname: "Fault Owner".into(),
            avatar_url: "/avatars/fault.svg".into(),
            bio: "Before the fault".into(),
        })
        .await
        .expect("create profile");
    let grpc = crate::support::grpc_server(app.state.clone());
    let state = || async {
        sqlx::query_as::<_, (String, String, i64, i64)>(
            "select account_visibility, username,
                    (select count(*) from profile_audit_events where action = 'UPDATE_PRIVACY'),
                    (select count(*) from profile_mutation_receipts)
             from profiles where user_id = $1",
        )
        .bind(user_id)
        .fetch_one(&app.pool)
        .await
        .expect("profile state")
    };
    let reject = "alter table profile_mutation_receipts add constraint test_reject_outcome check (outcome_json is null) not valid";
    let allow = "alter table profile_mutation_receipts drop constraint test_reject_outcome";

    let privacy_id = Uuid::new_v4();
    let privacy = proto::UpdateProfilePrivacyRequest {
        user_id: USER_ID.into(),
        actor_id: USER_ID.into(),
        account_visibility: "PRIVATE".into(),
    };
    sqlx::query(reject).execute(&app.pool).await.unwrap();
    grpc.update_profile_privacy(
        operation_request_with_key(privacy.clone(), privacy_id, user_id, "privacy-fault").await,
    )
    .await
    .expect_err("receipt failure must fail the privacy transaction");
    let unchanged = ("PUBLIC".to_owned(), "fault_owner".to_owned(), 0, 0);
    assert_eq!(state().await, unchanged);
    sqlx::query(allow).execute(&app.pool).await.unwrap();
    grpc.update_profile_privacy(
        operation_request_with_key(privacy, privacy_id, user_id, "privacy-fault").await,
    )
    .await
    .expect("safe privacy retry");
    assert_eq!(
        state().await,
        ("PRIVATE".to_owned(), "fault_owner".to_owned(), 1, 1)
    );

    let edit_id = Uuid::new_v4();
    let edit = proto::UpdateProfileRequest {
        user_id: USER_ID.into(),
        username: "fault_owner_after".into(),
        nickname: "Fault Owner After".into(),
        avatar_url: "/avatars/fault-after.svg".into(),
        bio: "After the fault".into(),
    };
    sqlx::query(reject).execute(&app.pool).await.unwrap();
    grpc.update_profile(
        operation_request_with_key(edit.clone(), edit_id, user_id, "edit-fault").await,
    )
    .await
    .expect_err("receipt failure must fail the edit transaction");
    assert_eq!(
        state().await,
        ("PRIVATE".to_owned(), "fault_owner".to_owned(), 1, 1)
    );
    sqlx::query(allow).execute(&app.pool).await.unwrap();
    grpc.update_profile(operation_request_with_key(edit, edit_id, user_id, "edit-fault").await)
        .await
        .expect("safe edit retry");
    assert_eq!(
        state().await,
        ("PRIVATE".to_owned(), "fault_owner_after".to_owned(), 1, 2)
    );
}

/// Holds pool connections open together once, so connection setup does not
/// separate the contenders' arrival at their receipt inserts.
async fn warm_pool(pool: &sqlx::PgPool, connections: usize) {
    let mut held = Vec::with_capacity(connections);
    for _ in 0..connections {
        held.push(pool.acquire().await.expect("warm connection"));
    }
}

/// Waits until exactly one contender is parked on the effect statement after
/// inserting its receipt and the other is waiting on that uncommitted receipt.
/// The parked transaction has a 500 ms lock timeout, which bounds how long the
/// second contender may take to arrive.
async fn wait_for_parked_contenders(connection: &mut sqlx::PgConnection, effect_statement: &str) {
    let receipt_insert = "insert into profile_mutation_receipts";
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        // Backend activity is otherwise snapshotted once per transaction.
        sqlx::query("select pg_stat_clear_snapshot()")
            .execute(&mut *connection)
            .await
            .expect("fresh activity snapshot");
        let waiting: Vec<String> = sqlx::query_scalar(
            "select regexp_replace(query, '^\\s+', '') from pg_stat_activity
             where datname = current_database() and wait_event_type = 'Lock'
               and pid <> pg_backend_pid()",
        )
        .fetch_all(&mut *connection)
        .await
        .expect("lock waiters");
        let receipts = waiting
            .iter()
            .filter(|query| query.starts_with(receipt_insert))
            .count();
        let effects = waiting
            .iter()
            .filter(|query| query.starts_with(effect_statement))
            .count();
        if waiting.len() == 2 && receipts == 1 && effects == 1 {
            return;
        }
        assert!(
            waiting.len() == receipts + effects && receipts <= 1 && effects <= 1,
            "unexpected lock waiters: {waiting:#?}"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "contenders never parked: {waiting:#?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
}
