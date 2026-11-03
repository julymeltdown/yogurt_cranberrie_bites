alter table gateway_async_operation_queue
    add column origin_traceparent text null,
    add constraint gateway_async_operation_queue_origin_traceparent_check
        check (
            origin_traceparent is null
            or (
                length(origin_traceparent) = 55
                and origin_traceparent ~ '^00-[0-9a-f]{32}-[0-9a-f]{16}-0[01]$'
            )
        ) not valid;
