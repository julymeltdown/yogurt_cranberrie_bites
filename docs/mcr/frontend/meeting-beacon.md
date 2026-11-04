# FE20 — Physical meeting beacon

Date: 2026-11-04 (Asia/Seoul). Status: **MOCK_READY**. Original FE20's frontend
criteria are now implemented under its frozen-contract/deterministic-fixture scope,
including photo selection, upload, attachment, save and fresh-controller reread.
Real-server integration, root photo enablement, devices and IN02 acceptance are
not established. The original text-only checkpoint below is historical; the
FE20-PHOTO-CLIENT continuation at the end supersedes its photo gap.

## Ownership and provenance

- Task: FE20, W07 `/world/check-in`, M06 `/meet/beacons/:beaconId`.
- Assigned branch: `codex/mcr-fe-physical-meeting`.
- Source base: `48458d597af73bd695c8d5813b123c6a96e55321`.
- Exact write scope: `frontend/src/features/meet-beacon/**` and this document.
  The coordinated folder supersedes the plan's `features/beacon/` destination.
- All feature logic, rendering, CSS, and tests are newly authored. No legacy
  rendering, source archive bytes, character art, backend code, or source-pack
  originals were copied or modified. Existing headless modules are imported.
- FE19 source: `70d89728a035ae448b0fa136fabcb14e44e5bc10` for the current places
  controller. The actual controller supplies the read-only place/map state.
- Canonical request/session: `7bfea962136df4c35be43571c657d140d2982079`;
  canonical FE02 privacy: `4207792d7f2ae1b1b6676e724e1712e43d72de43`.
- C04 beacon source: `b615e35bb07fb5797e6d7445a93aa6e1eeab7152`; receipt source:
  `904441752d56f96e5af16fbb1c7d0c2b88229e48`; operation registry:
  `ca9f955cd22f7e4c1da029d034c1923f1e98bb9d`.
- The pinned beacon/common/party/operation SHA-256 values and tested feature
  source hashes are in `__tests__/evidence/browser/report.json` under the feature.
  The delivery commit and its parent make the final patch ancestry reviewable.

Dependencies remain the retained React/TypeScript/Zod stack plus existing
session, privacy, and FE19 modules. No manifests or lockfiles changed. Testing
uses existing Vitest/Testing Library/Playwright and the existing conspicuously
synthetic C04 beacon and FE19 test fixtures. None is product venue authority.

## Behavior

`MeetingBeaconForm` renders a controlled place/audience/TTL/identification form,
requiring explicit physical acknowledgement. The operator maximum is 1–900
seconds, initially 900; event cutoff can shorten expiry. Only reviewed, open
physical places with a safe source reference and qualified revisions are offered.
No coordinates, distance estimates, GPS, route safety, upload field, or inferred
arrival are sent. The audience is selected from current qualified choices, not
entered as arbitrary user or party identifiers.

`MeetingBeaconDetail` displays audience, owner confirmation time (`updated_at`
after the acknowledged create/update), expiry, and beacon/place/map revisions.
An invalidation immediately removes the old private instructions. A later
revision is visibly superseding and needs explicit review before it becomes
current. Old revisions cannot overwrite the current watermark. Expired/revoked,
failed authorization, and stale place/map states remove instructions and photos.
Generic `RESOURCE_UNAVAILABLE` never invents a specific revocation or expiry fact.

Create/update/revoke use only canonical `SessionController.request`, registered
domain operations, and canonical UUID idempotency metadata. Each command is sent
once. Unknown, 202 pending, or malformed-success outcomes lock further writes.
The user can explicitly invoke `getConcertCommandStatus` using the same actor,
event, operation, and key. A successful receipt is followed by a fresh authorized
beacon read; the receipt itself cannot restore private guidance. Missing, foreign,
unparseable, and retired receipts cannot unlock the mutation or trigger replay.
Revocation requires a separate UI confirmation.

FE02 `privacy.run` fences all network work. Synchronous registered cleanup clears
drafts, details, photos, command bindings, and observers on policy/identity/event
changes; late responses cannot repopulate them. The feature owns no access token,
second identity, QueryClient, world socket, Phaser instance, or policy implementation.
Server authorization and invariants remain authoritative.

## Composition API

The feature barrel exports `createMeetingBeaconController`, `MeetingBeaconForm`,
`MeetingBeaconDetail`, `ExplicitArrivalControls`, and their public types.

Construct one shell-owned controller with:

```ts
createMeetingBeaconController({
  auth,       // canonical session + privacy + request
  places,     // existing FE19 controller: read + subscribe
  authority,  // MeetingAuthorityPort: read + subscribe
  now,        // qualified server-time estimate, monotonic anchor; null if unavailable
  photos,     // optional qualified display port; omit until available
})
```

The composition owner supplies and revokes `MeetingAuthority` only after current
server policy checks. It binds the privacy scope, lease expiry, read/write gates,
event cutoff, operator TTL, audience choices, and current per-place revisions.
It must publish membership removal, blocking, event closure, map/place changes,
and other revocations through the canonical privacy/authority lifecycle. A broad
event capability alone does not prove permission to read a private beacon.
Missing, expired, or cross-scope authority disables the flow. No production
adapter fabricating these grants is included.

FE19's list/detail `revision` is the catalog revision. It is not silently mapped
to C04 `meeting.place_revision`. Until the owner supplies a server-qualified
per-place mapping, authoring/detail qualification remains unavailable. The map
revision comes from the current FE19 map. This seam needs a concrete BE08/BE09
adapter during integration.

Call `start()`, `select(beaconId)`, `refresh()` for explicit read recovery, and
`invalidate({beaconId, revision, state?})` with authenticated scoped invalidation
metadata. Use `editCurrent()` before an update form; it resets acknowledgement.
`confirmRevision()` explicitly accepts superseding guidance. Keep the controller
alive through panel navigation while a command is unresolved; dispose it when
its owning shell ends. Privacy cleanup is always immediate.

`ExplicitArrivalControls` receives the current `MeetBeacon`, display phase,
current privacy scope, qualified clock, pending flag, and an expiring authorized
binding containing party ID, beacon ID/revision, and separate party meeting
revision. It emits only:

```ts
{ state: 'HERE', meeting_revision: 8, physical_presence_acknowledged: true }
```

States also include `ON_MY_WAY`, `MET`, `NEED_HELP`, and `LEFT`. No callback runs
on render, movement, or revision change. Each report requires a fresh explicit
acknowledgement; changing scope/revision resets it. FE23's owner rechecks current
membership, meeting revision, self revision, and policy and sends the registered
`setConcertRally` operation. FE20 does not own rally HTTP mutations.

## Original text-only checkpoint: photo boundary and remaining work

The delivery owner confirmed the pinned C04 `MeetingDescription` is text-only.
It has no image/media reference. Upload attachment and persistence therefore
remain a required product gap, pending a separately coordinated contract change
and FE04 upload/privacy binding checks. FE20 neither invents a field nor duplicates
FE04's upload owner.

The optional `MeetingPhotoPort` displays an already authorized photo only when
its scope, audience, beacon/revision, place, expiry, permission, review, and safe
HTTPS source match; FE19 place photo permission must also be `GRANTED`. It is
display-only and has no production source here. Missing qualification renders
no image. There are no placeholder or synthetic production photos.

No private command persistence is introduced. Status reconciliation is bounded
to the current shell/controller; reload cannot recover an in-memory unknown key.
A reload recovery adapter needs a separately qualified privacy/lifetime decision.
No mutation is automatically replayed. Root route registration, real backend
authorization/race/DB-time checks, two-account IN02 flows, actual mobile devices,
and production photo/venue rights remain **NOT_RUN** or unresolved integration
work. This feature handoff does not mark the whole FE20 task **ACCEPTED**.

## Validation

Pinned runtime: Node 24.22.2, pnpm 11.7.0 from `/Users/lhs/Desktop/mcr-tools`.
Commands ran in this assigned checkout; frontend commands ran from `frontend/`.
Evidence is under `frontend/src/features/meet-beacon/__tests__/evidence/`.

| Command | Result |
| --- | --- |
| `pnpm install --frozen-lockfile --offline` | Exit 0; retained lockfile unchanged |
| `pnpm typecheck` | Exit 0 |
| `pnpm test src/features/meet-beacon/__tests__ src/features/places/__tests__ src/shared/api/__tests__ src/shared/query/__tests__ src/features/auth/__tests__/domain-request.test.ts` | Exit 0; 325 tests / 9 files, including 36 FE20 tests |
| `node src/features/meet-beacon/__tests__/browser-smoke.mjs` | Exit 0; Chromium 156.0.8078.80; 360/390/430px |
| `pnpm build` | Exit 0; baseline bundle-size warning remains; root does not yet register FE20 |
| `pnpm exec oxlint src/features/meet-beacon` | Exit 0 |
| `python3 scripts/mcr/check-plan.py` | Exit 0; planning/source checks only |
| `python3 scripts/mcr/check-boundaries.py --base 48458d597af73bd695c8d5813b123c6a96e55321 --lane frontend --allow-doc docs/mcr/frontend/meeting-beacon.md` | Exit 0; exact path/import review also performed |
| `node scripts/mcr/check-ui-origin.mjs` | Exit 0; no legacy bytes/import violations |
| `git diff --check` | Exit 0 |

Focused cases cover qualified audiences/TTL/cutoff, stale/place/map revisions,
expired/revoked/denied reads, explicit arrival, synchronous privacy cleanup,
noncooperative late requests, denied/unknown writes, safe receipt recovery, and
fully bound photo presentation. The browser fixture is conspicuously synthetic.
It verifies keyboard reporting, touch targets of at least 44px, no horizontal
overflow, moved/expired/revoked states, unknown-command locking, privacy cleanup,
and disposal. Its virtual-movement button is a fixture counter, not a real world
or multiplayer acceptance test. Screenshots were visually inspected; new CSS has
no decorative left borders, rails, or accent bars.

The first focused run exposed a test recorder retaining a consumed fetch Request;
the harness now records a clone. The first boundary invocation omitted required
CLI flags and exited 2; the correctly scoped invocation passes. Neither initial
failure was treated as passing evidence.

## FE20-PHOTO-CLIENT continuation

This completes the remaining original FE20 **client implementation**. It is not
a new product-scope reduction or a draft-only completion. The frontend task is
satisfied at **MOCK_READY** within its specified frozen-contract/fixture scope;
no required feature-local photo behavior is left unimplemented. Live backend,
root service admission, real venue/photo rights, IN02 and physical-device proofs
remain separate integration/acceptance responsibilities, not missing client code.

### Exact ownership, contract and reuse provenance

Preserved checkout, new branch `codex/mcr-fe-FE20-PHOTO-CLIENT`, assigned fresh
base `1249430fa09537fa7a5b3499916e29ad0683fe32`. Original feature ownership was
explicitly released by its stopped writer: `features/meet-beacon/**` and this
document only. Later explicit disjoint grants added **only**
`shared/media/types.ts`, `shared/media/contract.ts`, and the purpose-type/import
hunks in `features/event-feed/FeedImage.tsx` and `features/feed-thread/media.ts`.
The four-file support commit is `591040ff`; it is separate from photo behavior.
An additional explicit sole-writer transfer grants only
`shared/api/domain-operations.generated.ts`: the unchanged canonical
`scripts/mcr/generate-domain-transport.py` regenerates that file, then its
`--check` verifies it. No other generator or backend/OpenAPI output is written.
Its new BEACON enum follows the canonical published feed schema. The two consumer
type projections reference MediaPurpose; their existing POST-only runtime guards
remain unchanged and have a concrete negative BEACON display/attachment test.
No app/root/meeting-entry, shared controller/transport, generator, backend,
manifest, lockfile, artwork or `docs/source/` bytes are changed.

BE meeting-photo developer `01a1a202-3b96-7822-ab54-2f7d29aa42e5` published the
first agreed input at `89ce3df69299ed04b57413fd0510705197a27ba2` and final frozen
`meeting-photo/v1` plus C04 schema source at `17fd2e93`, delivered through
`a3526254a716e14e3134de551c3b277379a1096f`. Media developer
`01a1a213-03de-72d3-9ecf-2f72b574cf82` confirmed the public semantics. The exact
source document is `backend/docs/api/meeting-photo-contract.md`. Its deterministic
JSON is copied byte for byte from
`backend/services/concert-service/tests/meeting_photo_support/contract-fixtures.json`
to feature-owned `__tests__/photo-client/frozen-contract.json`; SHA-256
`0a8b39b3c4fb36170a5d3a2d7082e15d809482983800d3474f1d7bf75792340e`.
This is public fixture data, never a backend implementation import.

Published beacon schema SHA-256:
`183186acc1c3ac916e1011c022174041c6557a148347dd135ffaf6ee84385573`.
Published feed schema SHA-256:
`f9b4b8912b2bf4513ab474ad690efdeaa23d64f97b220f9626c5b37c5e2e99ed`.
The agreed additions are optional nullable `meeting.media_id`, separate
Create/Update `photo_permission_acknowledged` required true for every non-null
attachment (including retention), and dedicated `purpose: BEACON`. An omitted or
null media ID on a complete update removes the attachment. Physical-presence
acknowledgement remains independent. Existing operation IDs and receipt shapes
are unchanged. Generated OpenAPI/types and runtime/default-root enablement are
separately owned; their absence is not misrepresented as published live support.

All new rendering is feature-authored. The client imports the **unchanged**
FE04 `createImageUploadController`, `createMediaUploadTransport`, `prepareImage`,
image policy, and `createMediaPreviewController`; it does not duplicate their
compression worker, upload state machine, retries, byte/MIME/dimension checks,
receipt recovery or Blob reader. Unit PNG helpers adapt existing synthetic FE04
fixture bytes. Browser artwork is a generated geometric test image, not product
or venue artwork. Exact imported source hashes, changed paths, commands/exits and
commit ancestry are recorded in `__tests__/photo-client/evidence/qualification.json`
and the delivery receipt.

### Implemented behavior and original FE20 disposition

| Original frontend criterion | Delivered behavior and evidence |
| --- | --- |
| New audience/TTL form and identification | Original qualified physical-place/audience/TTL/identification form retained; text-only requests remain compatible. Existing controller/UI regressions pass. |
| Photo/identification | Optional configured client selects a bounded image, runs FE04's actual worker preparation, owns a private Blob preview, requires a separate photo-rights assertion, uploads as BEACON only, and attaches only a current READY identifier. No guessed IDs or purpose aliases. |
| Save and persistent reread | Create/update send the agreed media ID and acknowledgement. Success and receipt recovery fetch the current authorized beacon before metadata/content. A newly constructed controller, without the local draft/upload result, rereads the saved ID and displays newly fetched/decoded Blob bytes. Replacement/removal persist through the next server-fixture revision. |
| Last-confirmed, moved/stale/revoked/expired | Existing revision/owner confirmation/expiry safeguards retained. Old photo readers and URLs clear on refresh, invalidation, replacement, revocation, expiry or place permission withdrawal; late bytes cannot restore them. |
| Explicit arrival; virtual motion is not arrival | Original ExplicitArrivalControls remains unchanged and its acknowledgement/revision/virtual-motion regressions pass. Neither selecting nor uploading/saving a photo emits arrival. |
| Canonical identity, audience and privacy | A feature adapter captures the full policy epoch, uses canonical Auth request/privacy/session, fences FE04 preparation/requests/previews, aborts and revokes synchronously, and never recaptures an old owner after policy/account/event changes. FE19 reviewed place and photo permission remain required. |
| Cancellation, failure and uncertain writes | Selection/removal cancels local work and revokes previews. Failed or pending/unknown uploads cannot be attached or saved. Unknown uploads retain their operation/key across target-navigation attempts; status recovery never replays a mutation. Unknown beacon saves reconcile their original receipt then reread the persisted resource and current image. Removing an unconfirmed upload abandons the local draft; it does not claim a server delete. |

Existing `MeetingPhotoPort` remains display-only for historical text-only callers;
it never grants attachment or save authority. Any beacon with a saved media ID
uses only the authenticated metadata+binary Blob flow and cannot fall back to a
remote photo URL. Protected API paths never become image `src`. No title, private
filename, raw error, credential or upload key is rendered. No draft, bytes, URL,
private command or identity is persisted in browser storage.

The optional new composition argument is:

```ts
createMeetingBeaconController({
  auth, places, authority, now,
  photoClient: { contract: 'meeting-photo/v1' },
})
```

Omitting `photoClient` keeps existing text-only behavior and an honest unavailable
attachment message. The declaration is a consumer-version opt-in, not server
authority or a readiness producer. Actual root composition remains untouched.
The feature exposes selectPhoto/uploadPhoto/checkPhotoUpload/retryPhotoPreparation/
removePhoto; it owns and disposes their scoped FE04 instances. Existing route
owners must retain the feature controller while a command is unresolved, as
already documented for the text-only command lifecycle. A same-session `block`
policy change advances the epoch while leaving its gate open; `suspended` is the
closed-gate browser case. Both invalidate the original captured photo owner.

### Own qualification and remaining owners

The final focused suite passes **205 tests in 14 files**, covering the original
FE20 text/TTL/audience/arrival/revision protections, new photo flow, FE04, retained
media ownership, Feed/thread compatibility and canonical Auth domain requests. Native Chrome passes 360/390/430
journeys using the actual compression worker (PNG to sanitized WebP), upload
controller, canonical request construction, authorized metadata/binary reader and
real Blob URLs. Each saves a photo and rereads it through a fresh controller,
without a second upload; suspension clears all draft/detail images and revokes
every created Blob URL. A fourth 390px case verifies unknown finalize UI locks,
status checking without replay or beacon save, and explicit local removal.
Deterministic HTTP storage, identity, audience and place/photo permission are
conspicuous synthetic fixtures; this is not server durability, real permission,
live authorization, device or multiuser proof. Screenshots were visually inspected.

Typecheck, scoped lint, production build, planning provenance and UI-origin checks
pass; the existing large-chunk build warning remains. The first build lacked two
already-tracked Auth print PNGs excluded by this checkout's sparse patterns.
Materializing those unchanged tracked assets fixed the build; no Auth source or
asset was edited. Initial harness failures (missing synthetic origin, denied
fixture photo permission, unstable React fixture snapshot) were corrected and
rerun; none is reported as passing evidence. Exact logs are feature-owned under
`__tests__/photo-client/evidence/`.

**Remaining frontend behavior within original FE20 feature scope: none.** Root
integration/actual version admission belongs to the existing meeting-entry and
ProductRoot owner `01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08`, with no transferred root
write scope here. BE202 owns the real meeting-photo producer/atomic persistence;
media213 owns authoritative media lifetime/erase/read; ProductRoot retains backend/
OpenAPI composition and the real readiness projection. The explicitly transferred
frontend transport projection is regenerated here with only the feed hash header
changed (108 operations unchanged), and its canonical drift check passes. Those owners
must qualify actual live authorization, revision/audience/expiry binding and
atomic old-byte removal before enabling the real root service. Operator venue/
photo-rights decisions, IN02 and devices remain their separate acceptance gates.
The original FE20 client task is complete at MOCK_READY; INTEGRATED/ACCEPTED is
not asserted by these fixtures. Direct non-force master publication and actual
persistent-goal completion receipts accompany the committed handoff.
