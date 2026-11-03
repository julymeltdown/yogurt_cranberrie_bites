# Event moderation authority clarification

Date: 2026-11-03. C04 remains **REVIEW_CANDIDATE / NOT_ACCEPTED**. This reviewed
contract correction makes the existing event route's authority explicit; it adds
no moderation producer, role mapping, migration or active invalidation protocol.

The prior `moderate_user` rule said to revoke sessions through an unspecified
identity/policy service. That could allow an interpretation where an event
moderator revokes global Auth login families. The route is
`/api/concert/events/{eventId}/admin/users/{userId}/moderation`; Concert owns its
event restriction, while Auth owns global account/session state.

The candidate now declares `PATH_EVENT_ONLY` in the operation and generated
OpenAPI. A fresh role in the exact event and current event restriction revision
are required. Suspension advances the event admission/revocation epoch and
invalidates that event's membership, sockets, tickets and private grants. Auth
refresh families and other events remain unchanged. Unsuspension requires fresh
authorization and revives no old authority or consent. A global account action
requires a separately authorized Auth-owned operation. Schema descriptions
distinguish event suspension from owning account deletion projections.

The assembler refuses a silent change to global scope, an unscoped route or an
Auth-admin actor classification. Its adversarial regression checks all three
changes; the valid generated operation carries the explicit scope extension.
No C03 core route/definition or C05 realtime contract changed. All 106 domain
operations remain unimplemented candidates.

`bash scripts/mcr/check.sh contracts` exited **0**: inherited/core/domain artifacts
reproduce, 33 core / 24 realtime / 36 domain indexed fixtures pass, and
**138 contract regressions** pass (21 asset, 23 core, 31 realtime, 47 domain and
16 bundle). The retained log is [event-moderation-scope.log](event-moderation-scope.log).
This is static semantic/assembly evidence, not actual server authorization,
database atomicity, revocation delivery, cross-event runtime isolation or C04
freeze. Those remain required before an accepting moderation producer.

Current source pins are in [event-moderation-scope.json](event-moderation-scope.json).
Earlier BE15 evidence retains the hashes it actually reviewed; its private HMAC,
receipt/outbox implementation and public receipt projection are unchanged.
