-- Add the closed target exclusion child; preserve every published child kind.
ALTER TABLE public.concert_product_child_intents
 DROP CONSTRAINT concert_product_child_intents_kind_check;
ALTER TABLE public.concert_product_child_intents
 ADD CONSTRAINT concert_product_child_intents_kind_check
 CHECK (kind IN ('PROFILE_PUBLIC','LOCAL_PUBLIC','GIFT_PRIVATE','SOCIAL_READ','NOTIFICATION_INBOX','TARGET_READ'));
