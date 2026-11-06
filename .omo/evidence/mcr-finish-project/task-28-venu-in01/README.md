# Task 28 — VENU-IN01 integrated venue flows and reference comparisons

Date: 2026-11-06
Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w4-in01`
Branch: `codex/mcr-exec-w4-in01`
Base: `13d2ee25f` (`master`, venue backend plus the four venue clients: shell ports, map/friends, invitations/conversation, place notes)
Captured commit: `9143e2489` (`test(venue): execute integrated flows and reference comparisons`). The captures were generated from that commit's working tree, not from the base commit.
Commit: local only; no push or merge. Plan row: `.omo/plans/mcr-finish-project.md` item 28.

## Disposition

Plan row 28 closes as `[~]`, not `[x]`. The evidence is complete for what the current
producer can answer, but it is not a full pass: 23 of 48 stable reference cases and
7 of 10 extension IA states are `NOT_REACHABLE` while
`PgVenueLocationAuthority::friend_in` is a fail-closed stub returning
`VenueLocationError::PolicyUnavailable` (HTTP 503). A bilateral friend authority is
being implemented. Once it lands, the full matrix must be re-run against it; that
rerun covers the friend location, sharing audience, meeting, invitation and
conversation case families. Until then row 28 is partial, and no friend-dependent
row is accepted.

## Outcome

The integrated venue surface was driven through the real production composition
(`VenueShellFeatures` → `createVenueSources` → frozen Location/Notes/Meeting
clients → venue adapters → Map/Notes/Meeting panels) with a deterministic
session, community clock and transport. Every stable reference case and every
extension IA state was captured at 360, 390 and 430 CSS px.

| Scope | Total | PASS | DIFFER | NOT_REACHABLE |
| --- | ---: | ---: | ---: | ---: |
| Stable reference cases | 48 | 23 | 2 | 23 |
| Extension IA states | 10 | 3 | 0 | 7 |

- **PASS** means the app surface matches the referenced state within the
  measured component deltas. The only remaining deltas are the intentional
  44px hit-target override (reference study controls are 35–40px tall).
- **DIFFER** is `P01-whole-plan` and `P04-whole-plan`: those are alternative
  whole-site illustrations, not the selected base. The app renders the
  provisional Photo 5 siteplan. The difference is intentional and recorded,
  not hidden.
- **NOT_REACHABLE** is a producer outage, not a visual failure. The bilateral
  `VENUE_FRIEND_V1` authority returns 503
  (`PgVenueLocationAuthority::friend_in` → `VenueLocationError::PolicyUnavailable`).
  Every friend-location, sharing-audience, meeting, invitation and conversation
  case is recorded against the truthful unavailable panel. A deterministic
  fixture capture of the same state is retained under `fixtures/` for visual
  inspection and is explicitly excluded from the verdict.

## Extension IA states

| State | Verdict | Live status |
| --- | --- | --- |
| venue overview | PASS | reachable |
| Friends list / clusters | NOT_REACHABLE | 503 friend authority |
| location consent / status controls | NOT_REACHABLE | no production `LocationSharingAudience`; friend authority 503 |
| selected friend / request | NOT_REACHABLE | 503 friend authority |
| fixed-place invitation composer | NOT_REACHABLE | 503 friend authority |
| invitation decisions / status | NOT_REACHABLE | 503 friend authority |
| meeting conversation | NOT_REACHABLE | 503 friend authority |
| notes map / list | PASS | notes reads authorized |
| note composer / audience / expiry | PASS | notes create authorized |
| note / replies / Meet Here | NOT_REACHABLE | replies render; Meet Here enters the blocked invitation flow |

## Material fixes applied after inspecting the captures

1. **Map landmark taxonomy.** Reviewed meeting places now show their flag and
   label by default (`pc-landmark--meeting`), matching the reference's visible
   Main Gate / Sub Gate pins. Non-meeting art labels remain hover/focus only.
2. **Note marker taxonomy.** Note markers now show the place label and the
   envelope count (`MAIN GATE ✉ 2`) with a 44px target, instead of an unlabeled
   clipboard glyph.
3. **Friend row density and type.** Names 22→18px, place 16→14px, source
   12→11px, row padding 4→3px, plus semantic phone/check-in/last-known icons.
   Reference row height is ~79 CSS px at 390; the app row is 74px.
4. **Selected friend density.** Portrait 80→64px and heading 26→24px; the
   reference component portrait is ~57px. Selecting a friend on MAP no longer
   collapses the siteplan to the notes compact height, matching the reference
   tall-map-plus-bottom-sheet composition.
5. **Note card density and typography.** Portrait 64→48px, name 19→16px, body
   16→14px, meta 14→11px. Reference measurements at 390: portrait 47px, name
   14.6px, body 12.7px, card 121px; app: 48px, 16px, 14px, 117px.
6. **Notes tab chrome.** The sharing footer is not shown on NOTES, matching the
   supplied notes screens; sharing remains available on MAP and FRIENDS, and
   opening NOTES closes any stale consent dialog.
7. **Sharing control glyph.** The pawn glyph is replaced with a semantic people
   icon; the existing 44px Pause/Revoke controls are unchanged.
8. **Invitation deep-link recovery.** A direct canonical
   `/meet/invitations/:id` route now reads the two authoritative inbox
   directions once and retries the exact ID when no prior target exists. This
   makes pending invitation decisions reachable without inventing a recipient.
   The fixture proves `DECLINE`/`ACCEPT` and invitation status render.

All visible controls in all 57 captures satisfy the 44px minimum target and no
capture has horizontal overflow (asserted by the driver).

## Measured deltas (390 CSS px)

| Element | App | Reference | Delta | Disposition |
| --- | ---: | ---: | ---: | --- |
| Tab height | 44px | 36.6px | +7.4px | accepted 44px target |
| Friend row | 74px | 79.2px | -5.2px | within tolerance |
| Friend portrait | 56px | 53.1px | +2.9px | within tolerance |
| Note card | 116.6px | 120.8px | -4.2px | within tolerance |
| Note portrait | 48px | 46.8px | +1.2px | within tolerance |
| Note name | 16px | 14.6px | +1.4px | within tolerance |
| Note body | 14px | 12.7px | +1.3px | within tolerance |
| Primary button | 44px | 35.1px | +8.9px | accepted 44px target |

Reference values are developer-measured from the supplied JPEG composites. The
exact ROIs, per-width transforms, app metrics and per-case deltas are in
`report.json`; each case records its original photo SHA-256.

## Provisional variant caveat

Photo 5 (base map), Photo 9 (Friends) and Photo 10 (Notes) are **PROVISIONAL**
primary roles. This task does not approve that variant, does not add a product
variant selector, and does not claim pixel identity, live service, device or
physical-venue acceptance. Photo 1/4/7/8 are retained as distinct alternatives.

## Evidence layout

```
task-28-venu-in01/
  report.json                 machine-readable case/IA report
  cases.json                  stable case and IA register with measured ROIs
  states.json                 capture-run report and all DOM metrics
  sha256.txt                  every evidence file, SHA-256
  captures/<caseId>/<w>.png   48 cases × 360/390/430
  fixtures/<caseId>/<w>.png   deterministic same-state capture for blocked cases
  reference/<caseId>.png      developer-measured reference crop
  overlay/<caseId>-<w>.png    component-aligned 50% overlay
  diff/<caseId>-<w>.png       absolute difference
  ia/<iaId>/<w>.png           10 extension IA states × 3 widths
  ia-live/<iaId>/<w>.png      truthful unavailable capture for blocked IA states
  metrics/<state>-<w>.json    viewport, DPR, boxes, type, hit targets, overflow
```

The comparison boundary is the venue panel surface: tabs, siteplan, sheets,
sharing footer and meeting leaf. Shell header and bottom navigation are FE
shell scope and are excluded.

## Commands and results

Toolchain: Node `v24.22.2` (`/tmp/mcr-node24/bin`), `corepack pnpm@11.7.0`.

| Command | Exit | Result |
| --- | ---: | --- |
| `corepack pnpm@11.7.0 install --frozen-lockfile --offline` | 0 | 199 packages reused from the shared store; lockfile unchanged |
| `corepack pnpm@11.7.0 typecheck` | 0 | `tsc -b --pretty false` |
| `corepack pnpm@11.7.0 exec vitest run src/features/paradise-city-map/__tests__/consumer src/app/__tests__/venue-host-integration src/app/__tests__/venue-shell-ports/production.test.ts src/app/venue-notes-adapter/__tests__/adapter.test.ts` | 0 | 8 files, 154 tests passed |
| `corepack pnpm@11.7.0 exec oxlint <changed source and fixture paths>` | 0 | no warnings |
| `node src/app/__tests__/venue-in01/browser/native.mjs` | 0 | 57 state captures (19 states × 3 widths), zero page errors, 44px and overflow assertions |
| `python3 frontend/src/app/__tests__/venue-in01/compare.py` | 0 | 48 cases + 10 IA, 579 evidence files |

## Limits and what remains

- No live backend, GPS, device, physical venue, two-client flow or character
  runtime was exercised. Captures are `CONTROLLED_FIXTURE` over the real
  production clients and adapters.
- Portrait slots show the product's truthful `Character unavailable` fallback;
  the reference shows character art. Meeting presentation currently supplies
  `portrait: null`, so invitation/conversation portrait boxes are empty until a
  reviewed character presentation is plumbed into the meeting leaf.
- The friend, meeting and invitation families remain live-blocked by the 503
  bilateral friend authority. Their visual captures are supplementary, not
  acceptance. The full matrix is re-run once the bilateral friend authority lands
  (case families: friend location, sharing audience, meeting, invitation,
  conversation).
- Location sharing cannot start because the production composition has no
  `LocationSharingAudience` provider; the consent UI and truthful refusal are
  captured, but opt-in sharing remains blocked.
- Provisional Photo 5 / 9 / 10 approval, pixel identity, current GPS, physical
  arrival and IN01 acceptance remain open. QA08 (200 concurrent users) remains
  `DEFERRED_BY_USER` and is not claimed.
- The worktree was removed by an external cleanup during the first capture run;
  the branch was recreated from `13d2ee25f`, all source changes were re-applied
  from the session record, dependencies were reinstalled and the full capture
  matrix was regenerated from the `9143e2489` working tree before this report was
  written. The surviving partial run was discarded. An earlier revision of the
  report pinned those captures to `13d2ee25f`; that provenance was wrong for
  `MapCanvas.tsx` (`7ea57b56…` here vs `8d631bc2…` at that commit) and for the
  test-only `venue-in01/fixture.tsx` and `browser/native.mjs`, which do not exist
  at `13d2ee25f`. Only the recorded commit changed; capture bytes did not.
