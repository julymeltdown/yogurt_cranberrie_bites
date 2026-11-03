alter table post_event_outbox
    add column origin_traceparent text null;

alter table post_event_outbox
    add constraint post_event_outbox_origin_traceparent_length_check
    check (origin_traceparent is null or char_length(origin_traceparent) = 55)
    not valid;
