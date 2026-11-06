# Task 27 — Complete the place notes client

Status: **IMPLEMENTED / DEVELOPER-VALIDATED**. Branch `codex/mcr-exec-w4-notes`.
Delivered base: `6cb31ca70` (`origin/master`, "merge(venue): complete map and friends
client states"). The surviving WIP was authored on `d890b7755`, which is already an
ancestor of `6cb31ca70`; the branch was fast-forwarded to master and the WIP was
re-applied and reconciled file by file. No backend, protocol, lockfile or generated
contract change. No push or merge.

## Delivered behavior

- **Map clusters from server rows.** `VenueCapabilities.loadNoteSummary` asks the
  existing frozen `listVenuePlaceNotes` operation (`limit=1`) once per mapped
  landmark, three at a time, and accepts a row only when its physical
  `landmark_id`/`landmark_revision` matches the reviewed association, `server_now`
  does not exceed the borrowed clock, `visible_count` is a safe integer not below
  the returned item count, and the page lease is current. Map markers and the
  "Places with notes" rows render only `noteClusters`; a loaded page length never
  becomes a place total. A denied place contributes no marker and no count.
- **List and selection.** Choosing a marker or a cluster row selects
  `{ kind: 'notes', id }` and performs the authorized page read. The heading shows
  the server `notesVisibleCount` ("MAIN GATE / 2 NOTES") and, when the page is
  shorter, "Showing the latest N of M." The manual "Read notes at" control remains
  for places without a marker and defaults to the first cluster.
- **Composer.** Audience `FRIENDS`/`EVERYONE` and expiry `1 HOUR`/`END OF DAY` are
  limited to the current `rights.noteAudiences`/`noteExpiries`; the adapter refuses
  anything outside the contract before any write (`rejected`, no request) and maps
  `END_OF_DAY` to the wire enum `VENUE_END_DAY`. Place selection is limited to
  `rights.postNote`.
- **Replies.** The reply composer is bound to the parent id and revision
  (`expected_parent_revision`); only the parent id and body are sent. Threads are
  read through `getVenuePlaceNote` when `reply_count` exceeds the loaded replies, and
  reply authors are rendered from the producer reply row, not the root author.
- **Meet Here.** `MEET HERE` on a note calls the existing single entry point
  `MeetingConsumerPort.open({ action: 'INVITE', origin: 'map-note', ...reviewed
  landmark })` on the row-26 `VenueCommunityFeatures` leaf (mounted once by
  `venue-root` and proxied by `venue-root/binding`). The capture shows the real
  "Meet at MAIN GATE?" invitation flow. No fork, no second capability family, no new
  protocol. The action is enabled only when the server marks the note `meet_here`
  for this viewer and the landmark has reviewed meeting context.
- **Truthful states.**
  - *Expired* — rows are filtered by `created_at <= server_now < expires_at` and the
    page lease is withdrawn at the earliest row expiry. The controller now re-reads
    the place when its disclosure lease ends instead of presenting an ended lease as
    "no notes" (regression: `re-reads the place when its disclosure lease ends`), so
    only the expired note disappears and the count drops (capture `expired-*`).
  - *Hidden / withdrawn* — nothing is synthesized: withdrawn rows disappear on the
    next producer read, the count follows, and only `can_withdraw` notes show
    `WITHDRAW` (capture `withdrawn-*`).
  - *Non-author withdraw* — the control is absent unless the server sets
    `can_withdraw`; the adapter also refuses an unmarked note and a mismatched
    revision (tests `refuses a client-side withdrawal…`), and a decoded 4xx is a
    definitive `rejected` rather than an uncertain outcome.
  - *Unreviewed landmark* — the landmark card renders "Illustrated reference ·
    real-place approval unavailable" and disables `+ LEAVE A NOTE`, `CHECK IN` and
    `MEET HERE` (capture `unreviewed-*`).
  - *Stranger / blocked denial* — a denied read renders "Notes are unavailable for
    this place." with no markers, counts, rows or enabled composer, and a denied
    summary renders "Note counts are unavailable for this place." (capture
    `denied-*`).
- **Timestamps** are rendered only from producer values (`created_at`,
  `expires_at`) relative to the qualified server clock; no `Date.now()` is used in
  the notes projection.

## Merge reconciliation (file by file)

`git fetch origin`, then `git merge --ff-only origin/master` after stashing the WIP.
Master already contained `09be648b8`/`e61dd7f98` map work, so the surviving delta is
notes-only; master's version was kept wherever it already implemented the same
behavior.

| File | Resolution |
| --- | --- |
| `model/controller.ts` | Kept master's availability/snapshot/refresh flow and collapsed `run()` to one result variable; added `loadSummary`, `withdraw`, applied-command reloads and the lease re-read. |
| `app/venue-entry/capability-adapters.ts` | Kept master's `availability`/`refresh`/`revoke`/`decideLocationRequest` and unavailable-object lines; added `withdraw`, `loadNoteSummary` and the cluster fields to the aggregate. |
| `ports.ts` | Kept `availability`/`refresh`/`revoke`/`decideLocationRequest`; added `NoteCluster`, `notesVisibleCount`, `loadNoteSummary` and `withdraw`. |
| `__tests__/consumer/harness.ts` | Kept master's availability/refresh/revoke/`deny`/`noDevice`; added notes reads and `notesRead`. |
| `model/location-adapter.ts` | Auto-merged; the only delta is `noteClusters: []`/`notesVisibleCount: null`. Check-in freshness projection untouched. |
| `ui/MapCanvas.tsx` | Note markers render server counts; the stored-portrait `ResizeObserver` measurement and friendship clustering are unchanged. |
| `ui/ParadiseCityMap.tsx`, `paradise-city-map.css` | Master's friends/availability/sharing UI retained; notes cluster row, server total heading, reply authors, reply load, withdraw and the compact note card actions added. No left accent bars. |
| `shared/api/paradise-city-v1/notes.ts` | Master rejected every page whose `access_until` was not ahead of `server_now`; the producer legitimately returns `access_until === server_now` for an *empty authorized* page. An empty page is now accepted and its place lease comes from the reviewed provider; a genuinely expired page still fails. |

## Checks (Node `v24.22.2`, pnpm `11.7.0`)

| Command (in `frontend/`) | Exit | Result |
| --- | --- | --- |
| `corepack pnpm@11.7.0 install --frozen-lockfile --offline` | 0 | `Already up to date` |
| `corepack pnpm@11.7.0 run typecheck` | 0 | `tsc -b` clean |
| `corepack pnpm@11.7.0 exec vitest run src/features/paradise-city-map src/app/venue-notes-adapter src/shared/api/paradise-city-v1/__tests__/notes.test.ts src/app/venue-entry src/app/venue-root src/app/__tests__/shell.test.tsx src/app/__tests__/venue-shell-ports src/app/__tests__/venue-host-integration` | 0 | 14 files, 273 tests passed |
| `corepack pnpm@11.7.0 exec vitest run` (whole suite) | 1 | 414 files / 5754 tests passed, 14 skipped, **3 pre-existing failures** (below) |
| `corepack pnpm@11.7.0 exec oxlint <16 changed files>` | 0 | No findings |
| `node src/app/__tests__/venue-notes-ui/native.mjs` | 0 | `result: PASS`, no console/page errors |

New regression tests: server-count clusters and one bounded read per place, empty
authorized page acceptance, place total vs page length, denied place omitted from the
summary, definitive non-author withdrawal denial, refusal without `can_withdraw`,
exact revision/idempotency withdrawal, reply author projection, non-contract
audience/expiry rejection, lease-expiry re-read and its single-failure bound, plus
view tests for server-count markers, cluster selection, right-limited composer,
withdraw revision, reply author identity, expiry at the clock boundary and the server
total heading.

Pre-existing failures observed and **not** caused by this task:

1. `src/shared/api/__tests__/c07-social-client/compatibility.test.ts` — "pins the
   exact public cohort" compares `frontend/src/shared/api/domain-operations.generated.ts`
   (`80905d6d…` pinned vs `2c45873e…` computed). It fails identically on the main
   checkout at master `937ab6699` (`/Users/lhs/Desktop/mcr/frontend`), which this
   branch does not touch.
2. `src/app/operations-composition/__tests__/composition.test.tsx` and
   `src/app/operations-composition/__tests__/reports-production/composition.test.tsx`
   — two operations-composition abort tests that are pre-existing timing flakes.
   They fail intermittently on unmodified master (4/6 failing runs on the base tree
   and 3/4 on the merged tree) and are not caused by this branch.

## Browser evidence

`node src/app/__tests__/venue-notes-ui/native.mjs` drives headless Chrome against
Vite with the **actual** `VenueShellFeatures`, siteplan loader, location and notes
adapters and `ParadiseCityMapPanel`; only the HTTP producer rows are deterministic.
Captures at 360×800, 390×844 and 430×932 (full page, no horizontal overflow
asserted), 30 PNGs in `.omo/evidence/mcr-finish-project/task-27-notes/`:
`clusters`, `selected`, `reply-thread`, `meet-here`, `composer` (audience + expiry),
`created`, `withdrawn`, `unreviewed`, `expired`, `denied`. `report.json` and
`sha256.txt` accompany them.

Qualification: **SUBSTITUTED_FIXTURE** (deterministic session, community clock,
location, landmark and note rows; no live backend, device or physical venue).

## Limitations and risks

- The frozen contract has no dedicated count endpoint; the cluster summary costs one
  `limit=1` list request per mapped landmark (3 concurrent), refreshed when the Notes
  tab is opened. Landmark counts are bounded by the siteplan.
- Creation stays disabled when the notes source has no live read lease for the
  selected place (fail-closed); a place whose read is denied cannot be posted to from
  this client even though a separate creation-rights provider might allow it.
- An empty authorized page keeps the reviewed provider's place lease; if that
  provider lease ends, the panel shows the read as unavailable rather than empty.
- Real multiuser/authorization behavior is enforced by the server and was not
  re-proven here; this task is the client.
