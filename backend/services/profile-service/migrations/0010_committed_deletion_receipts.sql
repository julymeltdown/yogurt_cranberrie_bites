-- BE42 owner-local version 1. No historical backfill, no new retention policy.
-- Current-source provenance and qualification are recorded in the owner document.
CREATE FUNCTION profile_deletion_clean_v1(u uuid) RETURNS boolean LANGUAGE sql AS $$
 select not exists(select 1 from profiles where user_id=u and (deleted_at is null or username <> 'deleted-'||u::text or nickname<>'' or avatar_url<>'' or bio<>'' or follower_count<>0 or following_count<>0 or post_count<>0))
 and not exists(select 1 from avatars where user_id=u and (deleted_at is null or octet_length(content)<>0 or filename<>''))
 and not exists(select 1 from profile_mutation_receipts where actor_id=u and outcome_json is not null);
$$;
CREATE TABLE profile_deletion_fences_v1 (
 user_id uuid PRIMARY KEY REFERENCES profile_user_deletion_tombstones(user_id),
 fence_revision bigint GENERATED ALWAYS AS IDENTITY UNIQUE CHECK(fence_revision>0),
 established_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
 recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(user_id,fence_revision)
);
CREATE TABLE profile_deletion_receipts_v1 (
 saga_id uuid PRIMARY KEY CHECK(saga_id<>'00000000-0000-0000-0000-000000000000'),
 user_id uuid NOT NULL UNIQUE CHECK(user_id<>'00000000-0000-0000-0000-000000000000'),
 actor_id uuid NOT NULL CHECK(actor_id<>'00000000-0000-0000-0000-000000000000'),
 retention_until timestamptz NOT NULL CHECK(isfinite(retention_until)),
 retention_seconds bigint NOT NULL,
 retention_nanos integer NOT NULL CHECK(retention_nanos BETWEEN 0 AND 999999999),
 fence_revision bigint NOT NULL CHECK(fence_revision>0),
 audit_event_id uuid NOT NULL UNIQUE REFERENCES profile_audit_events(id) CHECK(audit_event_id=saga_id),
 effect_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
 recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 FOREIGN KEY(user_id,fence_revision) REFERENCES profile_deletion_fences_v1(user_id,fence_revision)
);
CREATE FUNCTION profile_deletion_immutable_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'profile deletion proof is immutable' USING ERRCODE='23514'; END $$;
CREATE TRIGGER profile_deletion_fence_immutable_v1 BEFORE UPDATE OR DELETE OR TRUNCATE ON profile_deletion_fences_v1
 FOR EACH STATEMENT EXECUTE FUNCTION profile_deletion_immutable_v1();
CREATE TRIGGER profile_deletion_receipt_immutable_v1 BEFORE UPDATE OR DELETE OR TRUNCATE ON profile_deletion_receipts_v1
 FOR EACH STATEMENT EXECUTE FUNCTION profile_deletion_immutable_v1();
CREATE FUNCTION profile_deletion_link_guard_v1() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE linked boolean;
BEGIN
 IF TG_OP='TRUNCATE' THEN linked:=EXISTS(select 1 from profile_deletion_receipts_v1);
 ELSIF TG_TABLE_NAME='profile_audit_events' THEN linked:=EXISTS(select 1 from profile_deletion_receipts_v1 where audit_event_id=OLD.id);
 ELSE linked:=EXISTS(select 1 from profile_deletion_receipts_v1 where user_id=OLD.user_id or saga_id=OLD.saga_id);
 END IF;
 IF linked THEN RAISE EXCEPTION 'profile linked deletion evidence is immutable' USING ERRCODE='23514'; END IF;
 IF TG_OP='TRUNCATE' THEN RETURN NULL; END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profile_deletion_legacy_guard_v1 BEFORE UPDATE OR DELETE ON profile_user_deletion_tombstones
 FOR EACH ROW EXECUTE FUNCTION profile_deletion_link_guard_v1();
CREATE TRIGGER profile_deletion_legacy_truncate_v1 BEFORE TRUNCATE ON profile_user_deletion_tombstones
 FOR EACH STATEMENT EXECUTE FUNCTION profile_deletion_link_guard_v1();
CREATE TRIGGER profile_deletion_audit_guard_v1 BEFORE UPDATE OR DELETE ON profile_audit_events
 FOR EACH ROW EXECUTE FUNCTION profile_deletion_link_guard_v1();
CREATE TRIGGER profile_deletion_audit_truncate_v1 BEFORE TRUNCATE ON profile_audit_events
 FOR EACH STATEMENT EXECUTE FUNCTION profile_deletion_link_guard_v1();
CREATE FUNCTION profile_deletion_commit_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_TABLE_NAME='profile_deletion_fences_v1' THEN
   IF NEW.established_xid IS DISTINCT FROM pg_current_xact_id() OR NOT EXISTS(
      select 1 from profile_deletion_receipts_v1 r where r.user_id=NEW.user_id and r.fence_revision=NEW.fence_revision
        and r.effect_xid=pg_current_xact_id()) THEN
      RAISE EXCEPTION 'profile fence lacks same-transaction receipt' USING ERRCODE='23514';
   END IF;
 ELSE
   IF NEW.effect_xid IS DISTINCT FROM pg_current_xact_id()
    OR NEW.retention_until IS DISTINCT FROM (timestamptz '2000-01-01 00:00:00+00' +
        trunc(((NEW.retention_seconds-946684800)::numeric*1000000000+NEW.retention_nanos)/1000)*interval '1 microsecond')
    OR NOT profile_deletion_clean_v1(NEW.user_id)
    OR NOT EXISTS(select 1 from profile_deletion_fences_v1 f where f.user_id=NEW.user_id and f.fence_revision=NEW.fence_revision and f.established_xid=pg_current_xact_id())
    OR NOT EXISTS(select 1 from profile_user_deletion_tombstones t where t.user_id=NEW.user_id and t.saga_id=NEW.saga_id and t.actor_id=NEW.actor_id and t.retention_until=NEW.retention_until
       and t.xmin::text::bigint=mod(pg_current_xact_id()::text::numeric,4294967296)::bigint)
    OR NOT EXISTS(select 1 from profile_audit_events a where a.id=NEW.saga_id and a.actor_id=NEW.actor_id and a.profile_user_id=NEW.user_id
       and a.action='ACCOUNT_DELETION_APPLIED' and a.outcome='SUCCESS'
       and a.xmin::text::bigint=mod(pg_current_xact_id()::text::numeric,4294967296)::bigint
       and a.metadata_json::jsonb @> jsonb_build_object('proofVersion',1,'sagaId',NEW.saga_id,'retentionSeconds',NEW.retention_seconds,'retentionNanos',NEW.retention_nanos,'fenceRevision',NEW.fence_revision)) THEN
      RAISE EXCEPTION 'profile receipt lacks actual cleanup transaction proof' USING ERRCODE='23514';
   END IF;
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER profile_deletion_fence_commit_v1 AFTER INSERT ON profile_deletion_fences_v1
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION profile_deletion_commit_v1();
CREATE CONSTRAINT TRIGGER profile_deletion_receipt_commit_v1 AFTER INSERT ON profile_deletion_receipts_v1
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION profile_deletion_commit_v1();
-- Retained replay rows may keep metadata, never deleted actors' bodies.
CREATE FUNCTION profile_deletion_replay_guard_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.outcome_json IS NOT NULL THEN PERFORM profile_assert_subjects_not_deleted(array[NEW.actor_id]); END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profile_deletion_replay_guard_v1 BEFORE INSERT OR UPDATE ON profile_mutation_receipts
 FOR EACH ROW EXECUTE FUNCTION profile_deletion_replay_guard_v1();
-- Soft-deleted rows cannot be used as a raw-body resurrection loophole.
CREATE FUNCTION profile_deletion_body_guard_v1() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE denied boolean;
BEGIN
 PERFORM profile_lock_subject_ids(array[NEW.user_id]);
 IF TG_OP='UPDATE' AND OLD.user_id<>NEW.user_id AND EXISTS(select 1 from profile_user_deletion_tombstones where user_id=OLD.user_id) THEN
   RAISE EXCEPTION 'Deleted profile identity cannot be transplanted' USING ERRCODE='23514';
 END IF;
 denied:=EXISTS(select 1 from profile_user_deletion_tombstones where user_id=NEW.user_id);
 IF denied THEN
   IF NEW.deleted_at IS NULL THEN RAISE EXCEPTION 'Deleted profile cannot reactivate' USING ERRCODE='23514'; END IF;
   IF TG_TABLE_NAME='profiles' THEN
     IF NEW.username<>'deleted-'||NEW.user_id::text OR NEW.nickname<>'' OR NEW.avatar_url<>'' OR NEW.bio<>'' OR NEW.follower_count<>0 OR NEW.following_count<>0 OR NEW.post_count<>0 THEN
       RAISE EXCEPTION 'Deleted profile body cannot be restored' USING ERRCODE='23514';
     END IF;
   ELSE
     IF octet_length(NEW.content)<>0 OR NEW.filename<>'' THEN RAISE EXCEPTION 'Deleted avatar cannot be restored' USING ERRCODE='23514'; END IF;
   END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profile_deletion_body_guard_v1 BEFORE INSERT OR UPDATE ON profiles FOR EACH ROW EXECUTE FUNCTION profile_deletion_body_guard_v1();
CREATE TRIGGER avatar_deletion_body_guard_v1 BEFORE INSERT OR UPDATE ON avatars FOR EACH ROW EXECUTE FUNCTION profile_deletion_body_guard_v1();
