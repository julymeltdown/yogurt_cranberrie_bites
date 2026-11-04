-- Add only the closed Social read-child kind. Published 0028/0037 are immutable.
-- Preserve the encrypted reference bounds, 202-child barrier, full scope FK,
-- immutable identity and actual-terminal completion guards.
ALTER TABLE public.concert_product_child_intents
 DROP CONSTRAINT concert_product_child_intents_kind_check;
ALTER TABLE public.concert_product_child_intents
 ADD CONSTRAINT concert_product_child_intents_kind_check
 CHECK (kind IN ('PROFILE_PUBLIC','LOCAL_PUBLIC','GIFT_PRIVATE','SOCIAL_READ'));
