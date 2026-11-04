-- BE42 owner-local version 1. No historical backfill, no new retention policy.
-- Current-source provenance and qualification are recorded in the owner document.
CREATE FUNCTION post_deletion_clean_v1(u uuid) RETURNS boolean LANGUAGE sql AS $$
 select not exists(select 1 from posts where author_id=u and (deleted_at is null or content<>''))
 and not exists(select 1 from comments where author_id=u and (deleted_at is null or content<>''))
 and not exists(select 1 from post_likes where user_id=u and deleted_at is null)
 and not exists(select 1 from post_mutation_receipts where actor_id=u and outcome_json is not null)
 and not exists(select 1 from post_event_outbox where actor_id=u and payload::jsonb ? 'content')
 and not exists(select 1 from event_publications p join post_event_outbox e on e.event_id=p.event_id where e.actor_id=u and p.payload::jsonb ? 'content')
 and not exists(select 1 from post_concert_posts where author_id=u and (state<>'DELETED' or content<>'{"text":"[deleted]","media_ids":[],"links":[]}'::jsonb))
 and not exists(select 1 from post_concert_comments where author_id=u and (state<>'DELETED' or text<>'[deleted]'))
 and not exists(select 1 from post_concert_reactions where actor_id=u)
 and not exists(select 1 from feed_recommendation_impressions where viewer_id=u or author_id=u)
 and not exists(select 1 from feed_recommendation_snapshot_items where author_id=u or post_id in(select id from posts where author_id=u))
 and not exists(select 1 from feed_recommendation_snapshots where viewer_id=u)
 and not exists(select 1 from feed_recommendation_events where viewer_id=u or actor_id=u or (subject_type='USER' and subject_id=u) or (subject_type='POST' and subject_id in(select id from posts where author_id=u)) or (subject_type='COMMENT' and subject_id in(select id from comments where author_id=u)))
 and not exists(select 1 from feed_author_affinities where viewer_id=u or author_id=u)
 and not exists(select 1 from feed_recent_actions where viewer_id=u or post_id in(select id from posts where author_id=u))
 and not exists(select 1 from feed_experiment_assignments where viewer_id=u)
 and not exists(select 1 from feed_embeddings where subject_id=u or subject_id in(select id from posts where author_id=u))
 and not exists(select 1 from post_engagement_velocity where post_id in(select id from posts where author_id=u))
 and not exists(select 1 from feed_fanout_jobs where author_id=u)
 and not exists(select 1 from home_timeline_entries where viewer_id=u or author_id=u)
 and not exists(select 1 from high_fanout_posts where author_id=u)
 and not exists(select 1 from feed_snapshots where viewer_id=u)
 and not exists(select 1 from feed_snapshot_items where author_id=u or post_id in(select id from posts where author_id=u))
 and not exists(select 1 from feed_impressions where viewer_id=u or author_id=u or post_id in(select id from posts where author_id=u))
 and not exists(select 1 from feed_author_affinity where viewer_id=u or author_id=u);
$$;
CREATE TABLE post_deletion_fences_v1 (
 user_id uuid PRIMARY KEY REFERENCES post_user_deletion_tombstones(user_id),
 fence_revision bigint GENERATED ALWAYS AS IDENTITY UNIQUE CHECK(fence_revision>0),
 established_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
 recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(user_id,fence_revision)
);
CREATE TABLE post_deletion_receipts_v1 (
 saga_id uuid PRIMARY KEY CHECK(saga_id<>'00000000-0000-0000-0000-000000000000'),
 user_id uuid NOT NULL UNIQUE CHECK(user_id<>'00000000-0000-0000-0000-000000000000'),
 actor_id uuid NOT NULL CHECK(actor_id<>'00000000-0000-0000-0000-000000000000'),
 retention_until timestamptz NOT NULL CHECK(isfinite(retention_until)),
 retention_seconds bigint NOT NULL,
 retention_nanos integer NOT NULL CHECK(retention_nanos BETWEEN 0 AND 999999999),
 fence_revision bigint NOT NULL CHECK(fence_revision>0),
 audit_event_id uuid NOT NULL UNIQUE REFERENCES post_audit_events(id) CHECK(audit_event_id=saga_id),
 effect_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
 recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 FOREIGN KEY(user_id,fence_revision) REFERENCES post_deletion_fences_v1(user_id,fence_revision)
);
CREATE FUNCTION post_deletion_immutable_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'post deletion proof is immutable' USING ERRCODE='23514'; END $$;
CREATE TRIGGER post_deletion_fence_immutable_v1 BEFORE UPDATE OR DELETE OR TRUNCATE ON post_deletion_fences_v1
 FOR EACH STATEMENT EXECUTE FUNCTION post_deletion_immutable_v1();
CREATE TRIGGER post_deletion_receipt_immutable_v1 BEFORE UPDATE OR DELETE OR TRUNCATE ON post_deletion_receipts_v1
 FOR EACH STATEMENT EXECUTE FUNCTION post_deletion_immutable_v1();
CREATE FUNCTION post_deletion_link_guard_v1() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE linked boolean;
BEGIN
 IF TG_OP='TRUNCATE' THEN linked:=EXISTS(select 1 from post_deletion_receipts_v1);
 ELSIF TG_TABLE_NAME='post_audit_events' THEN linked:=EXISTS(select 1 from post_deletion_receipts_v1 where audit_event_id=OLD.id);
 ELSE linked:=EXISTS(select 1 from post_deletion_receipts_v1 where user_id=OLD.user_id or saga_id=OLD.saga_id);
 END IF;
 IF linked THEN RAISE EXCEPTION 'post linked deletion evidence is immutable' USING ERRCODE='23514'; END IF;
 IF TG_OP='TRUNCATE' THEN RETURN NULL; END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER post_deletion_legacy_guard_v1 BEFORE UPDATE OR DELETE ON post_user_deletion_tombstones
 FOR EACH ROW EXECUTE FUNCTION post_deletion_link_guard_v1();
CREATE TRIGGER post_deletion_legacy_truncate_v1 BEFORE TRUNCATE ON post_user_deletion_tombstones
 FOR EACH STATEMENT EXECUTE FUNCTION post_deletion_link_guard_v1();
CREATE TRIGGER post_deletion_audit_guard_v1 BEFORE UPDATE OR DELETE ON post_audit_events
 FOR EACH ROW EXECUTE FUNCTION post_deletion_link_guard_v1();
CREATE TRIGGER post_deletion_audit_truncate_v1 BEFORE TRUNCATE ON post_audit_events
 FOR EACH STATEMENT EXECUTE FUNCTION post_deletion_link_guard_v1();
CREATE FUNCTION post_deletion_commit_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_TABLE_NAME='post_deletion_fences_v1' THEN
   IF NEW.established_xid IS DISTINCT FROM pg_current_xact_id() OR NOT EXISTS(
      select 1 from post_deletion_receipts_v1 r where r.user_id=NEW.user_id and r.fence_revision=NEW.fence_revision
        and r.effect_xid=pg_current_xact_id()) THEN
      RAISE EXCEPTION 'post fence lacks same-transaction receipt' USING ERRCODE='23514';
   END IF;
 ELSE
   IF NEW.effect_xid IS DISTINCT FROM pg_current_xact_id()
    OR NEW.retention_until IS DISTINCT FROM (timestamptz '2000-01-01 00:00:00+00' +
        trunc(((NEW.retention_seconds-946684800)::numeric*1000000000+NEW.retention_nanos)/1000)*interval '1 microsecond')
    OR NOT post_deletion_clean_v1(NEW.user_id)
    OR NOT EXISTS(select 1 from post_deletion_fences_v1 f where f.user_id=NEW.user_id and f.fence_revision=NEW.fence_revision and f.established_xid=pg_current_xact_id())
    OR NOT EXISTS(select 1 from post_user_deletion_tombstones t where t.user_id=NEW.user_id and t.saga_id=NEW.saga_id and t.actor_id=NEW.actor_id and t.retention_until=NEW.retention_until
       and t.xmin::text::bigint=mod(pg_current_xact_id()::text::numeric,4294967296)::bigint)
    OR NOT EXISTS(select 1 from post_audit_events a where a.id=NEW.saga_id and a.actor_id=NEW.actor_id and a.post_id is null and a.comment_id is null
       and a.action='ACCOUNT_DELETION_APPLIED' and a.outcome='SUCCESS'
       and a.xmin::text::bigint=mod(pg_current_xact_id()::text::numeric,4294967296)::bigint
       and a.metadata_json::jsonb @> jsonb_build_object('proofVersion',1,'sagaId',NEW.saga_id,'retentionSeconds',NEW.retention_seconds,'retentionNanos',NEW.retention_nanos,'fenceRevision',NEW.fence_revision)) THEN
      RAISE EXCEPTION 'post receipt lacks actual cleanup transaction proof' USING ERRCODE='23514';
   END IF;
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER post_deletion_fence_commit_v1 AFTER INSERT ON post_deletion_fences_v1
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION post_deletion_commit_v1();
CREATE CONSTRAINT TRIGGER post_deletion_receipt_commit_v1 AFTER INSERT ON post_deletion_receipts_v1
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION post_deletion_commit_v1();
-- Retained replay rows may keep metadata, never deleted actors' bodies.
CREATE FUNCTION post_deletion_replay_guard_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.outcome_json IS NOT NULL THEN PERFORM post_assert_subjects_not_deleted(array[NEW.actor_id]); END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER post_deletion_replay_guard_v1 BEFORE INSERT OR UPDATE ON post_mutation_receipts
 FOR EACH ROW EXECUTE FUNCTION post_deletion_replay_guard_v1();
CREATE FUNCTION post_deletion_body_guard_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 PERFORM post_lock_subject_ids(array[NEW.author_id]);
 IF TG_OP='UPDATE' AND OLD.author_id<>NEW.author_id AND EXISTS(select 1 from post_user_deletion_tombstones where user_id=OLD.author_id) THEN
   RAISE EXCEPTION 'Deleted content identity cannot be transplanted' USING ERRCODE='23514';
 END IF;
 IF EXISTS(select 1 from post_user_deletion_tombstones where user_id=NEW.author_id)
    AND (NEW.deleted_at IS NULL OR NEW.content<>'') THEN
   RAISE EXCEPTION 'Deleted content cannot be restored' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER post_deletion_body_guard_v1 BEFORE INSERT OR UPDATE ON posts FOR EACH ROW EXECUTE FUNCTION post_deletion_body_guard_v1();
CREATE TRIGGER comment_deletion_body_guard_v1 BEFORE INSERT OR UPDATE ON comments FOR EACH ROW EXECUTE FUNCTION post_deletion_body_guard_v1();
-- Current Concert writes join the same permanent subject deny. Cleaned DELETED
-- rows remain updatable only as metadata tombstones; no raw text/media survives.
CREATE FUNCTION post_deletion_concert_guard_v1() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE subjects uuid[]; parent_author uuid;
BEGIN
 IF TG_TABLE_NAME='post_concert_posts' THEN
   IF NEW.state='DELETED' AND NEW.content='{"text":"[deleted]","media_ids":[],"links":[]}'::jsonb THEN RETURN NEW; END IF;
   subjects:=array[NEW.author_id];
 ELSIF TG_TABLE_NAME='post_concert_comments' THEN
   IF NEW.state='DELETED' AND NEW.text='[deleted]' THEN RETURN NEW; END IF;
   select author_id into parent_author from post_concert_posts where post_id=NEW.post_id;
   subjects:=array[NEW.author_id,parent_author];
 ELSIF TG_TABLE_NAME='post_concert_reactions' THEN
   select author_id into parent_author from post_concert_posts where post_id=NEW.post_id;
   subjects:=array[NEW.actor_id,parent_author];
 ELSIF TG_TABLE_NAME='post_concert_receipts' THEN
   select author_id into parent_author from post_concert_posts where post_id=NEW.post_id;
   subjects:=array[NEW.actor_id,parent_author];
 ELSE
   select actor_id into parent_author from post_concert_receipts where operation_id=NEW.operation_id;
   subjects:=array[parent_author];
 END IF;
 PERFORM post_assert_subjects_not_deleted(subjects);
 RETURN NEW;
END $$;
CREATE TRIGGER post_deletion_concert_post_v1 BEFORE INSERT OR UPDATE ON post_concert_posts FOR EACH ROW EXECUTE FUNCTION post_deletion_concert_guard_v1();
CREATE TRIGGER post_deletion_concert_comment_v1 BEFORE INSERT OR UPDATE ON post_concert_comments FOR EACH ROW EXECUTE FUNCTION post_deletion_concert_guard_v1();
CREATE TRIGGER post_deletion_concert_reaction_v1 BEFORE INSERT OR UPDATE ON post_concert_reactions FOR EACH ROW EXECUTE FUNCTION post_deletion_concert_guard_v1();
CREATE TRIGGER post_deletion_concert_receipt_v1 BEFORE INSERT OR UPDATE ON post_concert_receipts FOR EACH ROW EXECUTE FUNCTION post_deletion_concert_guard_v1();
CREATE TRIGGER post_deletion_concert_outbox_v1 BEFORE INSERT ON post_concert_outbox FOR EACH ROW EXECUTE FUNCTION post_deletion_concert_guard_v1();
-- Actual legacy producers put personal text in payload.content. Preserve
-- event IDs/order/attempts/publication timestamps; never invent a delivery ACK.
CREATE FUNCTION post_deletion_outbox_body_guard_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF EXISTS(select 1 from post_user_deletion_tombstones where user_id=NEW.actor_id) THEN
   NEW.payload:=(NEW.payload::jsonb-'content')::text;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER post_deletion_outbox_body_guard_v1 BEFORE INSERT OR UPDATE OF payload ON post_event_outbox
 FOR EACH ROW EXECUTE FUNCTION post_deletion_outbox_body_guard_v1();
CREATE FUNCTION post_deletion_publication_body_guard_v1() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE source post_event_outbox%rowtype;
BEGIN
 -- Serialize with cleanup's actual source-row UPDATE. A delayed genuine
 -- completion cannot restore an already scrubbed source from its old claim DTO.
 SELECT * INTO source FROM post_event_outbox WHERE event_id=NEW.event_id FOR SHARE;
 IF FOUND AND EXISTS(select 1 from post_user_deletion_tombstones where user_id=source.actor_id) THEN
   NEW.payload:=source.payload;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER post_deletion_publication_body_guard_v1 BEFORE INSERT OR UPDATE OF payload ON event_publications
 FOR EACH ROW EXECUTE FUNCTION post_deletion_publication_body_guard_v1();
