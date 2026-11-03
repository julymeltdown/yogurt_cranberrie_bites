# FE13 avatar editor

Status: **MOCK_READY**; bounded implementation delivered, FE13 acceptance remains
**NOT_RUN**. Source base `9f72a71a99091fe89411b180af3404be85d963f1`.
First feature commit `34b2246` adds draft/controller/UI. Shared FE12 prerequisite
`c2149dae63d21324179be3dca7df4bc79a003881` was explicitly authorized by delivery
and cherry-picked as local duplicate `c321279`; delivery already owns equivalent
`1db1680` and must **skip the duplicate dependency commit** when integrating the
feature commits. The follow-up commit containing this document adds the shared
adapter, lifecycle refinements and browser evidence.

Owned paths: `frontend/src/features/avatar/**` and this document only. No copied
Bigyeting UI, duplicate renderer, backend imports, source-art changes or dependency
changes. HTTP inputs are frozen C03 `contracts/core/v1/operations.json` and
`contracts/fixtures/core/{avatar-state,set-avatar-request,private-participant,error-version-conflict}.json`;
asset inputs remain C06 AvatarConfig/AvatarManifest v1. All implementation and
rendering UI is newly authored. The shared compositor is consumed from the
committed dependency, not copied from another checkout.

## Public composition

Import `AvatarEditor`, `createAvatarEditorController` and
`createReviewedAvatarPreview` from `features/avatar`.

- `createAvatarEditorController({auth, createPreview, onCommitted?})` takes only
  canonical session `privacy` and `request` methods; no second identity/cache,
  token persistence, Phaser instance or socket.
- `createPreview(scope, signal)` must resolve the retained authorized bootstrap
  for that exact user/event/privacy generation and return
  `createReviewedAvatarPreview({document: avatar_manifest, pin, binding?, resources, signal})`.
  The pin is `{manifest_id, manifest_version, content_hash}`. There are no art,
  review-receipt, baked-tone or fallback defaults. Missing approved art is
  unavailable. The adapter uses FE12 validation and `thumbnail` on the same
  compositor as world/profile and rejects fallback pixels for editor selections.
- Render `<AvatarEditor controller={controller} />`. The component starts/stops
  its mount lifetime and supports React strict effects. Root composition finally
  disposes the controller **before** disposing canonical auth/privacy.
- Stop releases every owned thumbnail/cache/atlas and aborts requests. Only the
  safe immutable draft remains in the same controller's memory for panel resume.
  Its privacy cleanup remains registered while hidden; identity/event/policy
  changes erase it synchronously. Resume checks server state and requires an
  explicit reconciliation choice before saving. Nothing goes to browser storage.
- `onCommitted(avatar, scope)` lets delivery update/invalidate world/profile from
  the same verified response. Check scope and monotonic avatar revision again at
  each receiving projection. Root route, shell cache and world wiring are
  delivery-owned and are **not** included here.

## State and transport

GET `/api/concert/events/{eventId}/me` verifies the event and canonical participant
and profile identities before reading `profile.avatar`. PUT `/me/avatar`
(`setConcertAvatar`) submits only `{expected_revision, config}` through the
canonical persistent HTTP transport. Save success must return a strictly newer
avatar revision and the submitted config. Avatar revision is independent of
participant/preferences revision. Drafts never publish an optimistic world
outfit; controls are single-flight while saving.

Presets/items/tones come only from the immutable current manifest. Unknown IDs,
wrong slots, unsupported baked tones and historical source-sheet IDs are
unavailable. Accessory `null` is supported. Failed saves keep drafts.
VERSION_CONFLICT and uncertain responses block writes until an explicit
latest-state read and user reconciliation. "Keep my draft" rebases the next
explicit save; "Use saved outfit" adopts server state. Neither replays a write.
Stale or equal-revision/different-config reads are rejected. Cancel restores
persisted state without writing, and cannot claim an uncertain command was undone.
Late initialization, thumbnail or save results cannot reattach pixels or publish
old-scope state, including reentrant teardown during notification.

## Verification — 2026-11-04

All commands below exited **0** using Node **24.22.2**, retained pnpm **11.7.0**
and existing lockfile resolutions. The first installation used
`pnpm install --offline --frozen-lockfile` in this isolated checkout.

From `frontend/`:

```text
node node_modules/vitest/vitest.mjs run src/features/avatar src/world/avatar src/shared/query/__tests__/privacy.test.ts src/shared/api/__tests__/http.test.ts
node node_modules/typescript/bin/tsc -b --pretty false
node node_modules/vite/bin/vite.js build
node node_modules/oxlint/bin/oxlint src vite.config.ts
node src/features/avatar/__tests__/browser-smoke.mjs
```

Combined **180/180** tests pass, including **24 FE13** tests. Focused checks cover
valid/unknown selections, draft/cancel, denied save, VERSION_CONFLICT, uncertain
network/malformed responses, explicit reconciliation, stale reads/identity,
single-flight writes, preview disposal/churn, policy teardown, hidden draft
privacy, interrupted-save resume and reentrant notification teardown. The build
retains its existing large-chunk advisory; editor root wiring is not claimed by
the production entry build. The browser fixture independently compiles/mounts the
new UI and shared adapter.

Actual native **Chrome 156.0.8078.80** passes widths **360/390/430** with canonical
Auth and HTTP against explicit test-only endpoints, real canvas composition and
RGBA equality to the shared profile thumbnail. It tests keyboard preset
activation/focus, 44px controls, no clipping, failed saves, 409 read/rebase,
simulated upstream 503 **after** server commit, same-controller panel resume,
ignored-abort selection churn and disposal. Product entry rejects synthetic art
without a reviewed binding. Asset requests contain no credentials or referrer;
there are zero page errors. Unit tests separately inject network rejection.

Evidence: `frontend/src/features/avatar/__tests__/evidence/native-01/report.json`
and `avatar-{360,390,430}.png`. The report records the prerequisite HEAD and exact
source hashes for the tested uncommitted follow-up; hashes match the delivered
source. The 390px screenshot was visually inspected. Images are clearly labeled
synthetic geometric fixtures and prove UI/lifecycle behavior only.

At repository root, `python3 scripts/mcr/check-plan.py` and `git diff --check`
also exit 0. Planning preservation is distinct from application qualification.
Only assigned feature/document paths changed in the feature commits.

## Remaining acceptance

Production selected-source derivatives/review receipt, live BE07 persistence,
root route and world/profile propagation, two-account real-server flows,
actual iOS/Android, QA06, AR03 and IN01 are **NOT_RUN**. The public API and honest
unavailable-art state are ready for delivery integration; this checkpoint does
not accept FE13, approve artwork or certify release readiness.
