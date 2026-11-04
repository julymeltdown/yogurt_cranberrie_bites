# Participant post-response delivery retention (CR10)

Status: IN_PROGRESS. Base: `c9dd4c980f39b12eda5a79fff871c8de48f8df71`.

## Scope and provenance

Owned production files are the Participant Gateway inbound HTTP and outbound gRPC adapters. New tests/support and this document are the only other writable paths. No protocol, manifest, composition, generic Product, or source archive changes.

The controlled-peer regression fixture derives from `backend/services/api-gateway/tests/concert_participant.rs` at the base revision. Its identity and route helpers are reused unchanged by relative path. These peers issue synthetic capabilities and therefore establish Gateway behavior only, never native owner settlement.

## Defect and required behavior

After receiving a serialized authenticated Participant response, freshness checks can return before preserving its delivery capability. HTTP access rechecks and disclosure-body construction can also discard it. An expired response must disclose no protected content. Once the complete reply has passed authenticated metadata, bounded positive original expiry, actor/event/schema, closed failure and capability validation, a late refusal must retain that same capability through a newly authored closed public error. Only actual outer HTTP EOF can settle it. Drop, partial consumption, body error, invalid reply, and unknown delivery remain quarantined.

## Validation

`participant_delivery_retention` contains a post-gRPC shorter-content-ceiling expiry regression, error-data-versus-EOF assertions, partial/drop negatives, mismatched actor, malformed capability, missing no-store metadata and zero-expiry negatives. Pre-fix execution: NOT_RUN. Fixed execution: NOT_RUN. Actual PG16/mTLS/HTTP/error-EOF/Auth-refresh qualification: NOT_RUN. Exact signed-JWT expiry scheduling and the Root production incident are not reproduced by these controlled peers.
