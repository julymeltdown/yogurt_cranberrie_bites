# Meeting-entry lifecycle adapter

Date: 2026-11-04 (Asia/Seoul). Status: **MOCK_READY**. This is the bounded
post-FE20 composition handoff; production route registration and real IN02
acceptance remain **NOT_RUN**.

## Assignment and ancestry

The delivery owner granted exactly `frontend/src/app/meeting-entry/**` and this
document in a new isolated checkout, after FE20's committed handoff. The source
FE20 checkout is stopped and clean. This checkout is
`/Users/lhs/Desktop/mcr-worktrees/fe-meeting-entry`, branch
`codex/mcr-fe-meeting-entry`, base `ee41a3b61183a224a784d303210fcf2ef55737a5`.
That integrated base contains FE20 source
`4c7837423135416d8412d63e5073deb8fcc5f6be` plus the current media/party
prerequisites. The plan, assessment, and execution documents have no changes
between the previously inspected FE20 base and this adapter base.

All adapter code, route-ready rendering, and tests are newly authored. It imports
the committed FE20 public API, existing FE19 type, canonical auth/privacy types,
and the existing canonical route-ID validator. Tests reuse FE20's conspicuously
synthetic canonical Auth/FE19 fixture owners and dispose its initial beacon
controller before creating the adapter. No legacy code/art, backend code,
source-archive bytes, or character originals were copied or modified. No package,
lockfile, existing root/router, shared contract, or infrastructure file changed.

The adapter introduces no HTTP client, identity, policy engine, QueryClient,
Phaser instance, socket, clock estimator, upload owner, or photo purpose. Its
only dependencies are existing committed modules and retained React/Router test
tooling. The final delivery commit and its parent identify the exact patch;
tested source hashes and the base SHA are recorded in the browser report.

## Public API

The barrel exports:

- `createMeetingEntry({ auth, release })`: lifecycle bridge, `read`, `subscribe`,
  `withdraw`, and `dispose`.
- `MeetingEntryFeatures`: mount once inside the authorized shell, outside panels.
- `MeetingCheckInPanel`: route-ready W07 `/world/check-in` presentation.
- `MeetingBeaconPanel`: route-ready M06 `/meet/beacons/:beaconId` presentation;
  it accepts route params or an explicit `beaconId` prop.
- `useMeetingEntry`: read-only current feature-owner access for separately owned
  rally/pickup composition.
- `MeetingEntryRelease`, `MeetingEntryAuth`, `MeetingClockPort`,
  `MeetingInvalidation`, and bridge/view types.

Pass the **existing canonical** auth `session`, `privacy`, and `request` surface.
A `MeetingEntryRelease` contains the current privacy scope and shell abort
signal, actual FE19 controller's `read`/`subscribe`, explicit FE20
`MeetingAuthorityPort`, scoped clock port, and optional qualified photo and
authenticated invalidation ports. It is a prepared runtime input, not a wire
DTO or a fabricated server authorization result. `auth: null` or `release: null`
renders safe unavailable UI and dispatches no meeting request.

The clock port returns `{ scope, nowMs }` from a qualified server-time estimate
maintained by its existing owner using a monotonic anchor; `null` means absent.
The adapter never falls back to `Date.now`, bootstrap capabilities, virtual
coordinates, GPS, or a synthetic clock. A clock scope mismatch withdraws
presentation. Temporary clock/authority loss can reopen under the same captured
lease after explicit current qualification; its unresolved command lock survives.

The scope and source-port references are captured once. Identity changes, FE02
policy changes, external shell abort, explicit withdrawal, and disposal
permanently end that lease, scrub the feature, abort work, and remove owned
subscriptions. A producer cannot restamp an old entry with a replacement user or
policy generation. The adapter never disposes the canonical Auth or FE19 owners.
Partial subscription setup failures also release the feature's acquired work.

No beacon read occurs merely from constructing the bridge or opening the shell.
Panel selection explicitly reads the requested target through FE20's canonical
request/privacy path. Missing qualified physical places or place revisions keep
the adapter unavailable. Invalid beacon IDs dispatch no meeting request. A
route-ID change hides the old target before its selection effect runs. An
in-flight command completing for another target cannot render that target as the
linked meeting. Panel navigation retains the same feature owner and its
unresolved-command lock; shell withdrawal disposes it.

`available` means the current feature owner has a qualified read gate, clock,
and at least one qualified physical place. It is not a certification of any
private beacon or a write grant. FE20 still qualifies the selected beacon's
audience, current map/place revisions, expiry, and write authority; the server
remains authoritative. The adapter never turns capabilities into private
meeting permission or creates arrival mutations. FE23 independently reauthorizes
explicit arrival intents.

## Required production inputs and root handoff

The delivery owner must supply actual qualified sources and register the
provider/panels. Existing root files remain outside this assignment. The intended
composition is:

```tsx
<MeetingEntryFeatures auth={canonicalAuth} release={qualifiedMeetingRelease}>
  {existingPanelOutlet}
</MeetingEntryFeatures>
```

Register `MeetingCheckInPanel` at W07 and `MeetingBeaconPanel` at M06. Preserve
the one persistent world shell; do not mount the provider separately inside
each panel. Pass `null` until all mandatory sources are current and qualified.
The delivery handoff requests this exact root registration separately and does
not edit the existing router.

The public per-place revision projection is still missing. The BE09 owner
confirmed `concert_place_current_state.revision` is the authoritative
`place_revision`, distinct from the event catalog revision. FE19's catalog
`revision` must **never** substitute for it. The adapter requires explicit FE20
`placeRevisions`; no mapping or new endpoint is invented here.

The C04 beacon description remains text-only. FE04 supports POST/GIVEAWAY/REPORT
media purposes and has no meeting-photo purpose. Optional reviewed photo display
stays a qualified FE20 input only; upload attachment/persistence requires the
separate contract/media/privacy binding work recorded in the FE20 handoff. No
synthetic or placeholder product photo is introduced.

Unknown-command keys remain memory-only in the retained shell owner. This
adapter validates panel-navigation recovery, not reload recovery or private
persistence. Actual server/DB-time authorization, operator TTL/cutoff sources,
venue provenance, production root registration, two-account IN02, and devices
remain unqualified. No accepted-task or release count changes follow from these
fixture checks.

## Validation and evidence

Runtime: Node 24.22.2 / pnpm 11.7.0 from `/Users/lhs/Desktop/mcr-tools`.
Frontend commands ran in this assigned checkout's `frontend/`. Evidence paths
are relative to `frontend/src/app/meeting-entry/__tests__/evidence/`.

| Command | Result |
| --- | --- |
| `pnpm install --frozen-lockfile --offline` | Exit 0; lockfile unchanged |
| `pnpm typecheck` | Exit 0 |
| `pnpm test src/app/meeting-entry/__tests__ src/features/meet-beacon/__tests__ src/features/places/__tests__ src/shared/api/__tests__ src/shared/query/__tests__ src/features/auth/__tests__/domain-request.test.ts` | Exit 0; **369 tests / 11 files**, including **22 adapter tests**; `combined-tests.log` |
| `node src/app/meeting-entry/__tests__/browser.mjs` | Exit 0; Chrome 156.0.8078.80, 360/390/430px; `browser/report.json` |
| `pnpm build` | Exit 0; inherited bundle-size warning remains; `build.log` |
| `pnpm exec oxlint src/app/meeting-entry` | Exit 0 |
| `python3 scripts/mcr/check-plan.py` | Exit 0; planning checks only |
| `python3 scripts/mcr/check-boundaries.py --base ee41a3b61183a224a784d303210fcf2ef55737a5 --lane frontend --allow-doc docs/mcr/frontend/meeting-entry.md` | Exit 0; manual exact path/import review also performed |
| `node scripts/mcr/check-ui-origin.mjs` | Exit 0; no violations |
| `git diff --cached --check` | Exit 0 |

Tests cover absent runtime/authority/clock, cross-scope sources, missing place
revisions, scoped invalidations, permanent policy/shell withdrawal, ignored-abort
late reads, partial registration failure, preservation of Auth/FE19 owners,
malformed links, route-ID changes, StrictMode setup/disposal, and unknown outcomes
across panel navigation. Browser checks exercise the actual adapter/provider and
panels in a **synthetic MemoryRouter fixture**, not the production router. They
verify keyboard navigation, no horizontal overflow, 44px minimum controls, one
live subscription set after StrictMode setup, private cleanup on clock/policy
loss, no mutation replay during navigation, and zero remaining subscriptions
after withdrawal. The 390px screenshot was visually inspected.

An initial typecheck caught a widened literal in the empty places projection;
the projection now has the explicit existing `PlacesState` type. One log capture
was attempted before its evidence directory existed; no tests ran in that
attempt, and the subsequent actual combined run above passed. These were not
counted as passing checks.
