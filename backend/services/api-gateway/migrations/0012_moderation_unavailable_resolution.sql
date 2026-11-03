do $$
declare
    state_constraint text;
begin
    select conname
    into state_constraint
    from pg_constraint
    where conrelid = 'moderation_content_decision_outbox'::regclass
      and contype = 'c'
      and pg_get_constraintdef(oid) like '%TARGET_UNAVAILABLE%';

    if state_constraint is null then
        raise exception 'moderation content outbox state constraint was not found';
    end if;

    execute format(
        'alter table moderation_content_decision_outbox drop constraint %I',
        state_constraint
    );
end
$$;

alter table moderation_content_decision_outbox
    add constraint moderation_content_decision_outbox_state_check
    check (state in ('PENDING', 'TARGET_UNAVAILABLE', 'COMPLETED', 'RESOLVED_UNAVAILABLE'));
