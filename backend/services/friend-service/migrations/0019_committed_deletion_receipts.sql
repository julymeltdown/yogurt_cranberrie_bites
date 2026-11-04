-- BE42 owner-local version 1. No historical backfill, no new retention policy.
-- Current-source provenance and qualification are recorded in the owner document.
CREATE FUNCTION friend_deletion_clean_v1(u uuid) RETURNS boolean LANGUAGE sql AS $$
 select not exists(select 1 from follows where (follower_id=u or followee_id=u) and deleted_at is null)
 and not exists(select 1 from follow_requests where (follower_id=u or followee_id=u) and (deleted_at is null or status='PENDING'))
 and not exists(select 1 from user_blocks where (blocker_id=u or blocked_id=u) and deleted_at is null)
 and not exists(select 1 from user_mutes where (muter_id=u or muted_id=u) and deleted_at is null)
 and not exists(select 1 from people_recommendation_candidates where viewer_id=u or candidate_user_id=u)
 and not exists(select 1 from people_recommendation_impressions where viewer_id=u or candidate_user_id=u)
 and not exists(select 1 from people_recommendation_snapshot_items where candidate_user_id=u)
 and not exists(select 1 from people_recommendation_snapshots where viewer_id=u)
 and not exists(select 1 from people_recommendation_events where viewer_id=u or actor_id=u or subject_id=u)
 and not exists(select 1 from people_recommendation_refresh_jobs where viewer_id=u)
 and not exists(select 1 from people_experiment_assignments where viewer_id=u)
 and not exists(select 1 from people_embeddings where subject_id=u);
$$;
CREATE TABLE friend_deletion_fences_v1 (
 user_id uuid PRIMARY KEY REFERENCES friend_user_deletion_tombstones(user_id),
 fence_revision bigint GENERATED ALWAYS AS IDENTITY UNIQUE CHECK(fence_revision>0),
 established_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
 recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(user_id,fence_revision)
);
CREATE TABLE friend_deletion_receipts_v1 (
 saga_id uuid PRIMARY KEY CHECK(saga_id<>'00000000-0000-0000-0000-000000000000'),
 user_id uuid NOT NULL UNIQUE CHECK(user_id<>'00000000-0000-0000-0000-000000000000'),
 actor_id uuid NOT NULL CHECK(actor_id<>'00000000-0000-0000-0000-000000000000'),
 retention_until timestamptz NOT NULL CHECK(isfinite(retention_until)),
 retention_seconds bigint NOT NULL,
 retention_nanos integer NOT NULL CHECK(retention_nanos BETWEEN 0 AND 999999999),
 fence_revision bigint NOT NULL CHECK(fence_revision>0),
 audit_event_id uuid NOT NULL UNIQUE REFERENCES friend_audit_events(id) CHECK(audit_event_id=saga_id),
 effect_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
 recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 FOREIGN KEY(user_id,fence_revision) REFERENCES friend_deletion_fences_v1(user_id,fence_revision)
);
CREATE FUNCTION friend_deletion_immutable_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'friend deletion proof is immutable' USING ERRCODE='23514'; END $$;
CREATE TRIGGER friend_deletion_fence_immutable_v1 BEFORE UPDATE OR DELETE OR TRUNCATE ON friend_deletion_fences_v1
 FOR EACH STATEMENT EXECUTE FUNCTION friend_deletion_immutable_v1();
CREATE TRIGGER friend_deletion_receipt_immutable_v1 BEFORE UPDATE OR DELETE OR TRUNCATE ON friend_deletion_receipts_v1
 FOR EACH STATEMENT EXECUTE FUNCTION friend_deletion_immutable_v1();
CREATE FUNCTION friend_deletion_link_guard_v1() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE linked boolean;
BEGIN
 IF TG_OP='TRUNCATE' THEN linked:=EXISTS(select 1 from friend_deletion_receipts_v1);
 ELSIF TG_TABLE_NAME='friend_audit_events' THEN linked:=EXISTS(select 1 from friend_deletion_receipts_v1 where audit_event_id=OLD.id);
 ELSE linked:=EXISTS(select 1 from friend_deletion_receipts_v1 where user_id=OLD.user_id or saga_id=OLD.saga_id);
 END IF;
 IF linked THEN RAISE EXCEPTION 'friend linked deletion evidence is immutable' USING ERRCODE='23514'; END IF;
 IF TG_OP='TRUNCATE' THEN RETURN NULL; END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER friend_deletion_legacy_guard_v1 BEFORE UPDATE OR DELETE ON friend_user_deletion_tombstones
 FOR EACH ROW EXECUTE FUNCTION friend_deletion_link_guard_v1();
CREATE TRIGGER friend_deletion_legacy_truncate_v1 BEFORE TRUNCATE ON friend_user_deletion_tombstones
 FOR EACH STATEMENT EXECUTE FUNCTION friend_deletion_link_guard_v1();
CREATE TRIGGER friend_deletion_audit_guard_v1 BEFORE UPDATE OR DELETE ON friend_audit_events
 FOR EACH ROW EXECUTE FUNCTION friend_deletion_link_guard_v1();
CREATE TRIGGER friend_deletion_audit_truncate_v1 BEFORE TRUNCATE ON friend_audit_events
 FOR EACH STATEMENT EXECUTE FUNCTION friend_deletion_link_guard_v1();
CREATE FUNCTION friend_deletion_commit_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_TABLE_NAME='friend_deletion_fences_v1' THEN
   IF NEW.established_xid IS DISTINCT FROM pg_current_xact_id() OR NOT EXISTS(
      select 1 from friend_deletion_receipts_v1 r where r.user_id=NEW.user_id and r.fence_revision=NEW.fence_revision
        and r.effect_xid=pg_current_xact_id()) THEN
      RAISE EXCEPTION 'friend fence lacks same-transaction receipt' USING ERRCODE='23514';
   END IF;
 ELSE
   IF NEW.effect_xid IS DISTINCT FROM pg_current_xact_id()
    OR NEW.retention_until IS DISTINCT FROM (timestamptz '2000-01-01 00:00:00+00' +
        trunc(((NEW.retention_seconds-946684800)::numeric*1000000000+NEW.retention_nanos)/1000)*interval '1 microsecond')
    OR NOT friend_deletion_clean_v1(NEW.user_id)
    OR NOT EXISTS(select 1 from friend_deletion_fences_v1 f where f.user_id=NEW.user_id and f.fence_revision=NEW.fence_revision and f.established_xid=pg_current_xact_id())
    OR NOT EXISTS(select 1 from friend_user_deletion_tombstones t where t.user_id=NEW.user_id and t.saga_id=NEW.saga_id and t.actor_id=NEW.actor_id and t.retention_until=NEW.retention_until
       and t.xmin::text::bigint=mod(pg_current_xact_id()::text::numeric,4294967296)::bigint)
    OR NOT EXISTS(select 1 from friend_audit_events a where a.id=NEW.saga_id and a.actor_id=NEW.actor_id and a.follower_id=NEW.user_id and a.followee_id is null
       and a.action='ACCOUNT_DELETION_APPLIED' and a.outcome='SUCCESS'
       and a.xmin::text::bigint=mod(pg_current_xact_id()::text::numeric,4294967296)::bigint
       and a.metadata_json::jsonb @> jsonb_build_object('proofVersion',1,'sagaId',NEW.saga_id,'retentionSeconds',NEW.retention_seconds,'retentionNanos',NEW.retention_nanos,'fenceRevision',NEW.fence_revision)) THEN
      RAISE EXCEPTION 'friend receipt lacks actual cleanup transaction proof' USING ERRCODE='23514';
   END IF;
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER friend_deletion_fence_commit_v1 AFTER INSERT ON friend_deletion_fences_v1
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION friend_deletion_commit_v1();
CREATE CONSTRAINT TRIGGER friend_deletion_receipt_commit_v1 AFTER INSERT ON friend_deletion_receipts_v1
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION friend_deletion_commit_v1();
