-- Additive C06 representation storage. Existing layered rows remain unchanged.
ALTER TABLE public.concert_avatar_settings
    ADD COLUMN representation text NOT NULL DEFAULT 'layered',
    ADD COLUMN preset_id text,
    ALTER COLUMN body_id DROP NOT NULL,
    ALTER COLUMN eyes_id DROP NOT NULL,
    ALTER COLUMN hair_id DROP NOT NULL,
    ALTER COLUMN outfit_id DROP NOT NULL,
    ALTER COLUMN skin_tone_id DROP NOT NULL,
    DROP CONSTRAINT concert_avatar_contract;

ALTER TABLE public.concert_avatar_settings ADD CONSTRAINT concert_avatar_representation CHECK (
    (representation = 'layered' AND contract_version = '1.0.0'
        AND preset_id IS NULL AND body_id IS NOT NULL AND eyes_id IS NOT NULL
        AND hair_id IS NOT NULL AND outfit_id IS NOT NULL AND skin_tone_id IS NOT NULL)
    OR
    (representation = 'whole_character' AND contract_version = '1.1.0'
        AND manifest_id = 'mcr_selected_static_20261103' AND manifest_version = 1
        AND preset_id IS NOT NULL AND preset_id IN (
            'photo01_front_01','photo01_front_02','photo01_front_03',
            'photo01_front_04','photo01_front_05','photo01_front_06')
        AND body_id IS NULL AND eyes_id IS NULL AND hair_id IS NULL
        AND outfit_id IS NULL AND accessory_id IS NULL AND skin_tone_id IS NULL)
);
-- Scope/revision/timestamp triggers and participant FK remain in force.
