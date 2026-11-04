-- Add the genuine Notification producer without rewriting published 0017.
-- Existing identity, quarantine, expiry and transition guards remain unchanged.
ALTER TABLE public.concert_owner_guard_participants
    DROP CONSTRAINT concert_owner_guard_participants_producer_check;
ALTER TABLE public.concert_owner_guard_participants
    ADD CONSTRAINT concert_owner_guard_participants_producer_check
    CHECK (producer IN ('CONCERT','POST','PROFILE','NOTIFICATION'));
