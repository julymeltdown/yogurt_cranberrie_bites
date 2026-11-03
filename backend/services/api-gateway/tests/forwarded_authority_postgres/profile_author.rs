//! Production Profile authority over mTLS with a counted real PostgreSQL port.
//! The counter observes actual lookup admission; it never replaces repository IO.
use super::*;
use platform::actor_delegation::exchange::{ChildRequest, IssuedChild, ParentRequest};
use profile_service::adapters::out::persistence::profile_repository::PgProfileRepository;
use profile_service::adapters::r#in::grpc::author_authority::{
    proto as projection, ProfileAuthorAuthorityGrpcServer, PROFILE_AUTHOR_FORWARDING,
    PROFILE_AUTHOR_POLICY,
};
use profile_service::application::author_existence::{
    AuthorProfileLookupPort, AuthorProfileQueryDto, ExistingAuthorProfileDto,
};
use profile_service::application::errors::ProfileResult;
use std::collections::BTreeMap;

const AUTHOR_METHOD: &str = "/authority.v1.ProfileAuthorAuthorityService/CheckAuthorProfile";
const CREATE_METHOD: &str = "/post.v1.PostService/CreatePost";

struct CountedLookup {
    repository: PgProfileRepository,
    lookups: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl AuthorProfileLookupPort for CountedLookup {
    async fn find_existing_author(
        &self,
        query: AuthorProfileQueryDto,
    ) -> ProfileResult<Option<ExistingAuthorProfileDto>> {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        self.repository.find_existing_author(query).await
    }
}

fn author_request(actor: Uuid) -> projection::CheckAuthorProfileRequest {
    projection::CheckAuthorProfileRequest {
        user_id: actor.to_string(),
    }
}

fn author_child(actor: Uuid) -> anyhow::Result<IssuedChild> {
    let now = chrono::Utc::now().timestamp();
    let parent_body = frame(&post::CreatePostRequest {
        author_id: actor.to_string(),
        content: "Profile authority denial fixture".into(),
        visibility: "PUBLIC".into(),
    });
    let child_body = frame(&author_request(actor));
    let signer = ActorDelegationSigner::local_parity(Workload::Gateway);
    let parent = signer.issue(
        Workload::Post,
        CREATE_METHOD,
        actor,
        None,
        &parent_body,
        now,
    );
    let exchange = DelegationExchange::new(
        signer,
        ActorDelegationVerifier::local_parity(Workload::Post),
        ResourceEvidenceVerifier::new(&[
            ResourceEvidenceSigner::from_seed(&[11; 32])?.public_key()
        ])?,
    )?;
    let prepared = exchange
        .prepare(
            Workload::Post,
            ParentRequest {
                token: &parent,
                method: CREATE_METHOD,
                body: &parent_body,
                operation: None,
            },
            ChildRequest {
                audience: Workload::Profile,
                method: AUTHOR_METHOD,
                body: &child_body,
                operation: None,
            },
            now,
        )
        .map_err(|error| anyhow::anyhow!(error.code()))?;
    exchange
        .finish(prepared, None, now)
        .map_err(|error| anyhow::anyhow!(error.code()))
}

async fn database_rows(pool: &PgPool) -> anyhow::Result<BTreeMap<String, serde_json::Value>> {
    let tables: Vec<String> = sqlx::query_scalar(
        "select tablename::text from pg_tables where schemaname=current_schema() order by tablename",
    ).fetch_all(pool).await?;
    anyhow::ensure!(
        tables.iter().any(|table| table == "profiles"),
        "Profile table required"
    );
    let mut snapshot = BTreeMap::new();
    for table in tables {
        let quoted = format!("\"{}\"", table.replace('"', "\"\""));
        let rows: serde_json::Value = sqlx::query_scalar(&format!(
            "select coalesce(jsonb_agg(to_jsonb(t) order by to_jsonb(t)::text),'[]'::jsonb) from {quoted} t"
        )).fetch_one(pool).await?;
        snapshot.insert(table, rows);
    }
    Ok(snapshot)
}

async fn author_call(
    channel: Channel,
    request: Request<projection::CheckAuthorProfileRequest>,
) -> Result<Response<projection::CheckAuthorProfileResponse>, Status> {
    let mut client = tonic::client::Grpc::new(channel);
    client
        .ready()
        .await
        .map_err(|_| Status::unavailable("fixture channel unavailable"))?;
    client
        .unary(
            request,
            http::uri::PathAndQuery::from_static(AUTHOR_METHOD),
            tonic_prost::ProstCodec::default(),
        )
        .await
}

#[tokio::test]
async fn check_author_profile_wire_denials_preserve_pg_and_skip_lookup() -> anyhow::Result<()> {
    let profile = profile_support::start_postgres_app(false).await?;
    let actor = Uuid::new_v4();
    let other = Uuid::new_v4();
    for user in [actor, other] {
        sqlx::query("insert into profiles(user_id,username,nickname,avatar_url,bio,account_visibility) values ($1,$2,'private nickname','private avatar','private biography','PRIVATE')")
            .bind(user).bind(format!("author_{}", user.simple())).execute(&profile.pool).await?;
    }
    let initial = database_rows(&profile.pool).await?;
    let lookups = Arc::new(AtomicUsize::new(0));
    let profile_tls = tls(Workload::Profile)?;
    let post_tls = tls(Workload::Post)?;
    let gateway_tls = tls(Workload::Gateway)?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("https://{}", listener.local_addr()?);
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = ServerTask::spawn(
        Server::builder().tls_config(profile_tls.server_config()?)?
            .add_service(guard_with_forwarding(
                &PROFILE_AUTHOR_POLICY, PROFILE_AUTHOR_FORWARDING,
                projection::profile_author_authority_service_server::ProfileAuthorAuthorityServiceServer::new(
                    ProfileAuthorAuthorityGrpcServer::new(Arc::new(CountedLookup {
                        repository: PgProfileRepository::new(profile.pool.clone()), lookups: lookups.clone(),
                    }))),
                registry()?, ActorDelegationVerifier::local_parity(Workload::Profile),
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async { let _ = stopped.await; }),
        stop,
    );
    let post_channel = post_tls.connect(&endpoint, Workload::Profile).await?;
    let gateway_channel = gateway_tls.connect(&endpoint, Workload::Profile).await?;
    for case in [
        "wrong_workload",
        "missing_identity",
        "missing_child",
        "other_author",
    ] {
        let child = author_child(actor)?;
        let before = lookups.load(Ordering::SeqCst);
        let (channel, request, expected) = match case {
            "wrong_workload" => (
                gateway_channel.clone(),
                identified(
                    author_request(actor),
                    Workload::Gateway,
                    "gateway-fixture",
                    Some(&child.token),
                ),
                Code::PermissionDenied,
            ),
            "missing_identity" => {
                let mut request = Request::new(author_request(actor));
                request.metadata_mut().insert(HEADER, child.token.parse()?);
                (post_channel.clone(), request, Code::Unauthenticated)
            }
            "missing_child" => (
                post_channel.clone(),
                identified(author_request(actor), Workload::Post, "post-fixture", None),
                Code::Unauthenticated,
            ),
            // A valid child for actor A must not disclose existing private author B.
            // The production guard may reject at body binding before the handler.
            _ => (
                post_channel.clone(),
                identified(
                    author_request(other),
                    Workload::Post,
                    "post-fixture",
                    Some(&child.token),
                ),
                Code::PermissionDenied,
            ),
        };
        assert_eq!(
            author_call(channel, request).await.unwrap_err().code(),
            expected,
            "{case}"
        );
        assert_eq!(
            lookups.load(Ordering::SeqCst),
            before,
            "{case}: no repository lookup"
        );
        assert!(
            database_rows(&profile.pool).await? == initial,
            "{case}: unchanged database rows"
        );

        // The rejected request did not spend the proof, and the same service and
        // actual repository must still accept an authorized existing author.
        let allowed = author_call(
            post_channel.clone(),
            identified(
                author_request(actor),
                Workload::Post,
                "post-fixture",
                Some(&child.token),
            ),
        )
        .await?;
        assert_eq!(allowed.into_inner().user_id, actor.to_string());
        assert_eq!(lookups.load(Ordering::SeqCst), before + 1);
        assert!(
            database_rows(&profile.pool).await? == initial,
            "authorized read changed rows"
        );

        // Reject actual consumption replay, not a token that merely expired.
        let replay = author_call(
            post_channel.clone(),
            identified(
                author_request(actor),
                Workload::Post,
                "post-fixture",
                Some(&child.token),
            ),
        )
        .await
        .unwrap_err();
        assert!(
            chrono::Utc::now().timestamp() < child.expires_at,
            "replay fixture expired"
        );
        assert_eq!(
            replay.code(),
            Code::Unauthenticated,
            "consumed child replay"
        );
        assert_eq!(
            lookups.load(Ordering::SeqCst),
            before + 1,
            "replay: no additional lookup"
        );
        assert!(
            database_rows(&profile.pool).await? == initial,
            "replay changed database rows"
        );
    }
    // Establish that B also exists and that replay refusals did not break the server.
    let child = author_child(other)?;
    let allowed = author_call(
        post_channel,
        identified(
            author_request(other),
            Workload::Post,
            "post-fixture",
            Some(&child.token),
        ),
    )
    .await?;
    assert_eq!(allowed.into_inner().user_id, other.to_string());
    assert_eq!(lookups.load(Ordering::SeqCst), 5);
    assert!(
        database_rows(&profile.pool).await? == initial,
        "final read changed database rows"
    );
    server.stop().await?;
    profile.pool.close().await;
    println!("PROFILE_AUTHORITY_WIRE_DENIALS: real mTLS/Profile PG; wrong workload, missing identity/child, cross-author and replay deny without lookup or row changes; positive controls pass; listener stopped");
    Ok(())
}
