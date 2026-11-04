-- Additive reviewed motion-v2 representation. Existing rows/constraints semantics remain.
ALTER TABLE public.concert_avatar_settings DROP CONSTRAINT concert_avatar_representation;
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
    OR
    (contract_version = '2.0.0' AND manifest_id = 'mcr_selected_motion_reviewed_20261104' AND manifest_version = 2 AND representation = 'motion_preset'
        AND preset_id IN ('motion_v2_photo01_front_01','motion_v2_photo01_front_02','motion_v2_photo01_front_03','motion_v2_photo01_front_04','motion_v2_photo01_front_05','motion_v2_photo01_front_06') AND preset_id IS NOT NULL
        AND body_id IS NULL AND eyes_id IS NULL AND hair_id IS NULL
        AND outfit_id IS NULL AND accessory_id IS NULL AND skin_tone_id IS NULL)
    OR
    (contract_version = '2.0.0' AND manifest_id = 'mcr_selected_motion_reviewed_20261104' AND manifest_version = 2 AND representation = 'motion_layered' AND preset_id IS NULL AND skin_tone_id IS NULL
        AND body_id IS NOT NULL AND body_id IN ('motion_v2_body_0','motion_v2_body_1','motion_v2_body_2','motion_v2_body_3','motion_v2_body_4','motion_v2_body_5','motion_v2_body_6','motion_v2_body_7')
        AND eyes_id IS NOT NULL AND eyes_id IN ('motion_v2_eyes_0','motion_v2_eyes_1','motion_v2_eyes_2','motion_v2_eyes_3')
        AND hair_id IS NOT NULL AND hair_id IN ('motion_v2_hair_0','motion_v2_hair_1','motion_v2_hair_2','motion_v2_hair_3','motion_v2_hair_4','motion_v2_hair_5')
        AND (outfit_id IS NULL OR outfit_id IN ('motion_v2_outfit_0','motion_v2_outfit_1','motion_v2_outfit_2','motion_v2_outfit_3','motion_v2_outfit_4','motion_v2_outfit_5'))
        AND (accessory_id IS NULL OR accessory_id IN ('motion_v2_accessory_0','motion_v2_accessory_1','motion_v2_accessory_2','motion_v2_accessory_3')))
);
