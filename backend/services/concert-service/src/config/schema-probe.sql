SELECT
    to_regclass('public.concert_events') IS NOT NULL
    AND to_regclass('public.concert_participants') IS NOT NULL
    AND to_regclass('public.concert_avatar_settings') IS NOT NULL
    AND NOT EXISTS (
        SELECT name FROM unnest(ARRAY[
            'concert_event_slug_shape','concert_event_title_bounds','concert_event_phase',
            'concert_event_timezone','concert_event_revision','concert_event_times','concert_event_slug_unique',
            'concert_participant_revision','concert_participant_theme','concert_participant_key',
            'concert_participant_event','concert_participant_times','concert_avatar_revision','concert_avatar_representation',
            'concert_avatar_manifest_id','concert_avatar_manifest_version','concert_avatar_body','concert_avatar_eyes',
            'concert_avatar_hair','concert_avatar_outfit','concert_avatar_accessory','concert_avatar_skin_tone',
            'concert_avatar_key','concert_avatar_participant','concert_avatar_times'
        ]) AS required(name)
        EXCEPT
        SELECT conname::text FROM pg_constraint
        WHERE connamespace='public'::regnamespace AND convalidated
          AND conrelid IN ('public.concert_events'::regclass,'public.concert_participants'::regclass,'public.concert_avatar_settings'::regclass)
    )
    AND (SELECT count(*)=3 FROM pg_trigger WHERE NOT tgisinternal AND tgenabled='O' AND tgfoid='public.concert_guard_revision()'::regprocedure
         AND tgname IN ('concert_events_revision_guard','concert_participants_revision_guard','concert_avatars_revision_guard'));
