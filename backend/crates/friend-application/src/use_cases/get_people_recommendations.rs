use crate::ports::{
    PeopleRankerPort, PeopleRecommendationClockPort, PeopleRecommendationFallbackReason,
    PeopleRecommendationObserverPort, PeopleRecommendationRepositoryPort,
    PeopleRecommendationRequestOutcome, ProfileDiscoveryPort, RecommendationRollout,
    RelationshipPolicyRepositoryPort,
};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use friend_domain::error::FriendError;
use friend_domain::recommendation::{
    PeopleRecommendationImpression, PeopleSnapshot, PeopleSnapshotItem, ProfileDiscoverySnapshot,
};
use recommendation_policy::recommendation::{
    RecentActionLimit, RecommendationCursor, RecommendationSurface, RolloutMode,
};
use recommendation_policy::recommendation_sequence::{RecentSequence, RecentSequencePolicy};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

const DEFAULT_LIMIT: i32 = 20;
const MAX_LIMIT: i32 = 50;
const RAW_CANDIDATE_LIMIT: i32 = 800;
const MAX_FINAL_CANDIDATES: usize = 100;
const SNAPSHOT_TTL_MINUTES: i64 = 30;
const BATCH_SIZE: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetPeopleRecommendationsQuery {
    pub viewer_id: Uuid,
    pub cursor: Option<String>,
    pub limit: i32,
    pub request_id: String,
    pub session_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeopleRecommendationItem {
    pub candidate_user_id: Uuid,
    pub source: String,
    pub reason_code: String,
    pub score_micros: i64,
    pub rank_version: String,
    pub impression_id: Uuid,
    pub profile: ProfileDiscoverySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeopleRecommendationPage {
    pub request_id: String,
    pub snapshot_id: Uuid,
    pub rank_version: String,
    pub items: Vec<PeopleRecommendationItem>,
    pub next_cursor: Option<String>,
    pub fallback_used: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeopleSequenceConfig {
    pub enabled: bool,
    pub limit: i32,
}

impl Default for PeopleSequenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            limit: 20,
        }
    }
}

#[derive(Clone)]
pub struct GetPeopleRecommendationsUseCase<R, P, L> {
    repository: R,
    profiles: P,
    relationships: L,
    ranker: Arc<dyn PeopleRankerPort>,
    clock: Arc<dyn PeopleRecommendationClockPort>,
    sequence_config: PeopleSequenceConfig,
    observer: Arc<dyn PeopleRecommendationObserverPort>,
}

impl<R, P, L> GetPeopleRecommendationsUseCase<R, P, L>
where
    R: PeopleRecommendationRepositoryPort,
    P: ProfileDiscoveryPort,
    L: RelationshipPolicyRepositoryPort,
{
    pub fn new(
        repository: R,
        profiles: P,
        relationships: L,
        ranker: Arc<dyn PeopleRankerPort>,
        clock: Arc<dyn PeopleRecommendationClockPort>,
        observer: Arc<dyn PeopleRecommendationObserverPort>,
    ) -> Self {
        Self {
            repository,
            profiles,
            relationships,
            ranker,
            clock,
            sequence_config: PeopleSequenceConfig::default(),
            observer,
        }
    }

    pub fn with_sequence_config(mut self, config: PeopleSequenceConfig) -> Self {
        self.sequence_config = PeopleSequenceConfig {
            enabled: config.enabled,
            limit: config.limit.clamp(1, 100),
        };
        self
    }

    pub async fn execute(
        &self,
        query: GetPeopleRecommendationsQuery,
    ) -> Result<PeopleRecommendationPage> {
        let started = Instant::now();
        let result = self.execute_inner(query).await;
        self.observer.record_request(if result.is_ok() {
            PeopleRecommendationRequestOutcome::Success
        } else {
            PeopleRecommendationRequestOutcome::Failure
        });
        self.observer.observe_snapshot_latency(started.elapsed());
        result
    }

    async fn execute_inner(
        &self,
        query: GetPeopleRecommendationsQuery,
    ) -> Result<PeopleRecommendationPage> {
        validate_query(&query)?;
        let now = self.clock.now_utc();
        let serving = self.serving_selection().await;
        if serving.fallback_used {
            self.observer
                .record_fallback(PeopleRecommendationFallbackReason::ModelUnavailable);
        }
        let limit = normalize_limit(query.limit);
        let parsed_cursor = query
            .cursor
            .as_deref()
            .map(RecommendationCursor::parse)
            .transpose()?;

        let snapshot = if let Some(cursor) = parsed_cursor.as_ref() {
            self.repository
                .find_snapshot(query.viewer_id, cursor.snapshot_id, now)
                .await?
                .ok_or_else(|| FriendError::NotFound("Recommendation snapshot not found".into()))?
        } else {
            self.build_snapshot(query.viewer_id, now).await?
        };

        let mut snapshot_items = self
            .repository
            .load_snapshot_page(snapshot.id, parsed_cursor.as_ref(), limit)
            .await?;
        let has_more = snapshot_items.len() > limit as usize;
        if has_more {
            snapshot_items.truncate(limit as usize);
        }

        let candidate_ids = snapshot_items
            .iter()
            .map(|item| item.candidate_user_id)
            .collect::<Vec<_>>();
        let profiles = self.fetch_profiles(&candidate_ids).await?;
        let authoritative_ineligible = self
            .relationship_ineligible(query.viewer_id, &candidate_ids)
            .await?;
        let repository_ineligible = self
            .repository
            .ineligible_candidate_ids(query.viewer_id, &candidate_ids)
            .await?;

        let mut delivered = Vec::with_capacity(snapshot_items.len());
        let mut impressions = Vec::with_capacity(snapshot_items.len());
        for item in snapshot_items.iter() {
            if authoritative_ineligible.contains(&item.candidate_user_id)
                || repository_ineligible.contains(&item.candidate_user_id)
            {
                continue;
            }
            let Some(profile) = profiles.get(&item.candidate_user_id).cloned() else {
                continue;
            };
            if !profile.is_discoverable() {
                continue;
            }
            let impression_id = Uuid::new_v4();
            let position = i32::try_from(delivered.len()).unwrap_or(i32::MAX);
            delivered.push(PeopleRecommendationItem {
                candidate_user_id: item.candidate_user_id,
                source: item.source.as_str().into(),
                reason_code: item.reason_code.clone(),
                score_micros: item.score_micros,
                rank_version: item.rank_version.clone(),
                impression_id,
                profile,
            });
            impressions.push(PeopleRecommendationImpression {
                impression_id,
                snapshot_id: snapshot.id,
                viewer_id: query.viewer_id,
                candidate_user_id: item.candidate_user_id,
                source: item.source,
                reason_code: item.reason_code.clone(),
                position,
                rank_version: item.rank_version.clone(),
                score_micros: item.score_micros,
                request_id: query.request_id.clone(),
                session_id: query.session_id,
                occurred_at: now,
                propensity_micros: 1_000_000,
            });
        }
        self.repository
            .record_impressions(query.viewer_id, snapshot.id, &impressions)
            .await?;

        let next_cursor = has_more
            .then(|| snapshot_items.last())
            .flatten()
            .map(friend_domain::recommendation::recommendation_cursor_from_item)
            .map(|cursor| cursor.encode());

        Ok(PeopleRecommendationPage {
            request_id: query.request_id,
            snapshot_id: snapshot.id,
            rank_version: snapshot.rank_version,
            items: delivered,
            next_cursor,
            fallback_used: serving.fallback_used,
        })
    }

    async fn serving_selection(&self) -> ServingSelection {
        let fallback = ServingSelection {
            fallback_used: true,
        };
        let rollout = match self.repository.rollout("PEOPLE_RECOMMENDATION").await {
            Ok(rollout) => rollout,
            Err(error) => {
                tracing::warn!(error = ?error, "people recommendation rollout lookup failed; using deterministic ranker");
                return fallback;
            }
        };
        select_serving_ranker(&rollout, self.ranker.version())
    }

    async fn build_snapshot(&self, viewer_id: Uuid, now: DateTime<Utc>) -> Result<PeopleSnapshot> {
        let mut candidates = self
            .repository
            .list_candidates(viewer_id, now, RAW_CANDIDATE_LIMIT)
            .await?;
        if candidates.is_empty() {
            self.observer
                .record_fallback(PeopleRecommendationFallbackReason::EmptyCandidates);
            if let Err(error) = self.repository.enqueue_refresh(&[viewer_id], now).await {
                tracing::warn!(
                    error = ?error,
                    %viewer_id,
                    "empty people projection could not enqueue refresh"
                );
            }
        }
        let mut source_counts = HashMap::new();
        for candidate in &candidates {
            *source_counts.entry(candidate.source).or_insert(0_i64) += 1;
        }
        for (source, count) in source_counts {
            self.observer.record_candidates(source, count);
        }
        let candidate_ids = candidates
            .iter()
            .map(|candidate| candidate.candidate_user_id)
            .collect::<Vec<_>>();
        let repository_ineligible = self
            .repository
            .ineligible_candidate_ids(viewer_id, &candidate_ids)
            .await?;
        let relationship_ineligible = self
            .relationship_ineligible(viewer_id, &candidate_ids)
            .await?;
        let ineligible = repository_ineligible
            .union(&relationship_ineligible)
            .copied()
            .collect::<HashSet<_>>();

        let mut profile_ids = Vec::with_capacity(candidate_ids.len() + 1);
        profile_ids.push(viewer_id);
        profile_ids.extend(candidate_ids.iter().copied());
        let profiles = self.fetch_profiles(&profile_ids).await?;
        let viewer_profile = profiles.get(&viewer_id);
        let (recent_sequence, sequence_fallback) = if self.sequence_config.enabled {
            match self
                .repository
                .recent_actions(
                    viewer_id,
                    RecentActionLimit::new(self.sequence_config.limit),
                )
                .await
            {
                Ok(events) => match RecentSequence::build(
                    viewer_id,
                    RecommendationSurface::PeopleRecommendation,
                    now,
                    events,
                    RecentSequencePolicy {
                        max_actions: usize::from(
                            RecentActionLimit::new(self.sequence_config.limit).get(),
                        ),
                        ..RecentSequencePolicy::default()
                    },
                ) {
                    Ok(sequence) => (sequence, false),
                    Err(error) => {
                        tracing::warn!(error = ?error, %viewer_id, "invalid people recommendation sequence; ignoring recent actions");
                        (RecentSequence::default(), true)
                    }
                },
                Err(error) => {
                    tracing::warn!(error = ?error, %viewer_id, "people recommendation sequence lookup failed; using projection features");
                    (RecentSequence::default(), true)
                }
            }
        } else {
            (RecentSequence::default(), false)
        };
        if sequence_fallback {
            self.observer
                .record_fallback(PeopleRecommendationFallbackReason::SequenceUnavailable);
        }
        candidates.retain(|candidate| {
            !ineligible.contains(&candidate.candidate_user_id)
                && profiles
                    .get(&candidate.candidate_user_id)
                    .is_some_and(ProfileDiscoverySnapshot::is_discoverable)
        });
        for candidate in &mut candidates {
            if let (Some(viewer), Some(profile)) =
                (viewer_profile, profiles.get(&candidate.candidate_user_id))
            {
                candidate.features.profile_overlap = viewer.overlap_score(profile);
                candidate.features.candidate_quality = profile.quality_score();
            }
            candidate.features.impression_penalty = candidate
                .features
                .impression_penalty
                .max(recent_sequence.repeat_penalty(candidate.candidate_user_id));
            candidate.features.dismissal_penalty = candidate
                .features
                .dismissal_penalty
                .max(recent_sequence.negative_penalty(candidate.candidate_user_id));
        }

        let mut ranked = self.ranker.rank(candidates)?;
        ranked.truncate(MAX_FINAL_CANDIDATES);
        let snapshot = PeopleSnapshot {
            id: Uuid::new_v4(),
            viewer_id,
            rank_version: self.ranker.version().to_owned(),
            created_at: now,
            expires_at: now + Duration::minutes(SNAPSHOT_TTL_MINUTES),
        };
        let items = ranked
            .into_iter()
            .map(|candidate| PeopleSnapshotItem {
                snapshot_id: snapshot.id,
                candidate_user_id: candidate.candidate_user_id,
                source: candidate.source,
                reason_code: candidate.reason_code,
                score_micros: candidate.score_micros,
                rank_version: candidate.model_version,
                feature_snapshot_json: serde_json::to_string(&candidate.features.components())
                    .unwrap_or_else(|_| "{}".into()),
                mutual_cluster: candidate.mutual_cluster,
                generated_at: candidate.generated_at,
            })
            .collect::<Vec<_>>();
        self.repository.create_snapshot(&snapshot, &items).await?;
        Ok(snapshot)
    }

    async fn relationship_ineligible(
        &self,
        viewer_id: Uuid,
        candidate_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>> {
        let mut ineligible = HashSet::new();
        let mut seen = HashSet::with_capacity(candidate_ids.len());
        let unique_candidate_ids = candidate_ids
            .iter()
            .copied()
            .filter(|candidate_id| seen.insert(*candidate_id))
            .collect::<Vec<_>>();
        for batch in unique_candidate_ids.chunks(BATCH_SIZE) {
            let policies = self.relationships.batch_policy(viewer_id, batch).await?;
            if batch
                .iter()
                .any(|target_id| !policies.contains_key(target_id))
                || policies.keys().any(|target_id| !batch.contains(target_id))
            {
                anyhow::bail!("Incomplete relationship policy batch");
            }
            for target_id in batch {
                let policy = policies
                    .get(target_id)
                    .ok_or_else(|| anyhow::anyhow!("Incomplete relationship policy batch"))?;
                if policy.blocked_by_viewer
                    || policy.blocked_viewer
                    || policy.muted_by_viewer
                    || policy.following
                {
                    ineligible.insert(*target_id);
                }
            }
        }
        Ok(ineligible)
    }

    async fn fetch_profiles(
        &self,
        user_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, ProfileDiscoverySnapshot>> {
        let mut output = HashMap::new();
        let mut deduplicated = Vec::new();
        let mut seen = HashSet::new();
        for user_id in user_ids {
            if seen.insert(*user_id) {
                deduplicated.push(*user_id);
            }
        }
        for batch in deduplicated.chunks(BATCH_SIZE) {
            for profile in self.profiles.batch_profiles(batch).await? {
                output.insert(profile.user_id, profile);
            }
        }
        Ok(output)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServingSelection {
    fallback_used: bool,
}

fn select_serving_ranker(
    rollout: &RecommendationRollout,
    deterministic_version: &str,
) -> ServingSelection {
    let configured_deterministic_matches = rollout.deterministic_version == deterministic_version;
    let advanced_requested = matches!(rollout.mode, RolloutMode::Canary | RolloutMode::Learned);
    let learned_contract_ready = rollout.learned_ready && rollout.learned_version.is_some();
    if advanced_requested {
        tracing::warn!(
            mode = rollout.mode.as_str(),
            learned_ready = learned_contract_ready,
            canary_percent = rollout.canary_percent,
            "learned people ranker is not configured; using deterministic fallback"
        );
    }
    ServingSelection {
        fallback_used: advanced_requested || !configured_deterministic_matches,
    }
}

fn normalize_limit(limit: i32) -> i32 {
    if limit <= 0 {
        DEFAULT_LIMIT
    } else {
        limit.min(MAX_LIMIT)
    }
}

fn validate_query(query: &GetPeopleRecommendationsQuery) -> Result<(), FriendError> {
    if query.viewer_id.is_nil() {
        return Err(FriendError::InvalidInput("Viewer ID is required".into()));
    }
    if query.request_id.len() > 128 || query.request_id.trim().is_empty() {
        return Err(FriendError::InvalidInput(
            "Recommendation request ID is invalid".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_query, GetPeopleRecommendationsQuery};
    use uuid::Uuid;

    #[test]
    fn query_rejects_oversized_whitespace_request_id_before_normalization() {
        let query = GetPeopleRecommendationsQuery {
            viewer_id: Uuid::new_v4(),
            cursor: None,
            limit: 20,
            request_id: " ".repeat(129),
            session_id: None,
        };

        assert!(validate_query(&query).is_err());
    }
}
