//! Direct resource-authority boundary checks reusing the composed search fixture.
use super::*;
use platform::actor_delegation::exchange::ParentRequest;
use post_service::application::authority_witness::{
    AuthorityWitnessDto, AuthorityWitnessRepositoryPort, PurgeWitnessesDto, ResolveWitnessDto,
    WitnessCleanupOutcomeDto, WitnessErrorDto, WitnessSubsetDto,
};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Calls {
    current: AtomicUsize,
    historical: AtomicUsize,
    publish: AtomicUsize,
    purge: AtomicUsize,
}
pub(super) struct CountedRepository {
    pub repository: PgAuthorityWitnessRepository,
    pub calls: Arc<Calls>,
}
#[async_trait::async_trait]
impl AuthorityWitnessRepositoryPort for CountedRepository {
    async fn publish(&self, witness: &AuthorityWitnessDto) -> Result<(), WitnessErrorDto> {
        self.calls.publish.fetch_add(1, Ordering::SeqCst);
        self.repository.publish(witness).await
    }
    async fn resolve(
        &self,
        request: &ResolveWitnessDto,
    ) -> Result<WitnessSubsetDto, WitnessErrorDto> {
        self.calls.historical.fetch_add(1, Ordering::SeqCst);
        self.repository.resolve(request).await
    }
    async fn resolve_current(
        &self,
        request: &ResolveWitnessDto,
    ) -> Result<WitnessSubsetDto, WitnessErrorDto> {
        self.calls.current.fetch_add(1, Ordering::SeqCst);
        self.repository.resolve_current(request).await
    }
    async fn purge_expired(
        &self,
        request: &PurgeWitnessesDto,
    ) -> Result<WitnessCleanupOutcomeDto, WitnessErrorDto> {
        self.calls.purge.fetch_add(1, Ordering::SeqCst);
        self.repository.purge_expired(request).await
    }
}
impl Calls {
    fn observed(&self) -> [usize; 4] {
        [&self.current, &self.historical, &self.publish, &self.purge]
            .map(|count| count.load(Ordering::SeqCst))
    }
}
async fn rows(pool: &PgPool) -> anyhow::Result<BTreeMap<String, serde_json::Value>> {
    let tables: Vec<String> = sqlx::query_scalar("select tablename::text from pg_tables where schemaname=current_schema() order by tablename").fetch_all(pool).await?;
    anyhow::ensure!(tables
        .iter()
        .any(|table| table == "post_authority_resource_witnesses"));
    let mut snapshot = BTreeMap::new();
    for table in tables {
        let quoted = format!("\"{}\"", table.replace('"', "\"\""));
        let values: serde_json::Value = sqlx::query_scalar(&format!("select coalesce(jsonb_agg(to_jsonb(t) order by to_jsonb(t)::text),'[]'::jsonb) from {quoted} t")).fetch_one(pool).await?;
        snapshot.insert(table, values);
    }
    Ok(snapshot)
}

pub(super) async fn assert_resolve_post_resources_denials(
    pool: &PgPool,
    calls: &Calls,
    gateway: Channel,
    post: Channel,
    parent: owner::ParentAuthority,
    stage: Uuid,
    admitted_author: Uuid,
) -> anyhow::Result<()> {
    let verified = ActorDelegationVerifier::local_parity(Workload::Post)
        .verify_resource_parent(
            ParentRequest {
                token: &parent.token,
                method: &parent.method,
                body: &parent.framed_body,
                operation: None,
            },
            chrono::Utc::now().timestamp(),
        )
        .map_err(|error| anyhow::anyhow!(error.code()))?;
    let expires_at = verified.expires_at();
    let baseline = rows(pool).await?;
    let request = owner::ResolvePostResourcesRequest {
        parent: Some(parent.clone()),
        witness_id: stage.to_string(),
        target_author_ids: vec![admitted_author.to_string()],
    };
    for case in [
        "missing_identity",
        "wrong_workload",
        "wrong_actor",
        "unknown_stage",
        "foreign_parent",
        "unadmitted_author",
    ] {
        let before = calls.observed();
        let mut denied = request.clone();
        let (channel, request, code, lookup_delta) = match case {
            "missing_identity" => (
                gateway.clone(),
                Request::new(denied),
                Code::Unauthenticated,
                0,
            ),
            "wrong_workload" => (
                post.clone(),
                identified(denied, Workload::Post, "post-fixture", None),
                Code::PermissionDenied,
                0,
            ),
            _ => {
                let delta = match case {
                    "wrong_actor" => {
                        // Valid signature/body digest, but the token actor contradicts the body.
                        denied.parent.as_mut().unwrap().token =
                            ActorDelegationSigner::local_parity(Workload::Gateway).issue(
                                Workload::Post,
                                &parent.method,
                                Uuid::new_v4(),
                                None,
                                &parent.framed_body,
                                chrono::Utc::now().timestamp(),
                            );
                        0
                    }
                    "unknown_stage" => {
                        denied.witness_id = Uuid::new_v4().to_string();
                        1
                    }
                    "foreign_parent" => {
                        // Coherent actor/body with a fresh token cannot borrow this stage's parent digest.
                        denied.parent.as_mut().unwrap().token =
                            ActorDelegationSigner::local_parity(Workload::Gateway).issue(
                                Workload::Post,
                                &parent.method,
                                verified.actor(),
                                None,
                                &parent.framed_body,
                                chrono::Utc::now().timestamp(),
                            );
                        assert_ne!(denied.parent.as_ref().unwrap().token, parent.token);
                        1
                    }
                    "unadmitted_author" => {
                        denied.target_author_ids = vec![Uuid::new_v4().to_string()];
                        1
                    }
                    _ => unreachable!(),
                };
                (
                    gateway.clone(),
                    identified(denied, Workload::Gateway, "gateway-fixture", None),
                    Code::PermissionDenied,
                    delta,
                )
            }
        };
        let mut client =
            owner::post_resource_authority_service_client::PostResourceAuthorityServiceClient::new(
                channel,
            );
        let error = client.resolve_post_resources(request).await.unwrap_err();
        assert!(
            chrono::Utc::now().timestamp() < expires_at,
            "resource fixture expired during {case}"
        );
        assert_eq!(error.code(), code, "{case}");
        assert_eq!(
            calls.observed(),
            [before[0] + lookup_delta, before[1], before[2], before[3]],
            "{case}: repository admission/publication"
        );
        assert!(
            rows(pool).await? == baseline,
            "{case}: database rows changed"
        );
        // Resource lookup intentionally reuses its signed parent; unlike child admission,
        // repeat reads must succeed while the already-published witness remains live.
        for repeat in 0..2 {
            let mut client = owner::post_resource_authority_service_client::PostResourceAuthorityServiceClient::new(gateway.clone());
            let result = client
                .resolve_post_resources(identified(
                    request_for(&parent, stage, admitted_author),
                    Workload::Gateway,
                    "gateway-fixture",
                    None,
                ))
                .await?;
            assert!(!result.into_inner().evidence.is_empty());
            assert!(
                chrono::Utc::now().timestamp() < expires_at,
                "positive fixture expired"
            );
            assert_eq!(
                calls.observed(),
                [
                    before[0] + lookup_delta + repeat + 1,
                    before[1],
                    before[2],
                    before[3]
                ]
            );
            assert!(
                rows(pool).await? == baseline,
                "positive resource lookup changed rows"
            );
        }
    }
    Ok(())
}
fn request_for(
    parent: &owner::ParentAuthority,
    stage: Uuid,
    author: Uuid,
) -> owner::ResolvePostResourcesRequest {
    owner::ResolvePostResourcesRequest {
        parent: Some(parent.clone()),
        witness_id: stage.to_string(),
        target_author_ids: vec![author.to_string()],
    }
}
