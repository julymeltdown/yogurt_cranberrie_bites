//! Native query-producer acceptance: real PG/Redis and signed root admission
//! through the real method guard, with a relationship recorder (not live Friend).
mod support;
use anyhow::{ensure, Result};
use async_trait::async_trait;
use chrono::Utc;
use http_body_util::Full;
use platform::actor_delegation::{
    exchange::VerifiedParentAuthority, ActorDelegationSigner, ActorDelegationVerifier, HEADER,
};
use platform::grpc_auth::{CallerRegistry, Workload};
use post_domain::post::RelationshipPolicySnapshot;
use post_service::adapters::out::persistence::authority_witness_repository::PgAuthorityWitnessRepository;
use post_service::adapters::r#in::grpc::{
    method_policy::POST_METHOD_POLICY, query_witness::AdmittedQueryWitnessProducer, server::proto,
};
use post_service::application::authority_witness::{
    AuthorityWitnessRepositoryPort, ResolveWitnessDto,
};
use post_service::application::ports::FriendClientPort;
use post_service::application::query_witness::{
    OwnedQueryWitnessProducerPort, PublishedWitnessStageDto,
};
use post_service::application::use_cases::PostApplicationService;
use prost::Message;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tonic::body::Body;
use tonic::server::NamedService;
use tower::ServiceExt;
use uuid::Uuid;

const SEARCH: &str = "/post.v1.PostService/SearchPosts";
#[derive(Clone)]
struct RelationshipRecorder {
    repository: PgAuthorityWitnessRepository,
    stages: Arc<Mutex<Vec<Uuid>>>,
}
#[async_trait]
impl FriendClientPort for RelationshipRecorder {
    async fn fetch_feed_authors(
        &self,
        viewer_id: Uuid,
    ) -> Result<post_service::application::feed_authors::FeedAuthorsDto> {
        let _ = viewer_id;
        anyhow::bail!("unused owner-filtered feed fixture")
    }

    async fn fetch_following_ids(&self, _: Uuid) -> Result<Vec<Uuid>> {
        anyhow::bail!("unused following fixture")
    }
    async fn fetch_follower_ids(&self, _: Uuid) -> Result<Vec<Uuid>> {
        anyhow::bail!("unused follower fixture")
    }
    async fn is_following(&self, _: Uuid, _: Uuid) -> Result<bool> {
        anyhow::bail!("unused following fixture")
    }
    async fn relationship_policy(
        &self,
        viewer_id: Uuid,
        target_id: Uuid,
        stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<RelationshipPolicySnapshot> {
        let policies = self
            .batch_relationship_policies(viewer_id, &[target_id], stage)
            .await?;
        Ok(policies[&target_id])
    }
    async fn batch_relationship_policies(
        &self,
        viewer_id: Uuid,
        targets: &[Uuid],
        stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<HashMap<Uuid, RelationshipPolicySnapshot>> {
        let stage = stage.ok_or_else(|| anyhow::anyhow!("missing query witness"))?;
        let admitted =
            VerifiedParentAuthority::current().ok_or_else(|| anyhow::anyhow!("missing parent"))?;
        ensure!(stage.actor_id() == viewer_id && admitted.actor() == viewer_id);
        ensure!(
            stage.parent_digest() == &admitted.parent_digest() && stage.expires_at() > Utc::now()
        );
        let query = match admitted.request().method {
            "/post.v1.PostService/GetPost" => ResolveWitnessDto::posts(
                stage.stage_id(),
                *stage.parent_digest(),
                vec![Uuid::parse_str(
                    &proto::GetPostRequest::decode(
                        admitted.message().map_err(|denial| denial.status())?,
                    )?
                    .post_id,
                )?],
            )?,
            "/post.v1.PostService/AuthorizeNotificationPosts" => ResolveWitnessDto::posts(
                stage.stage_id(),
                *stage.parent_digest(),
                proto::AuthorizeNotificationPostsRequest::decode(
                    admitted.message().map_err(|denial| denial.status())?,
                )?
                .post_ids
                .iter()
                .map(|id| Uuid::parse_str(id))
                .collect::<Result<Vec<_>, _>>()?,
            )?,
            _ => ResolveWitnessDto::authors(
                stage.stage_id(),
                *stage.parent_digest(),
                targets.to_vec(),
            )?,
        };
        let resolved = self.repository.resolve(&query).await?;
        ensure!(targets
            .iter()
            .all(|id| resolved.members().iter().any(|row| row.author_id() == *id)));
        self.stages.lock().unwrap().push(stage.stage_id());
        Ok(targets
            .iter()
            .map(|id| {
                (
                    *id,
                    RelationshipPolicySnapshot {
                        muted_by_viewer: id.as_u128() <= 25,
                        ..Default::default()
                    },
                )
            })
            .collect())
    }
}
#[derive(Clone)]
struct SearchHandler {
    application: Arc<PostApplicationService>,
    override_query: Option<&'static str>,
    override_viewer: Option<Uuid>,
}
impl NamedService for SearchHandler {
    const NAME: &'static str = "post.v1.PostService";
}
impl tower::Service<http::Request<Body>> for SearchHandler {
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Infallible>> + Send>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, _: http::Request<Body>) -> Self::Future {
        let application = self.application.clone();
        let override_query = self.override_query;
        let override_viewer = self.override_viewer;
        Box::pin(async move {
            let admitted =
                VerifiedParentAuthority::current().expect("guard must scope admitted parent");
            let result: Result<()> = match admitted.request().method {
                "/post.v1.PostService/GetPost" => {
                    let request =
                        proto::GetPostRequest::decode(admitted.message().unwrap()).unwrap();
                    application
                        .get_post_detail(
                            Uuid::parse_str(&request.post_id).unwrap(),
                            admitted.actor(),
                        )
                        .await
                        .map(|_| ())
                }
                "/post.v1.PostService/AuthorizeNotificationPosts" => {
                    let request = proto::AuthorizeNotificationPostsRequest::decode(
                        admitted.message().unwrap(),
                    )
                    .unwrap();
                    post_service::application::use_cases::authorize_notification_posts::AuthorizeNotificationPostsUseCase::new(application)
                        .execute(post_service::application::dto::post_dto::AuthorizeNotificationPostsDto {
                            viewer_id: admitted.actor(), post_ids: request.post_ids.iter().map(|id| Uuid::parse_str(id).unwrap()).collect(),
                        }).await.map(|_| ())
                }
                _ => {
                    let request =
                        proto::SearchPostsRequest::decode(admitted.message().unwrap()).unwrap();
                    application
                        .search_posts(
                            override_viewer.unwrap_or(admitted.actor()),
                            override_query.unwrap_or(&request.query),
                            (!request.cursor.is_empty()).then_some(request.cursor.as_str()),
                            request.limit,
                        )
                        .await
                        .map(|_| ())
                }
            };
            let mut response = http::Response::new(Body::default());
            response.headers_mut().insert(
                "grpc-status",
                if result.is_ok() { "0" } else { "14" }.parse().unwrap(),
            );
            Ok(response)
        })
    }
}
fn request(viewer: Uuid, signed: bool) -> http::Request<Body> {
    root_request(
        SEARCH,
        viewer,
        proto::SearchPostsRequest {
            query: "witness query".into(),
            cursor: String::new(),
            limit: 1,
            viewer_id: viewer.to_string(),
        }
        .encode_to_vec(),
        signed,
    )
}
fn root_request(method: &str, actor: Uuid, message: Vec<u8>, signed: bool) -> http::Request<Body> {
    let bytes = [
        vec![0],
        (message.len() as u32).to_be_bytes().to_vec(),
        message,
    ]
    .concat();
    let token = ActorDelegationSigner::local_parity(Workload::Gateway).issue(
        Workload::Post,
        method,
        actor,
        None,
        &bytes,
        Utc::now().timestamp(),
    );
    let mut request = http::Request::builder()
        .uri(method)
        .header("content-type", "application/grpc")
        .header("x-service-caller", "gateway")
        .header("x-service-token", "test-query-gateway")
        .body(Body::new(Full::new(tonic::codegen::Bytes::from(bytes))))
        .unwrap();
    if signed {
        request.headers_mut().insert(HEADER, token.parse().unwrap());
    }
    request
}
fn guarded(
    application: Arc<PostApplicationService>,
) -> platform::grpc_method_policy::GuardedService<SearchHandler> {
    platform::grpc_method_policy::guard(
        &POST_METHOD_POLICY,
        SearchHandler {
            application,
            override_query: None,
            override_viewer: None,
        },
        CallerRegistry::new()
            .register(Workload::Gateway, "test-query-gateway")
            .unwrap(),
        ActorDelegationVerifier::local_parity(Workload::Post),
    )
    .unwrap()
}
async fn seed(pool: &PgPool) -> Result<()> {
    for n in 1..=50u128 {
        sqlx::query("insert into posts (id,author_id,content,visibility,created_at) values ($1,$2,'witness query','PUBLIC',clock_timestamp()-make_interval(secs=>$3))")
            .bind(Uuid::from_u128(1000+n)).bind(Uuid::from_u128(n)).bind(n as f64).execute(pool).await?;
    }
    Ok(())
}
fn composition(
    app: &support::TestApp,
    storage: PgPool,
) -> (Arc<PostApplicationService>, RelationshipRecorder) {
    let recorder = RelationshipRecorder {
        repository: PgAuthorityWitnessRepository::new(app.pool.clone()),
        stages: Arc::new(Mutex::new(vec![])),
    };
    let producer: Arc<dyn OwnedQueryWitnessProducerPort> = Arc::new(
        AdmittedQueryWitnessProducer::new(Arc::new(PgAuthorityWitnessRepository::new(storage))),
    );
    (
        app.application_with_query_witnesses(producer, Arc::new(recorder.clone())),
        recorder,
    )
}

#[tokio::test]
async fn actual_search_batches_publish_before_policy_and_exclude_sql_lookahead() -> Result<()> {
    let app = support::start_postgres_redis_app(false).await?;
    seed(&app.pool).await?;
    let (application, recorder) = composition(&app, app.pool.clone());
    let viewer = Uuid::new_v4();
    let response = guarded(application).oneshot(request(viewer, true)).await?;
    assert_eq!(response.headers()["grpc-status"], "0");
    let stages = recorder.stages.lock().unwrap().clone();
    assert_eq!(stages.len(), 2);
    assert_ne!(stages[0], stages[1]);
    let records = sqlx::query("select post_ids,actor_id,batch_sequence,parent_digest from post_authority_resource_witnesses where stage_id=$1")
        .bind(stages[0]).fetch_one(&app.pool).await?;
    let posts: Vec<Uuid> = records.try_get("post_ids")?;
    assert_eq!(posts.len(), 25);
    assert!(posts.contains(&Uuid::from_u128(1025)));
    assert!(
        !posts.contains(&Uuid::from_u128(1026)),
        "SQL lookahead cannot be authorized"
    );
    assert_eq!(records.try_get::<Uuid, _>("actor_id")?, viewer);
    assert_eq!(records.try_get::<i64, _>("batch_sequence")?, 0);
    let next = sqlx::query("select post_ids,batch_sequence,parent_digest from post_authority_resource_witnesses where stage_id=$1")
        .bind(stages[1]).fetch_one(&app.pool).await?;
    assert_eq!(next.try_get::<i64, _>("batch_sequence")?, 1);
    assert_eq!(
        next.try_get::<Vec<u8>, _>("parent_digest")?,
        records.try_get::<Vec<u8>, _>("parent_digest")?
    );
    let next_posts: Vec<Uuid> = next.try_get("post_ids")?;
    assert!(next_posts.contains(&Uuid::from_u128(1026)));
    assert!(!next_posts.contains(&Uuid::from_u128(1025)));
    Ok(())
}

#[tokio::test]
async fn missing_admission_or_unavailable_publication_prevents_dependent_policy_calls() -> Result<()>
{
    let app = support::start_postgres_redis_app(false).await?;
    seed(&app.pool).await?;
    let (application, recorder) = composition(&app, app.pool.clone());
    assert!(application
        .search_posts(Uuid::new_v4(), "witness query", None, 1)
        .await
        .is_err());
    assert!(recorder.stages.lock().unwrap().is_empty());
    let response = guarded(application)
        .oneshot(request(Uuid::new_v4(), false))
        .await?;
    assert_ne!(response.headers()["grpc-status"], "0");
    let closed = PgPool::connect_with(app.pool.connect_options().as_ref().clone()).await?;
    closed.close().await;
    let (application, recorder) = composition(&app, closed);
    let response = guarded(application)
        .oneshot(request(Uuid::new_v4(), true))
        .await?;
    assert_eq!(response.headers()["grpc-status"], "14");
    assert!(recorder.stages.lock().unwrap().is_empty());
    let count: i64 = sqlx::query_scalar("select count(*) from post_authority_resource_witnesses")
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(count, 0);
    Ok(())
}

#[tokio::test]
async fn concurrent_admitted_queries_keep_parent_actor_and_stage_separate() -> Result<()> {
    let app = support::start_postgres_redis_app(false).await?;
    seed(&app.pool).await?;
    let (application, recorder) = composition(&app, app.pool.clone());
    let owner = guarded(application);
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let (a, b) = tokio::join!(
        owner.clone().oneshot(request(first, true)),
        owner.oneshot(request(second, true))
    );
    assert_eq!(a?.headers()["grpc-status"], "0");
    assert_eq!(b?.headers()["grpc-status"], "0");
    assert_eq!(recorder.stages.lock().unwrap().len(), 4);
    let counts = sqlx::query("select actor_id,count(distinct parent_digest) as parents,count(*) as stages from post_authority_resource_witnesses group by actor_id")
        .fetch_all(&app.pool).await?;
    assert_eq!(counts.len(), 2);
    for row in counts {
        assert!([first, second].contains(&row.try_get::<Uuid, _>("actor_id")?));
        assert_eq!(row.try_get::<i64, _>("parents")?, 1);
        assert_eq!(row.try_get::<i64, _>("stages")?, 2);
    }
    Ok(())
}

#[tokio::test]
async fn changed_query_or_viewer_cannot_publish_under_the_admitted_root() -> Result<()> {
    let app = support::start_postgres_redis_app(false).await?;
    seed(&app.pool).await?;
    let (application, recorder) = composition(&app, app.pool.clone());
    for (override_query, override_viewer) in
        [(Some("other query"), None), (None, Some(Uuid::new_v4()))]
    {
        let service = platform::grpc_method_policy::guard(
            &POST_METHOD_POLICY,
            SearchHandler {
                application: application.clone(),
                override_query,
                override_viewer,
            },
            CallerRegistry::new().register(Workload::Gateway, "test-query-gateway")?,
            ActorDelegationVerifier::local_parity(Workload::Post),
        )?;
        let response = service.oneshot(request(Uuid::new_v4(), true)).await?;
        assert_eq!(response.headers()["grpc-status"], "14");
    }
    assert!(recorder.stages.lock().unwrap().is_empty());
    let count: i64 = sqlx::query_scalar("select count(*) from post_authority_resource_witnesses")
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(count, 0);
    Ok(())
}

#[tokio::test]
async fn fixed_post_and_notification_queries_record_exact_root_ids_and_missing_rows() -> Result<()>
{
    let app = support::start_postgres_redis_app(false).await?;
    seed(&app.pool).await?;
    let (application, recorder) = composition(&app, app.pool.clone());
    let viewer = Uuid::new_v4();
    let get_post = root_request(
        "/post.v1.PostService/GetPost",
        viewer,
        proto::GetPostRequest {
            post_id: Uuid::from_u128(1026).to_string(),
            viewer_id: viewer.to_string(),
        }
        .encode_to_vec(),
        true,
    );
    let response = guarded(application.clone()).oneshot(get_post).await?;
    assert_eq!(response.headers()["grpc-status"], "0");
    let missing = Uuid::from_u128(9999);
    let notify = root_request(
        "/post.v1.PostService/AuthorizeNotificationPosts",
        viewer,
        proto::AuthorizeNotificationPostsRequest {
            viewer_id: viewer.to_string(),
            post_ids: vec![Uuid::from_u128(1026).to_string(), missing.to_string()],
        }
        .encode_to_vec(),
        true,
    );
    let response = guarded(application).oneshot(notify).await?;
    assert_eq!(response.headers()["grpc-status"], "0");
    let stages = recorder.stages.lock().unwrap().clone();
    assert_eq!(stages.len(), 2);
    let row = sqlx::query("select root_kind,root_resources,post_ids,missing_post_ids from post_authority_resource_witnesses where stage_id=$1")
        .bind(stages[1]).fetch_one(&app.pool).await?;
    assert_eq!(row.try_get::<String, _>("root_kind")?, "NOTIFICATION_POSTS");
    assert_eq!(
        row.try_get::<Vec<Uuid>, _>("root_resources")?,
        vec![Uuid::from_u128(1026), missing]
    );
    assert_eq!(
        row.try_get::<Vec<Uuid>, _>("post_ids")?,
        vec![Uuid::from_u128(1026)]
    );
    assert_eq!(
        row.try_get::<Vec<Uuid>, _>("missing_post_ids")?,
        vec![missing]
    );
    Ok(())
}
