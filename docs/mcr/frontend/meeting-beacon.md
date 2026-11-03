# FE20 — Physical meeting beacon

Date: 2026-11-04 (Asia/Seoul). Status: **MOCK_READY**. Real-server integration,
photo attachment, and IN02 acceptance are not established by this handoff.

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

## Photo boundary and remaining work

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
