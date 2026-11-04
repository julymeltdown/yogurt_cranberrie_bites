-- Concert-local application receipts; these are not another source journal or queue.
CREATE TABLE public.concert_world_remote_runs (
 consumer_id uuid PRIMARY KEY CHECK(consumer_id<>'00000000-0000-0000-0000-000000000000'),
 run_epoch uuid NOT NULL CHECK(run_epoch<>'00000000-0000-0000-0000-000000000000'),
 lease_until timestamptz NOT NULL,
 updated_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE public.concert_world_remote_checkpoints (
 consumer_id uuid NOT NULL REFERENCES public.concert_world_remote_runs(consumer_id),
 source_owner text NOT NULL CHECK(source_owner IN ('AUTH','FRIEND')),
 source_epoch uuid NOT NULL CHECK(source_epoch<>'00000000-0000-0000-0000-000000000000'),
 sequence bigint NOT NULL CHECK(sequence BETWEEN 0 AND 9007199254740991),
 updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(consumer_id,source_owner)
);
CREATE TABLE public.concert_world_remote_effects (
 consumer_id uuid NOT NULL REFERENCES public.concert_world_remote_runs(consumer_id),
 source_owner text NOT NULL CHECK(source_owner IN ('AUTH','FRIEND')),
 source_epoch uuid NOT NULL CHECK(source_epoch<>'00000000-0000-0000-0000-000000000000'),
 sequence bigint NOT NULL CHECK(sequence BETWEEN 1 AND 9007199254740991),
 signal_id uuid NOT NULL CHECK(signal_id<>'00000000-0000-0000-0000-000000000000'),
 reason text NOT NULL,
 scope_kind text NOT NULL CHECK(scope_kind IN ('USER','FAMILY','PAIR')),
 scope_a uuid NOT NULL CHECK(scope_a<>'00000000-0000-0000-0000-000000000000'),
 scope_b uuid NOT NULL CHECK(scope_b<>'00000000-0000-0000-0000-000000000000'),
 victims jsonb NOT NULL CHECK(jsonb_typeof(victims)='array' AND jsonb_array_length(victims)<=12800),
 applied_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(consumer_id,source_owner,source_epoch,sequence),
 UNIQUE(consumer_id,source_owner,signal_id),
 CHECK((source_owner='AUTH' AND reason='AUTH_SESSION_CHANGED' AND scope_kind='FAMILY')
 OR (source_owner='AUTH' AND reason IN ('AUTH_USER_AUTHORITY_CHANGED','AUTH_CREDENTIALS_CHANGED','AUTH_USER_REMOVED') AND scope_kind='USER' AND scope_a=scope_b)
 OR (source_owner='FRIEND' AND reason='FRIEND_RELATIONSHIP_CHANGED' AND scope_kind='PAIR' AND scope_a<>scope_b)
 OR (source_owner='FRIEND' AND reason='FRIEND_USER_REMOVED' AND scope_kind='USER' AND scope_a=scope_b))
);
CREATE TRIGGER concert_world_remote_effects_immutable BEFORE UPDATE OR DELETE ON public.concert_world_remote_effects
 FOR EACH ROW EXECUTE FUNCTION public.concert_maintenance_immutable();
