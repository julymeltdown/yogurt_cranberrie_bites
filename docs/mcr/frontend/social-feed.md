# FE27 / FE28 social feed delivery

## Scope and provenance

Assigned base: `f0e44bd1bba943b02319f9d9a42433e253e8de6f`.
Worker branch: `codex/mcr-fe-social-feed`.
Only `frontend/src/features/event-feed/**`, `frontend/src/features/feed-thread/**`,
and this document are owned. All rendering and controllers are newly authored;
no Bigyeting rendering or code was copied. The pinned C04 v0.1.0 candidate comes
from `contracts/generated/mcr-domain.openapi.json`, `contracts/domain/v1/feed.schema.json`,
`common.schema.json`, and the generated operation registry at the assigned base.
The generated public schemas are interpreted strictly at decode time; unknown
fields, cross-event data, malformed profiles and unapproved notice states fail
closed. Dependencies: existing React, canonical FE09 session, FE02 privacy and
FE01 domain transport. No manifest, lockfile, contract or source archive changes.

## FE27 first slice

Status: `MOCK_READY` for controller slice; browser and live acceptance `NOT_RUN`.
Compact chronological list, four filters, explicit cursor loading, stable timestamp
and ID ordering, duplicate/cursor-cycle rejection, public related-target buttons,
source-stamped notices, independent notice detail, withdrawn-notice eviction,
loading/empty/denied/unavailable/offline/read-only states. No private activity is
projected. Privacy cleanup clears every slot synchronously and cancels work;
late former-account responses cannot repopulate it. Related navigation must be
reauthorized by the destination, which invokes `removeLink` if denied/revoked.

Validation (Node 24.22.2 / pnpm 11.7.0, 2026-11-04):

- `pnpm install --offline --frozen-lockfile`: exit 0.
- `pnpm exec vitest run src/features/event-feed/__tests__/controller.test.ts`: exit 0, 10 tests.
- `pnpm typecheck`: exit 0.
- `python3 scripts/mcr/check-plan.py` (repository root): exit 0; planning only.

Test fixtures stay under `__tests__`; production modules never import them.
Actual backend, two-user privacy, iOS/Android and notice operational verification
remain `NOT_RUN`. Images are explicitly unavailable; this slice establishes no
media-upload or image-viewing qualification. FE28 is still `IN_PROGRESS`.

## Root wiring owned by delivery

Import `createEventFeedController`, `EventFeed`, `EventNotice` from
`frontend/src/features/event-feed/index.ts`. Create one controller with the shell's
existing `auth` (`request` + `privacy`) and an `AccessPort`. Its `read()` returns
`{ available, readOnly }` from current bootstrap feed/event capability; its
`subscribe()` must fire on event lifecycle/capability changes. Fail closed before
bootstrap eligibility is established. The controller captures canonical user/event
and privacy generations itself. Do not make another session, cache or connection.
Call `start()` once after construction and `dispose()` on shell teardown.

- `/feed`: render `EventFeed`; supply `onPost(id)` -> `/feed/:postId`,
  `onCompose()` -> `/feed/new`, and destination-reauthorized `onLink(link)`.
- `/notices/:noticeId`: call `selectNotice(noticeId)` when route ID changes,
  render `EventNotice`, and supply retry for that exact ID.
- `removeLink({ target_type, target_id })` evicts a revoked related target.
  Global relationship changes must also flow through canonical
  `auth.privacy.changePolicy(...)` before any refresh.

Root routers and capability mapping are not modified by this worker. Notices
provide source and confirmation time; they do not invent venue guidance.
