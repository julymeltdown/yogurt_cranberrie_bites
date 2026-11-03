-- One bounded observation, not a claim, lock or authorization decision.
WITH observed AS MATERIALIZED (
    SELECT clock_timestamp() AS observed_at
), targets(target) AS (
    VALUES ('TIMELINE'), ('CACHE'), ('INTERNAL'), ('EXTERNAL')
), candidates AS MATERIALIZED (
    SELECT event.event_id, event.event_type, event.aggregate_id,
           event.aggregate_version, event.created_at, event.attempt_count,
           event.dead_lettered_at, event.delivery_required_targets,
           event.locked_at, observed.observed_at
    FROM post_event_outbox AS event
    CROSS JOIN observed
    WHERE NOT event.published
      AND (
          event.dead_lettered_at IS NOT NULL
          OR EXISTS (
              SELECT 1 FROM post_event_delivery_jobs AS job
              WHERE job.event_id = event.event_id
                AND (event.delivery_required_targets IS NULL
                     OR job.target = ANY(event.delivery_required_targets))
                AND NOT EXISTS (
                    SELECT 1 FROM event_publications AS publication
                    WHERE publication.event_id = job.event_id
                      AND publication.target = job.target
                )
                AND (job.state = 'DEAD'
                     OR (job.state = 'LEASED'
                         AND job.lease_expires_at <= observed.observed_at
                         AND job.attempt_count-job.progress_credits-COALESCE(post_timeline_pending_progress_credit(job.event_id,job.attempt_id),0) >= job.max_attempts))
          )
          OR EXISTS (
              SELECT 1 FROM unnest(event.delivery_required_targets) AS required(target)
              WHERE NOT EXISTS (
                  SELECT 1 FROM post_event_delivery_jobs AS job
                  WHERE job.event_id = event.event_id AND job.target = required.target
              )
          )
      )
    ORDER BY event.created_at, event.event_id
    LIMIT 100
)
SELECT
    event.event_id,
    event.event_type,
    event.aggregate_id,
    event.aggregate_version,
    event.created_at,
    event.observed_at,
    event.attempt_count AS legacy_parent_attempt_count,
    event.dead_lettered_at AS parent_dead_letter_summary,
    event.delivery_required_targets,
    ordering.unpublished_predecessors,
    ordering.unpublished_successors,
    CASE
        WHEN event.locked_at IS NOT NULL THEN 'LEGACY_PARENT_LOCK_PRESENT'
        WHEN jobs.job_count <> 4 THEN 'JOB_SET_INCOMPLETE'
        WHEN jobs.live_lease THEN 'LIVE_TARGET_LEASE'
        WHEN jobs.eligible AND (event.delivery_required_targets IS NOT NULL
                               OR event.dead_lettered_at IS NOT NULL)
            THEN 'OBSERVED_ELIGIBLE_FOR_AUDITED_RECOVERY'
        WHEN event.delivery_required_targets IS NULL
             AND event.dead_lettered_at IS NULL THEN 'LEGACY_SUMMARY_MISSING'
        ELSE 'NO_ELIGIBLE_UNFINISHED_JOB'
    END AS recovery_observation,
    jobs.target_observations
FROM candidates AS event
CROSS JOIN LATERAL (
    SELECT
        count(job.event_id) AS job_count,
        bool_or(COALESCE(job.state = 'LEASED'
                         AND job.lease_expires_at > event.observed_at, false)) AS live_lease,
        bool_or(
            job.event_id IS NOT NULL
            AND (event.delivery_required_targets IS NULL
                 OR targets.target = ANY(event.delivery_required_targets))
            AND publication.event_id IS NULL
            AND (job.state = 'DEAD'
                 OR (job.state = 'LEASED'
                     AND job.lease_expires_at <= event.observed_at
                     AND job.attempt_count-job.progress_credits-COALESCE(post_timeline_pending_progress_credit(job.event_id,job.attempt_id),0) >= job.max_attempts)
                 OR (event.delivery_required_targets IS NULL
                     AND job.state IN ('PENDING', 'RETRY')))
        ) AS eligible,
        jsonb_agg(jsonb_build_object(
            'target', targets.target,
            'required', CASE WHEN event.delivery_required_targets IS NULL THEN NULL
                             ELSE targets.target = ANY(event.delivery_required_targets) END,
            'state', COALESCE(job.state, 'MISSING'),
            'checkpoint', publication.event_id IS NOT NULL,
            'attemptCount', job.attempt_count,
            'progressCredits', job.progress_credits,
            'failureBudgetSpent', job.attempt_count-job.progress_credits,
            'pendingProgressCredit', CASE WHEN job.target='TIMELINE' AND job.state='LEASED' AND job.lease_expires_at<=event.observed_at THEN COALESCE(post_timeline_pending_progress_credit(job.event_id,job.attempt_id),0) ELSE 0 END,
            'maxAttempts', job.max_attempts,
            'nextAttemptAt', job.next_attempt_at,
            'leaseKind', job.lease_kind,
            'replayRestoreState', job.replay_restore_state,
            'leaseObservation', CASE WHEN job.state <> 'LEASED' OR job.state IS NULL THEN 'NONE'
                                     WHEN job.lease_expires_at > event.observed_at THEN 'LIVE'
                                     ELSE 'EXPIRED' END,
            'releaseResolutionId', job.release_resolution_id
        ) ORDER BY targets.target) AS target_observations
    FROM targets
    LEFT JOIN post_event_delivery_jobs AS job
        ON job.event_id = event.event_id AND job.target = targets.target
    LEFT JOIN event_publications AS publication
        ON publication.event_id = event.event_id AND publication.target = targets.target
) AS jobs
CROSS JOIN LATERAL (
    SELECT
        count(*) FILTER (WHERE
            (event.aggregate_version IS NULL AND other.aggregate_version IS NULL
             AND (other.created_at, other.event_id) < (event.created_at, event.event_id))
            OR (event.aggregate_version IS NOT NULL
                AND (other.aggregate_version IS NULL
                     OR other.aggregate_version < event.aggregate_version))
        ) AS unpublished_predecessors,
        count(*) FILTER (WHERE
            (event.aggregate_version IS NULL
             AND (other.aggregate_version IS NOT NULL
                  OR (other.aggregate_version IS NULL
                      AND (other.created_at, other.event_id) > (event.created_at, event.event_id))))
            OR (event.aggregate_version IS NOT NULL
                AND other.aggregate_version > event.aggregate_version)
        ) AS unpublished_successors
    FROM post_event_outbox AS other
    WHERE other.aggregate_id = event.aggregate_id AND NOT other.published
) AS ordering
ORDER BY event.created_at, event.event_id;
