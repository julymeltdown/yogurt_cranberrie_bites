-- Root response-scope lifecycle only. A finished root does not certify any
-- producer transaction; Auth/Friend independently resolve every tracked xid.
CREATE TABLE public.concert_product_owner_scopes (
 instance_id uuid NOT NULL CHECK(instance_id <> '00000000-0000-0000-0000-000000000000'),
 scope_id uuid NOT NULL CHECK(scope_id <> '00000000-0000-0000-0000-000000000000'),
 binding_digest bytea NOT NULL CHECK(octet_length(binding_digest)=32),
 binding jsonb NOT NULL CHECK(jsonb_typeof(binding)='object'),
 delivery_secret_hash bytea CHECK(delivery_secret_hash IS NULL OR octet_length(delivery_secret_hash)=32),
 phase text NOT NULL DEFAULT 'ACTIVE' CHECK(phase IN ('ACTIVE','SEALED','ROOT_FINISHED')),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 serialized_at timestamptz,
 sealed_at timestamptz,
 root_finished_at timestamptz,
 delivery_completed_at timestamptz,
 PRIMARY KEY(instance_id,scope_id),
 UNIQUE(instance_id,scope_id,binding_digest),
 CHECK(delivery_completed_at IS NULL OR (phase='ROOT_FINISHED' AND delivery_secret_hash IS NOT NULL AND serialized_at IS NOT NULL)),
 CHECK((phase='ACTIVE' AND sealed_at IS NULL AND root_finished_at IS NULL)
    OR (phase='SEALED' AND sealed_at IS NOT NULL AND root_finished_at IS NULL)
    OR (phase='ROOT_FINISHED' AND sealed_at IS NOT NULL AND root_finished_at IS NOT NULL
        AND ((serialized_at IS NOT NULL AND delivery_secret_hash IS NOT NULL)
          OR (serialized_at IS NULL AND delivery_secret_hash IS NULL))))
);
CREATE INDEX concert_product_owner_scopes_unfinished
 ON public.concert_product_owner_scopes(instance_id) WHERE phase<>'ROOT_FINISHED';

CREATE TABLE public.concert_product_owner_transactions (
 instance_id uuid NOT NULL,
 scope_id uuid NOT NULL,
 binding_digest bytea NOT NULL,
 identity_digest bytea NOT NULL CHECK(octet_length(identity_digest)=32),
 identity jsonb NOT NULL CHECK(jsonb_typeof(identity)='object'),
 auth_registration_id uuid,
 friend_registration_id uuid,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 confirmed_at timestamptz,
 PRIMARY KEY(instance_id,scope_id,identity_digest),
 FOREIGN KEY(instance_id,scope_id,binding_digest)
  REFERENCES public.concert_product_owner_scopes(instance_id,scope_id,binding_digest),
 CHECK((confirmed_at IS NULL)
    OR (confirmed_at IS NOT NULL AND auth_registration_id IS NOT NULL AND friend_registration_id IS NOT NULL)),
 CHECK(auth_registration_id IS NULL OR auth_registration_id<>'00000000-0000-0000-0000-000000000000'),
 CHECK(friend_registration_id IS NULL OR friend_registration_id<>'00000000-0000-0000-0000-000000000000')
);
CREATE UNIQUE INDEX concert_product_owner_transactions_auth_registration
 ON public.concert_product_owner_transactions(auth_registration_id) WHERE auth_registration_id IS NOT NULL;
CREATE UNIQUE INDEX concert_product_owner_transactions_friend_registration
 ON public.concert_product_owner_transactions(friend_registration_id) WHERE friend_registration_id IS NOT NULL;

CREATE FUNCTION public.concert_product_owner_scope_transition() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN
  RAISE EXCEPTION 'owner scope lifecycle evidence is immutable' USING ERRCODE='23514';
 END IF;
 IF (NEW.instance_id,NEW.scope_id,NEW.binding_digest,NEW.binding,NEW.created_at)
    IS DISTINCT FROM (OLD.instance_id,OLD.scope_id,OLD.binding_digest,OLD.binding,OLD.created_at)
 OR NOT COALESCE((
      (OLD.phase='ACTIVE' AND NEW.phase='ACTIVE' AND NEW.sealed_at IS NULL AND NEW.root_finished_at IS NULL
       AND NEW.delivery_completed_at IS NULL
       AND ((OLD.delivery_secret_hash IS NULL AND NEW.delivery_secret_hash IS NOT NULL AND NEW.serialized_at IS NOT DISTINCT FROM OLD.serialized_at)
         OR (NEW.delivery_secret_hash=OLD.delivery_secret_hash AND OLD.serialized_at IS NULL AND NEW.serialized_at IS NOT NULL)))
   OR (OLD.phase='ACTIVE' AND NEW.phase='SEALED' AND NEW.root_finished_at IS NULL
       AND NEW.delivery_completed_at IS NULL
       AND NEW.delivery_secret_hash IS NOT DISTINCT FROM OLD.delivery_secret_hash
       AND NEW.serialized_at IS NOT DISTINCT FROM OLD.serialized_at)
   OR (OLD.phase='SEALED' AND NEW.phase='ROOT_FINISHED' AND NEW.sealed_at=OLD.sealed_at
       AND NEW.delivery_completed_at IS NULL
       AND NEW.delivery_secret_hash IS NOT DISTINCT FROM OLD.delivery_secret_hash
       AND NEW.serialized_at IS NOT DISTINCT FROM OLD.serialized_at)
   OR (OLD.phase='ROOT_FINISHED' AND NEW.phase='ROOT_FINISHED'
       AND OLD.delivery_completed_at IS NULL AND NEW.delivery_completed_at IS NOT NULL
       AND NEW.sealed_at=OLD.sealed_at AND NEW.root_finished_at=OLD.root_finished_at
       AND NEW.delivery_secret_hash=OLD.delivery_secret_hash AND NEW.serialized_at=OLD.serialized_at)),false) THEN
  RAISE EXCEPTION 'invalid owner scope lifecycle transition' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_product_owner_scope_transition BEFORE UPDATE OR DELETE
 ON public.concert_product_owner_scopes FOR EACH ROW EXECUTE FUNCTION public.concert_product_owner_scope_transition();

CREATE FUNCTION public.concert_product_owner_transaction_transition() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE current_phase text;
BEGIN
 IF TG_OP='DELETE' THEN
  RAISE EXCEPTION 'owner transaction evidence is immutable' USING ERRCODE='23514';
 END IF;
 -- Serialize registration/confirmation against durable seal. No new work can
 -- cross this barrier, including a Track RPC returning after root cancellation.
 SELECT phase INTO current_phase FROM public.concert_product_owner_scopes
  WHERE instance_id=NEW.instance_id AND scope_id=NEW.scope_id FOR UPDATE;
 IF current_phase IS DISTINCT FROM 'ACTIVE' THEN
  RAISE EXCEPTION 'owner scope is sealed' USING ERRCODE='23514';
 END IF;
 IF TG_OP='UPDATE' AND (
   (NEW.instance_id,NEW.scope_id,NEW.binding_digest,NEW.identity_digest,NEW.identity,NEW.created_at)
    IS DISTINCT FROM (OLD.instance_id,OLD.scope_id,OLD.binding_digest,OLD.identity_digest,OLD.identity,OLD.created_at)
   OR OLD.confirmed_at IS NOT NULL
   OR (OLD.auth_registration_id IS NOT NULL AND NEW.auth_registration_id IS DISTINCT FROM OLD.auth_registration_id)
   OR (OLD.friend_registration_id IS NOT NULL AND NEW.friend_registration_id IS DISTINCT FROM OLD.friend_registration_id)) THEN
  RAISE EXCEPTION 'invalid owner transaction confirmation' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_product_owner_transaction_transition BEFORE INSERT OR UPDATE OR DELETE
 ON public.concert_product_owner_transactions FOR EACH ROW EXECUTE FUNCTION public.concert_product_owner_transaction_transition();
