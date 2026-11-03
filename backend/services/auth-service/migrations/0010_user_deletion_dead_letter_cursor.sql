-- Keep the bounded operator backlog scan indexable without adding entries for
-- sagas that are progressing normally or have already completed.
create index if not exists user_deletion_sagas_dead_lettered_cursor_idx
    on user_deletion_sagas (updated_at, id)
    where status = 'DEAD_LETTERED';
