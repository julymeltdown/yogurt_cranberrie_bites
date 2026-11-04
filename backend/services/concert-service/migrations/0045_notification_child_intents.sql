-- Add the closed remote Notification child only. Preserve published 0028/0037/0042.
ALTER TABLE public.concert_product_child_intents
 DROP CONSTRAINT concert_product_child_intents_kind_check;
ALTER TABLE public.concert_product_child_intents
 ADD CONSTRAINT concert_product_child_intents_kind_check
 CHECK (kind IN ('PROFILE_PUBLIC','LOCAL_PUBLIC','GIFT_PRIVATE','SOCIAL_READ','NOTIFICATION_INBOX'));
