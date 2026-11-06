# Task 25 — Complete the venue Map/Friends client states

Status: **IMPLEMENTED / DEVELOPER-VALIDATED**. Base `cb44bb77c`
(`feat(venue): wire production shell ports`), branch `codex/mcr-exec-w4-map`.
No backend, protocol, lockfile or generated-contract change. No push/merge.

## Delivered behavior

- **Permission request and denial.** Device permission stays separate from sharing
  consent: no geolocation call happens on render or before an explicit consent
  checkbox plus "Allow phone location". A denied browser permission clears the
  local watch, stops the self marker, asks the server to PAUSE the active lease,
  and renders "Phone permission denied" with a reviewed retry. A server refusal
  renders "Sharing not authorized" and never starts a watch.
- **Friend clusters and selection** use only `VisibleLocation` rows the server
  returned for this actor. Checked-in friends at one place cluster from their real
  projected points; selection lists exactly the clustered actor IDs. A friendship
  never supplies a point or a request right by itself.
- **Request / pause / revoke.** ASK LOCATION creates metadata only. Pause and
  revoke send `viewer_ids: []` (L9 contract), clear the selected audience and
  precise location, stop the local watch and re-read the snapshot. Revoke requires
  explicit confirmation. A server-side withdrawal omits the peer position and the
  cluster on the next read while preserving independent MESSAGE eligibility.
- **Stale / offline recovery.** The L01 adapter exposes `availability()` with
  server-clock `readAtMs`, age, policy `staleAtMs`, failure reason and refreshing.
  A snapshot older than `policy.fresh_ms` is labelled `Snapshot Ns ago` and every
  friend source becomes last-known; nothing is labelled current. `REFRESH`
  re-reads authority. Offline keeps only a stale last-known snapshot; 503 clears
  the roster. Recovery re-reads without duplicating actors (userId dedupe plus the
  server roster remains keyed).
- **503 fail-closed.** `LocationUnavailableError`, HTTP 503, other 5xx, auth loss
  and network loss are classified explicitly. A denied `VENUE_FRIEND_V1` gate
  produces `state: unavailable, reason: friend-gate-unavailable`; the FRIENDS tab
  renders "Friends unavailable" with the 503 reason and a retry, never an empty
  normal friend list. The shell now mounts this truthful unavailable panel
  (`production.start` returns true for a denied/offline producer; only a missing
  clock/access keeps the shell closed).

## W04 assumption

The assignment mentions a W04 decision (invitee consent, one inviter grant,
capacity 8, 24h expiry, host revoke) "where invitations surface here". No party or
venue invitation surface exists in the Map/Friends client: it surfaces ASK
LOCATION metadata, not W04 invitations. The frozen
`docs/mcr/contracts/paradise-city-v1/policy-freeze.md` records W04 as
`OPEN / NOT_IMPLEMENTED`, its five-minute proposal as unfrozen, and forbids L/M/N
from using W04 membership or invitations as authority. This change therefore adds
no W04 route, capability or policy; invitation UI remains task 26. Treating the
assignment's W04 wording as applicable here would violate the frozen venue policy.
The sharing audience remains the injected `LocationSharingAudience` seam; no
second consent protocol was invented.

## Changed paths

- `frontend/src/features/paradise-city-map/ports.ts`
- `frontend/src/features/paradise-city-map/model/controller.ts`
- `frontend/src/features/paradise-city-map/model/location-adapter.ts`
- `frontend/src/features/paradise-city-map/ui/ParadiseCityMap.tsx`
- `frontend/src/features/paradise-city-map/ui/paradise-city-map.css`
- `frontend/src/features/paradise-city-map/__tests__/consumer/*`
- `frontend/src/features/paradise-city-map/__tests__/browser/*`
- `frontend/src/app/venue-root/production.ts`
- `frontend/src/app/venue-entry/capability-adapters.ts`
- `frontend/src/app/WorldShell.tsx`
- `frontend/src/app/__tests__/venue-shell-ports/production.test.ts`
- `frontend/src/app/__tests__/venue-host-integration/mounting.test.tsx`

## Commands and results

| Command | Exit | Result |
| --- | --- | --- |
| `corepack pnpm@11.7.0 install --frozen-lockfile --offline` (Node 24.22.2, shared read-only store) | 0 | 199 packages reused, 0 downloaded; no manifest/lock change |
| `corepack pnpm@11.7.0 run typecheck` | 0 | `tsc -b --pretty false` |
| `MCR_VENU_TMP=<private> corepack pnpm@11.7.0 exec vitest run --config src/features/paradise-city-map/__tests__/consumer/vitest.config.ts` | 0 | 5 files, 79 tests passed |
| `corepack pnpm@11.7.0 exec vitest run src/app/__tests__/venue-shell-ports src/app/__tests__/venue-host-integration --maxWorkers=2` | 0 | 2 files, 13 tests passed (includes the 503-mount case) |
| `corepack pnpm@11.7.0 exec oxlint <changed paths>` | 0 | no errors; one pre-existing `no-control-regex` warning in untouched `art-source/qualify.ts` |
| `node src/features/paradise-city-map/__tests__/browser/native.mjs` | 0 | PASS, 3 widths × 7 states, 0 page errors, 0 horizontal overflow, 0 sub-44px targets |

## Browser captures

Driver: `frontend/src/features/paradise-city-map/__tests__/browser/native.mjs`
(actual `VenueShellFeatures`, production L01 adapter, controller and panel; a
deterministic session/community clock/Location/landmark/friend fixture). Evidence
report: `task-25-map-friends/report.json` — `result: PASS`,
`qualification: SUBSTITUTED_FIXTURE`.

Captured at 360×800, 390×844, 430×932:

- `clustered-friends-{360,390,430}.png` — three checked-in friends in one cluster.
- `permission-denied-{360,390,430}.png` — denied device permission, no self marker,
  server lease paused, reviewed retry.
- `paused-{360,390,430}.png` — paused status, self marker removed.
- `revoked-{360,390,430}.png` — explicit revoke, audience removed.
- `stale-{360,390,430}.png` — "Snapshot 20s ago", last-known labels, REFRESH.
- `stale-recovered-{360,390,430}.png` — fresh snapshot, check-in labels restored.
- `unavailable-503-{360,390,430}.png` — Friends unavailable with the 503 reason,
  never an empty normal list.

Inspection: reviewed every scenario; no overlapping labels, no decorative left
accents, visible labels legible, all visible buttons ≥44px, no horizontal overflow.
The initial capture exposed the default "Character unavailable" fallback clipping
inside cluster portraits; the fallback was constrained and the fixture now renders
deterministic initials. Re-captured after the fix.

## Limitations / risks

- Browser evidence is a controlled fixture: no live backend, real two-client
  revocation transport, device GPS, or physical venue. The server remains the
  authority; the client reflects denials.
- Cross-client revocation currently arrives through the next authoritative read
  (stale refinement/refetch). A realtime invalidation transport remains Root's
  integration dependency per `location.md`; no client-side consent was invented.
- Production `sharingAudience` stays the injected seam and defaults unavailable;
  the shell does not invent a viewer picker.
- W04 invitation policy remains pending/unfrozen and is not implemented here.

## Cleanup

Private vitest/browser temp directories were removed. No browser, Vite, or Node
process remains. The temporary debug driver was deleted; only the committed
fixture/driver and capture evidence remain.
