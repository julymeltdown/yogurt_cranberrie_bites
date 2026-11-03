-- BE27 owned prerequisite schema. It is not the global migration ledger.
CREATE TABLE public.concert_intros (
  event_id uuid NOT NULL REFERENCES public.concert_events(event_id),
  intro_id uuid NOT NULL,
  owner_id uuid NOT NULL,
  revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
  state text NOT NULL CHECK(state IN ('DRAFT','PUBLISHED','CLOSED')),
  text text NOT NULL CHECK(char_length(text)<=1000),
  minimum_age smallint,
  maximum_age_exclusive smallint,
  expires_at timestamptz,
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  PRIMARY KEY(event_id,intro_id),
  UNIQUE(event_id,owner_id),
  CHECK((minimum_age IS NULL)=(maximum_age_exclusive IS NULL)),
  CHECK((minimum_age IS NULL AND maximum_age_exclusive IS NULL)
    OR (minimum_age BETWEEN 14 AND 100 AND maximum_age_exclusive BETWEEN 15 AND 101 AND minimum_age<maximum_age_exclusive)),
  CHECK((state='DRAFT') OR (btrim(text)<>'')),
  CHECK(updated_at>=created_at)
);
CREATE INDEX concert_intros_discovery ON public.concert_intros(event_id,created_at DESC,intro_id DESC) WHERE state='PUBLISHED';

-- The optional policy-edit consent seam for BE28/29 remains a required owner
-- port: it is validated inside the command transaction, not represented by a
-- permissive local fallback.

-- The adapter checks the transaction-scoped authority deadline before projection;
-- a production implementation must retain its owner-source locks through COMMIT.

CREATE FUNCTION public.concert_intro_update_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.event_id IS DISTINCT FROM OLD.event_id
     OR NEW.intro_id IS DISTINCT FROM OLD.intro_id
     OR NEW.owner_id IS DISTINCT FROM OLD.owner_id
     OR NEW.created_at IS DISTINCT FROM OLD.created_at THEN
    RAISE EXCEPTION 'introduction scope is immutable' USING ERRCODE='23514';
  END IF;
  IF OLD.state='CLOSED' THEN
    RAISE EXCEPTION 'closed introduction is terminal' USING ERRCODE='23514';
  END IF;
  IF NEW.revision<>OLD.revision+1 THEN
    RAISE EXCEPTION 'introduction revision must advance once' USING ERRCODE='23514';
  END IF;
  IF OLD.state='PUBLISHED' AND NEW.state='DRAFT' THEN
    RAISE EXCEPTION 'published introduction cannot return to draft' USING ERRCODE='23514';
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER concert_intro_update_guard BEFORE UPDATE ON public.concert_intros
  FOR EACH ROW EXECUTE FUNCTION public.concert_intro_update_guard();
