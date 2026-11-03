alter table auth_mail_outbox
    add column origin_traceparent text null;

alter table auth_mail_outbox
    add constraint auth_mail_outbox_origin_traceparent_check
    check (
        origin_traceparent is null
        or (
            char_length(origin_traceparent) = 55
            and origin_traceparent ~ '^00-[0-9a-f]{32}-[0-9a-f]{16}-0[01]$'
        )
    ) not valid;
