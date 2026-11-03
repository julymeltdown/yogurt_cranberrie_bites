# Relationship policy and private social sharing

Status: BE37 IN_PROGRESS; BE28–BE30 IN_PROGRESS. Real owner composition and
HTTP/WS integration NOT_RUN. No task acceptance or release claim.

## Source and scope

Base: `f0e44bd1bba943b02319f9d9a42433e253e8de6f`, branch
`codex/mcr-be-social-policy`. New MCR code; no copied Bigyeting implementation,
rendering, or source archive changes. Uses existing canonical Auth/Event IDs,
Chrono/UUID/async-trait/SQLx and the BE15 receipt/outbox producer seam.
C04 is the bounded v0.1 candidate at the base, not a full freeze:

| Input | SHA256 |
| --- | --- |
| common.schema.json | 303f3019611c636679282d6c29106deab83d7c0c9717bd8b9ba526d2ff865a35 |
| social.schema.json | b275b36f2ad8eee90922f7ad27cfcbece943773a94831720b6dfca588d0201d2 |
| operations.json | 42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223 |
| policy.json | 508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe |

## Shared typed interface

`concert_domain::relationship` owns `PolicyQuery {event, actor, subject, field}`,
`PolicyStamp {event_revision, relationship_revision, authority_revision}`,
`FreshAuthority`, `PolicySnapshot`, and the pure `decide` matrix.
`concert_application::relationship::RelationshipPolicy::authorize(query)` returns
`PolicyDecision` or coarse `RelationshipError::{Denied,Unavailable,Stale}`.

`PublicProfile` and `Presence` permit authorized public projections in READ_ONLY.
`Interaction` requires OPEN/LIVE. `SafetySelf` supports own safety operations in
READ_ONLY and requires actor=subject; producers independently check ownership and
command lifecycle. Self-pairs need current membership and real actor authority.
`PrivateHandle` always denies at this generic seam; only the dedicated reveal
transaction may reveal a platform/version-bound secret.

A decision is a point-in-time projection, not reusable permission for writes.
Consumers re-evaluate after invalidation and use `ensure_current` only with freshly
resolved revisions/authority. Check exact event/actor/subject/field. Never retain
an allow after policy failure. Neither a Profile row nor participant existence
establishes an authenticated actor, current suspension state, or absence of blocks.

## Required real authority binding

`FencedRelationshipAuthority::check_in(PgConnection, PolicyQuery, DB_now)` must
verify a request-bound authenticated actor, current owning Auth session/account
restrictions, subject restrictions, and inherited Friend blocks in both directions.
It returns the exact query, checked_at=DB_now, positive authority revision, bounded
valid_until, actor_allowed, subject_allowed and inherited_block. Unknown/outage
returns Unavailable. The owner revision must fence block/suspension/session
revocation through the protected transaction, or implement an independently
qualified equivalent revision protocol. An ordinary remote read without a fence
cannot supply mutation authority. Current imported Friend DTO has block booleans
but no such owner revision/fence. An adapter writer must add/qualify this owning
service capability; no permissive production implementation is supplied here.

`UnavailableAuthority` and `UnavailableRelationshipPolicy` are explicit production
defaults. Deterministic injected authority is test-only. Do not expose these
internal assertions as request-body fields or infer them from caller roles.

## Validation record

- `python3 scripts/mcr/check-plan.py`: exit 0, planning checks PASS; runtime NOT_RUN.
- Pinned Rust 1.88 rustfmt on owned Rust files: exit 0.
- Cargo and owned PostgreSQL checks: NOT_RUN at initial API checkpoint; queued
  behind existing shared-target lease. First checkpoint publishes the typed seam
  so community/admission/feed consumers can compile against its stable shape.

Deliver only assigned relationship directories/tests/scripts, three exact module
exports, reserved migration 0006 and this document. Integration, real authority
wiring, transports and remote master publication belong to delivery.
