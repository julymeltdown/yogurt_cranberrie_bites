# Participant post-response delivery retention (CR10)

Status: IN_PROGRESS. Base: `c9dd4c980f39b12eda5a79fff871c8de48f8df71`.

## Scope and provenance

Owned production files are the Participant Gateway inbound HTTP and outbound gRPC adapters. New tests/support and this document are the only other writable paths. No protocol, manifest, composition, generic Product, or source archive changes.

The controlled-peer regression fixture derives from `backend/services/api-gateway/tests/concert_participant.rs` at the base revision. Its identity and route helpers are reused unchanged by relative path. These peers issue synthetic capabilities and therefore establish Gateway behavior only, never native owner settlement.

## Defect and required behavior

After receiving a serialized authenticated Participant response, freshness checks can return before preserving its delivery capability. HTTP access rechecks and disclosure-body construction can also discard it. An expired response must disclose no protected content. Once the complete reply has passed authenticated metadata, bounded positive original expiry, actor/event/schema, closed failure and capability validation, a late refusal must retain that same capability through a newly authored closed public error. Only actual outer HTTP EOF can settle it. Drop, partial consumption, body error, invalid reply, and unknown delivery remain quarantined.

## Validation

`participant_delivery_retention` contains a post-gRPC shorter-content-ceiling expiry regression, error-data-versus-EOF assertions, partial/drop negatives, mismatched actor, malformed capability, missing no-store metadata and zero-expiry negatives. Pre-fix source `df8728b1bc6a490b69f6782e8a294d880908b200`: build PASS (exit 0, Rust 1.88 locked/offline, 30.39 seconds), copied component run FAIL as required (exit 101, 2 passed / 1 failed). The exact positive error EOF test observed completion 0 instead of 1; the drop/partial/invalid reply test passed. Frozen binary SHA256 `2256b6991f904660eb834ee007cb72566c064f3a9fe1a7d4c96f29830e385fab`; log SHA256 `48c0cfa0b9f359f0fb42088d803f7ad55491f4ac5f0ce6f6a7604c398f0025b7`. Actual zero-handle build-target release preceded the copied test; no PG was started. Expected functional failure is preserved independently of later fixes. Fixed execution: NOT_RUN. Actual PG16/mTLS/HTTP/error-EOF/Auth-refresh qualification: NOT_RUN. Exact signed-JWT expiry scheduling and the Root production incident are not reproduced by these controlled peers.

## Implementation and native regression boundary

The outbound adapter validates no-store metadata, positive expiry bounded by the sent original authority, opaque delivery structure, and the closed response schema/actor/event before post-response freshness checks. A late refusal clears protected bytes and keeps the exact capability and original expiry in a closed failure. The HTTP adapter captures delivery before its late access check; locally authored errors remain readable without a protected-content gate, while successful content retains the unchanged original DisclosureBody checks and error/drop quarantine semantics.

The new native host uses production Gateway state/router/transport layers, actual signed JWT verification and owner identity reads. A test-owned mTLS proxy forwards the original signed Participant request to the real production Concert main, preserves the genuine delivery capability, shortens only the returned disclosure ceiling and delays beyond it. Separate body gates hold an unopened error or return only half a data frame before actual TCP reset; they never poll/synthesize EOF. The controller requires real signup/mail verification/login/Profile/bootstrap, genuine terminal-error EOF/durable owner settlement, and actual Auth refresh 200 afterward. Negative roots must remain serialized/undelivered with four prepared fences until owned-cluster teardown.

All original 2,151 retained d0f source input hashes and the frozen Concert binary hash match the CR10 base. Nevertheless its embedded migration directory points into the advancing integration checkout, which now has an additional migration. Native qualification must rebuild the exact Concert main in this stable worktree; the six frozen peer embedded migration sets still match the base. No released build target is accessed for reuse.
