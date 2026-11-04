# Social policy production integration

Source qualification, 2026-11-04 11:03 UTC. The existing generator and drift check,
195 tests across three files, scoped types and lint all passed. Actual runtime
qualification remains NOT_RUN. This document does not establish IN03, production
availability or live privacy acceptance.

## Contract and strict transport boundary

The separately committed backend contract source is
`930380deed5c47b4c9eed920a8d9352278f0e90e`. Existing World Snapshot fields remain
unchanged. The server union adds explicit metadata-only `policy.baseline` and
`policy.invalidate` frames; the revision is a positive safe integer, scoped to the
canonical actor/event and bound to its existing World stream. No new socket,
credential, private content, handle or completion capability is introduced.

The frontend consumes the canonical JSON schema through its existing strict
decoder. The existing type generator regenerated `wire-types.ts` under the
allocated source workflow. Transport requires an actual accepted World snapshot
before policy metadata and checks its current stream and event before forwarding.
Metadata remains deliverable during a room or resync transition: it cannot be
silently dropped merely because movement is paused. It does not grant HTTP access.

The source workflow covers strict variant/field/revision rejection, missing or
mismatched snapshot binding, one incumbent connection, and the existing transport
and replication regressions. The actual workflow passed generated drift, scoped
types and lint. Receipts are in `../evidence/social-policy-wire/source-01/`.

The accepted job `ROOT-SOCIAL-WIRE-SOURCE-202611041103Z` ran on source
`2d7a42e7d9fc645a3d956cd552304539be602be7`, producing the generated type
postimage. It fully released at 11:03:21.640133 UTC after 4.770289 seconds.
Peak whole accounting was 57,417,728 bytes including 55,574,528 carry bytes;
temporary output peaked at 1,830,912 bytes. No server, browser, HTTP or PostgreSQL
job ran. The owned dependency link and private temporary/cache tree were removed.
This qualifies wire compatibility, not a working private Social availability or
identity-lifetime adapter.

## Unfinished shell lifetime and availability work

The actual default entry currently supplies no `SocialProductionPorts`; fallback
availability remains unavailable. `SocialAvailabilityPort` requires the exact
validated bootstrap object, full current `PrivacyScope`, and explicit per-service
readiness. World or policy availability cannot supply missing Intro, Inbox or Age
readiness. The backend owner confirmed there is no committed canonical public
endpoint for this tuple and is preparing an explicit actor/event-bound response.
Unsupplied services stay false. The existing Feed and social-sharing C03 flags
remain necessary conditions for their own services, not general readiness grants.

Two concrete incumbent lifetime constraints remain:

1. Hiding World currently causes `WorldReplication.setVisible(false)` to disconnect
   the socket while retaining the game canvas. A foreground Social view therefore
   cannot claim a current policy baseline merely because its background exists.
2. `WorldProduction`, World composition and the selected-character registry bind
   the global privacy tuple and register disposal on privacy changes. A Social
   policy-generation bump currently destroys that World authority. Keeping a
   background socket connected alone does not solve the identity lifetime or
   rebaseline problem.

Root owns the shared adapter and lifetime seams. Integration must keep one canonical
Auth and the one existing World transport, preserve movement blocking while a
foreground panel owns input, and separate Social private withdrawal from World
identity/admission lifetime. Identity/event loss, suspension, closure and true World
revocation must still terminate World authority. No stale private HTTP result,
cached disclosure or media URL may survive a Social withdrawal or refill a newer
tuple. New current-stream baseline plus fresh bootstrap and availability are
required before private reads resume; duplicate baselines never reopen revoked
authority. The original IN03 harness borrows this eventual production API rather
than constructing another Auth, socket or policy producer.

Authenticated C03 is validated in memory. Persisted evidence, if an already
authorized actual pair produces it, contains only the approved public resource
projection labelled `NOT_BYTE_EXACT_FULL_C03` and a projection-only digest. No raw
authenticated body or full-body digest is retained. The explicitly labelled Map
eligible-attendee fixture is independent of this real-service workflow.
