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

## FE28 text/thread and media consumer slice

Status: `MOCK_READY` for text/thread against deterministic HTTP;
`IN_PROGRESS` for complete FE28 pending qualified FE04 media composition.

`frontend/src/features/feed-thread/index.ts` exports
`createFeedThreadController`, `FeedThread`, `PostComposer`, and typed
`ThreadOptions` / `ImageUploadPort`. New modules implement protected detail and
comment pagination; post create/edit/delete, comment create/edit/delete and
reaction/unreaction; owner controls, revision-bearing requests, safe report entry,
and explicit deletion confirmation. Post and reply forms keep separate drafts.
Only server-confirmed results clear submitted text. Unknown outcomes retain a
frozen command body and UUID key, lock resubmission, and expose explicit bounded
`getConcertCommandStatus` reads. There is no automatic mutation replay or polling.
Pending/foreign/invalid/retired receipts never claim success. Status receipts must
match actor, event, operation, key and known resource. Definite version conflicts
retain the edit; reload current data then explicitly restart editing to obtain
a new revision. Deleted/denied parent or comment reads evict cached descendants.

Same-identity read-only transitions retain an outstanding command identity and
draft for explicit recovery; a privacy/session generation change synchronously
cancels work and clears all drafts, attachments, results and command state.
Ordinary read-only pages expose no writes. Production fixtures are absent.

For `/feed/new`, call `thread.select(null)` and render `PostComposer`.
For `/feed/:postId`, call `thread.select(postId)` and render `FeedThread`.
For `/feed/:postId/edit`, fetch the current detail then call `beginPostEdit()`
and render `PostComposer` only when owner validation succeeds. A `false` return
from `select` means an unresolved command is retained: keep the existing panel
and its status recovery action. Root back/navigation needs this same check;
do not remount the controller and lose the uncertain identity. Create the
controller once per active shell and dispose on teardown, alongside feed.
Supply reauthorized `onLink`, `onPost`, `onEditPost`, and an FE36-owned
`onReport({ target_type, target_id })`. Reports are entry callbacks, not fake
successful report submissions. Root routing and FE36 are delivery composition.
After writes, the thread reloads its authoritative detail. Refresh the feed when
returning to it. Images already on posts are explicitly labeled unavailable until
a qualified protected media reader is supplied; no invented URL is used.

Optional `ThreadOptions.media` accepts the FE04 canonical-session
`createImageUploadController` via its structural port. FE04 owner:
`01a1a202-375e-7401-bfa1-ff8721e90ed6`. The composer offers selection, preview,
explicit upload, retry of preparation, explicit uncertain-status recovery,
READY-attachment confirmation, removal, and discard. It blocks publishing while
an image selection remains unresolved. `attachReadyMedia()` accepts only the
strict generated `Domain_feed_Media` shape with purpose POST, matching current
canonical generation/user/event and exact same-event protected read path; it
adds at most four distinct media IDs. Privacy cleanup calls media.cancel()
synchronously. Delivery must inject the qualified FE04 controller/transport;
this worker adds no alternate fetch, token, or session owner. Image
upload/service acceptance remains unproven until that composition is run.

Validation at this slice:

- Focused controller/UI Vitest: 23 tests across FE27 and FE28, exit 0.
- `pnpm typecheck`: exit 0.
- `pnpm build`: exit 0 (existing >500kB chunk warning; root does not yet import
  these feature exports, so this is compilation/build evidence, not route wiring).
- `node src/features/feed-thread/__tests__/browser-smoke.mjs` (frontend cwd):
  exit 0, installed Chrome, widths 360/390/430, 42 same-origin fixture requests.
  Keyboard publish, committed-but-503 response, retained draft/key, explicit
  status recovery, thread/comment/reaction, synchronous privacy eviction and
  read-only composer removal all exercised. No page errors or horizontal overflow.
- Browser evidence: `frontend/src/features/feed-thread/__tests__/evidence/browser-report.json`
  and `thread-390.png`; the service is explicitly test-only, no real backend proof.

All live BE31/32/24 endpoints, two-user IN03, actual iOS/Android, production media
and final acceptance remain `NOT_RUN`. BE31/32 owner is
`01a1a205-3f89-78c3-aa67-719c4bb40b8d`. No master push was performed by this worker.

Follow-up hardening: feed order compares exact UTC fractions (including
submillisecond precision) before the UUID tie-breaker; equivalent timestamp
precision is treated as the same instant. Added a focused regression probe.
Both `EventFeed` and `FeedThread` optionally accept `renderMedia(mediaId, post)`
(`RenderPostMedia` export). Delivery can supply the qualified FE04 protected
media reader using the canonical identity; the callback receives only the
strict public post/ID. Without it, attachments stay explicitly unavailable.
This is an integration seam, not proof that protected image reading has run.

Delivery ancestry: first FE27 worker commit `97ef9fb` (base `f0e44bd`),
then FE28 worker commit `7e4f533` (parent `97ef9fb`). Delivery reported the FE27
integration as `9f72a71`; worker does not infer final remote delivery from this
message. Follow-up exact-fraction ordering test: 11 FE27 controller tests exit 0;
total focused controller/UI cases now 24. Typecheck and focused oxlint exit 0.
`node scripts/mcr/check-ui-origin.mjs`: exit 0, 311 files scanned, zero static
origin/import violations. This guard does not replace QA02 manual visual review.

Protected image display consumer: `FeedImage` and `MediaPreviewPort` are exported
from event-feed. Within `renderMedia`, create one FE04
`createMediaPreviewController({ session, request: auth.request, mediaId, purpose: 'POST' })`
per mounted image lifetime. Pass a stable `createPreview(mediaId)` factory to `FeedImage`, alongside canonical
`auth.privacy`, the media/event IDs and suitable alt text. It loads protected
metadata/bytes through the qualified port, displays only a READY POST blob URL
matching current canonical scope and exact read path, cancels on privacy cleanup,
and disposes on unmount. A failed image never hides post text. The component constructs and disposes inside its effect, so StrictMode setup/cleanup creates a fresh lifetime. Keep the factory callback stable across parent renders. FE04 owns byte validation and URL revocation.

Protected display consumer checks: five actual UI tests pass (exit 0), including
blob eviction on privacy invalidation and a fresh disposable preview controller
after StrictMode's setup-cleanup-setup. The UI fixture explicitly enables React's
act environment; the previous qualification warning is removed. The media
factory is stable per root composition; the component owns individual controller
lifetimes. Total focused cases: 26. Real FE04 controller composition still pending.

## QA cache resurrection fixes

Targeted QA reproduced two in-flight races: an older feed page could restore a
removed related-target link, and an older notices page could restore a notice
whose detail had become 404. Both were cached-response resurrection; no live
backend authorization bypass was established. The initial linked-target test
used an empty links fixture and did not exercise revocation; it now uses a real
PARTY link.

The controller records denied related targets and inaccessible notice IDs for
the current privacy scope, filters both captured prior rows and new page rows
at settlement, and clears these records only when that scope is replaced. This
covers delayed pagination and refresh without aborting unrelated safe reads or
replaying writes. Four real nonempty regression cases cover both races and both
read paths. FE27 controller tests: 15/15 pass, exit 0; typecheck exit 0.

## Actual FE04 consumer qualification

Synced the isolated worker branch onto delivery's `ee41a3b`, skipping already
integrated duplicate worker commits. This combined baseline contains FE04
`795f46b` as `0952149`, binary HTTP `4d65ed4` as `a56f043`, and the FE27/FE28
consumer commits. No other lane's source was edited by this worker.

`node src/features/feed-thread/__tests__/media-browser.mjs` (frontend cwd):
exit 0 on Chrome `156.0.8078.80`, width 390, StrictMode. Uses the actual FE04
image preparation worker, upload controller, canonical `auth.request` binary
transport, protected preview controller, and FE28 composer/thread/FeedImage.
Only its same-origin backend is test-only middleware. A synthetic test image
was prepared as WebP (562 bytes), SHA-256 matched the binary request, and exactly
one init, byte upload, finalization and post write occurred. Exactly one
metadata and one protected content GET completed. Only after validated READY
and explicit attachment confirmation did the composer send the media ID. The
thread displayed a 64px image from an ephemeral blob URL; privacy invalidation
removed it and a fetch of that blob URL failed, proving revocation. No page
errors or horizontal overflow were observed. Evidence:

- `frontend/src/features/feed-thread/__tests__/evidence/media-browser-report.json`
- `frontend/src/features/feed-thread/__tests__/evidence/media-thread-390.png`
- Reproducible harness: `__tests__/media-browser.mjs` and
  `__tests__/fixtures/media-preview.{html,tsx}` under feed-thread.

FE28 bounded implementation and fixture qualification are `MOCK_READY`, including
actual media consumer composition; this is not live BE24/31/32 or two-user IN03
acceptance. Actual devices, release acceptance, report submission integration
and root route composition remain with their assigned owners and are `NOT_RUN`
for this worker. The prior pending-controller-source limitation is resolved.
Concrete cache review fixes are in `7fecec0`; exact independent verification is
being performed by QA, with any further findings retained in this goal.

The thread controller also exposes `removeLink(target)` with the same current
privacy-scope revocation record, applied on every detail refresh. Destination
callbacks must notify both feed and thread controllers when access is denied;
an in-flight detail read cannot restore the related button. A real nonempty-link
regression covers that delayed refresh. This extends the QA fix to detail parity.

Independent targeted QA executed the exact `7fecec0c69e6a0ce9012070782aebecd93f7aa91`
archive snapshot (not an uncommitted approximation): 32/32 tests across three
files pass, exit 0. Both originally failing canonical cache-race probes are
green; the archived controller SHA-256 matches git-show
`0d528342c21b6d1e3c9a7038c6a243d2fb4d32e9f2e00beda6ac7c3091d83c5b`.
No further application fix was requested by that bounded report. Delivery
reported integration as `cf7836a`; this worker does not claim a master push.

Final affected validation: FE27 15 cases, thread controller 11 cases, UI 5 cases,
all exit 0 (31 total); canonical TypeScript check exit 0; focused oxlint exit 0;
source-origin guard exit 0 (432 files, no static violations); diff check exit 0.
The text browser evidence and actual-media browser evidence are committed with
reproducible test-only HTTP harnesses. The worker's bounded assignment is ready
for committed delivery handoff and stoppage, distinct from whole-task acceptance.
