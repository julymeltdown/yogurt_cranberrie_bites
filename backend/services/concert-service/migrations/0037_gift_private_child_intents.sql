-- Add only the closed Gift read-child kind. Published 0028 remains immutable.
-- The existing 202-child admission barrier, encrypted private reference bounds,
-- lifecycle triggers, scope binding FK and completion guards remain unchanged.
ALTER TABLE public.concert_product_child_intents
 DROP CONSTRAINT concert_product_child_intents_kind_check;
ALTER TABLE public.concert_product_child_intents
 ADD CONSTRAINT concert_product_child_intents_kind_check
 CHECK (kind IN ('PROFILE_PUBLIC','LOCAL_PUBLIC','GIFT_PRIVATE'));
