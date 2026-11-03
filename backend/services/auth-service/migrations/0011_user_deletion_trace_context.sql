alter table user_deletion_sagas
    add column origin_traceparent text null;

alter table user_deletion_sagas
    add constraint user_deletion_sagas_origin_traceparent_length_check
    check (origin_traceparent is null or char_length(origin_traceparent) = 55)
    not valid;
