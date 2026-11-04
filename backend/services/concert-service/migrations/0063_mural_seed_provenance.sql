-- Local owning-operator provisioning only; no public mutation or inferred authority.
-- 0019 and 0058 remain unchanged. The complete SHA-256 values are durable, never
-- truncated into the public snapshot UUID. The UUID remains an independent ID.
CREATE TABLE public.concert_mural_seed_provenance (
 event_id uuid PRIMARY KEY REFERENCES public.concert_mural_state(event_id),
 initial_snapshot_id uuid NOT NULL UNIQUE,
 manifest_sha256 bytea NOT NULL CHECK(octet_length(manifest_sha256)=32),
 input_digest bytea NOT NULL CHECK(octet_length(input_digest)=32),
 source_revision text NOT NULL CHECK(source_revision ~ '^[0-9a-f]{40}$'),
 operator_binding text NOT NULL CHECK(octet_length(operator_binding) BETWEEN 1 AND 512 AND length(btrim(operator_binding))>0),
 initial_created_at timestamptz NOT NULL,
 initial_snapshot_created_at timestamptz NOT NULL,
 provisioned_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE FUNCTION public.concert_mural_seed_provenance_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 RAISE EXCEPTION 'immutable initial mural provenance' USING ERRCODE='23514';
END $$;
CREATE TRIGGER concert_mural_seed_provenance_immutable
 BEFORE UPDATE OR DELETE ON public.concert_mural_seed_provenance
 FOR EACH ROW EXECUTE FUNCTION public.concert_mural_seed_provenance_immutable();
CREATE TRIGGER concert_mural_seed_provenance_truncate_guard
 BEFORE TRUNCATE ON public.concert_mural_seed_provenance
 FOR EACH STATEMENT EXECUTE FUNCTION public.concert_mural_seed_provenance_immutable();
