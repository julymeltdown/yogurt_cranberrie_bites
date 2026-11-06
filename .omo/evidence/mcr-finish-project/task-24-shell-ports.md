# Task 24 — production shell venue ports

Date: 2026-11-06
Repository: `/Users/lhs/Desktop/mcr-worktrees/exec-w4-shell`
Branch: `codex/mcr-exec-w4-shell`
Base: `cc8ad1b5b` (`master`, 2026-11-06 merge `PI11 responsive rooms, nearby and help`)
Delivered commit: `cb44bb77c` `feat(venue): wire production shell ports`
Follow-up fix: `d890b7755` `fix(venue): fix the 360 friend fallback and align the landmark gate`
Final fix: `6f98473fe` (same subject) — 56px 360 portrait and text-rect containment assertion
Harness fix: `627f62b75` `fix(ui): make the portrait clip assertion detect the regression`
Publication: local branch only; no push/merge
Plan row: `.omo/plans/mcr-finish-project.md` item 24 — “Wire the production shell to the venue ports”
Status: source delivered and validated with a deterministic browser fixture. No live backend, physical venue or device qualification is claimed.

## Delivered

`WorldShell` no longer publishes a venue root input with `meeting: null` and no `capabilities`/`phone`. `VenueShellFeatures` now builds one shell-owned production composition per verified siteplan/publication release:

- `createVenueSources` (`frontend/src/app/venue-root/production.ts`) creates the frozen-contract clients:
  - `createLocationClient(createHttpLocationTransport(...))` for `/venue/location`, `/venue/landmarks`, location commands and command status;
  - `createNotesClient(createHttpNotesTransport(...))` for the six frozen place-note operations;
  - `createMeetingClient(createHttpMeetingTransport(...))` for the frozen invitation/conversation/command operations.
- Reviewed landmark grants from `listVenueLandmarks` become `LocationAssociations`, `ReviewedNoteAssociations`, note creation rights and `MeetingLandmarkContext`; `presentation_id` must name an accepted illustrated landmark, and `can_check_in`/`can_meet`/`can_note` remain the only capability booleans.
- The default identity directory decodes the validated C03 participant for self and resolves a friend name only from the existing event-scoped `getConcertPublicProfile` route after the authoritative Location snapshot names that friend. A failed profile is omitted, never replaced with an invented name.
- `createVenueProduction` composes `createVenueLocationAdapter`, `createVenueNotesAdapter` and `aggregateVenueCapabilities`, supplies `createBrowserLocation` for the phone port, and builds a real `MeetingLeafRelease` with `createNormalizedMeetingAdapter`, the qualified community clock and the same shell signal.
- `VenueShellFeatures` loads reviewed landmarks, projects the venue with real meeting contexts, starts the authoritative location read, and publishes the input only when `capabilities.read()` returns a current snapshot. Without a qualified clock, current session/privacy, or a current server Location/Notes snapshot, the panel is not published and renders the existing truthful unavailable state. The `loadLandmarks` boolean is not a publication precondition: a false result empties the reviewed landmark projections while a current Location/Notes snapshot still publishes the panel.
- **Landmark scope, not a panel gate:** reviewed landmark authority is independent of the Location and Notes leases, matching `aggregateVenueCapabilities`. A Landmark read rejection empties `LocationAssociations`, note associations, note creation rights and `MeetingLandmarkContext`; it does not suppress an otherwise current Location/Notes capability. The panel can therefore remain published while check-in, post-note, meeting and Meet Here actions are unavailable. The regression test `keeps the independent location capability when reviewed landmark authority is denied` proves `loadLandmarks` false, `meetingPlaces` empty and `capabilities.read()` non-null with empty `checkIn`/`postNote` rights.
- `venue-root/input.ts` now requires `capabilities` and `phone`; omission is a type error. `projectVenueRootView` is exported so the same verified lease is projected before and after meeting contexts are known.

No route ownership, shared contract, generated registry, backend import, second protocol, plan file, or login/session owner was changed.

## Files

- `frontend/src/app/WorldShell.tsx`
- `frontend/src/app/venue-root/input.ts`
- `frontend/src/app/venue-root/index.ts`
- `frontend/src/app/venue-root/production.ts` (new)
- `frontend/src/app/__tests__/venue-host-integration/mounting.test.tsx` (fixture now mounts the real production ports)
- `frontend/src/app/__tests__/venue-shell-ports/production.test.ts` (new)
- `frontend/src/app/venue-entry/ParadiseCityMapPanel.tsx` (fallback label class)
- `frontend/src/features/paradise-city-map/ui/paradise-city-map.css` (56px friend portrait at every width, including ≤370px; wrapped 9px fallback)
- `frontend/src/app/__tests__/venue-shell-ports/browser/fixture.html` (new)
- `frontend/src/app/__tests__/venue-shell-ports/browser/fixture.tsx` (new)
- `frontend/src/app/__tests__/venue-shell-ports/browser/native.mjs` (new)
- `frontend/src/app/__tests__/venue-shell-ports/browser/evidence/*.png` and `report.json` (new)

## Commands and results

Working directory: `frontend/` unless noted. Toolchain: Node `v24.22.2` (official tarball extracted to `/tmp/mcr-node24`), `corepack pnpm@11.7.0`.

| Command | Exit | Result |
| --- | --- | --- |
| `corepack pnpm@11.7.0 install --frozen-lockfile --offline` | 0 | 199 packages reused from the shared store; lockfile unchanged. The worktree had no `node_modules`, so the parent/shared pnpm store at `~/Library/pnpm/store/v11` was used read-only. |
| `corepack pnpm@11.7.0 typecheck` (`tsc -b --pretty false`) | 0 | No TypeScript errors. |
| `corepack pnpm@11.7.0 exec vitest run src/app/__tests__/venue-shell-ports/production.test.ts src/app/__tests__/venue-host-integration/mounting.test.tsx src/features/paradise-city-map/__tests__/consumer src/app/venue-notes-adapter/__tests__/adapter.test.ts` | 0 | 8 files, 111 tests passed. Includes the new real-capability, landmark-denial and fail-closed port tests plus the existing venue shell/consumer/notes suites. |
| `corepack pnpm@11.7.0 lint` (`oxlint src vite.config.ts`) | 0 | No errors. Pre-existing warnings remain in unrelated generated/legacy files. |
| `corepack pnpm@11.7.0 exec oxlint src/app/venue-entry/ParadiseCityMapPanel.tsx src/app/__tests__/venue-shell-ports/production.test.ts src/app/__tests__/venue-shell-ports/browser/native.mjs` | 0 | Changed-file lint clean. |
| `node src/app/__tests__/venue-shell-ports/browser/native.mjs` | 0 | `{"result":"PASS","qualification":"SUBSTITUTED_FIXTURE",...}` at 360/390/430 for both states, zero page errors. The removed state is asserted by `role=status` visibility and zero MAP tabs, with no pinned sentence. The FRIENDS stage walks from the fallback label to its nearest clipping ancestor (`.pc-portrait`, `overflow:hidden`) and asserts the rendered text rectangle is inside that ancestor's content box; the same evaluate renders a deliberately 24px portrait and asserts the detector reports it as clipped. |

## Harness sensitivity (mutation evidence)

The assertion compares the rendered text rectangle with the content box of the **nearest clipping ancestor**, not the label's own scroll box or the portrait border box. A fail-fast control renders a deliberately 24px portrait inside the same evaluate and requires `clipped: true`, so a detector that stops seeing overflow fails the run instead of passing silently.

Runs in a throwaway worktree at `6f98473fe` (same source, symlinked `node_modules`; discarded afterwards):

| Mutated state | Result | Evidence |
| --- | --- | --- |
| 52px portrait, 9px fallback (current label rule) | PASS | text is `45.55px` wide inside the `50px` content box; the 52→56 change was cosmetic margin, not a clipping fix |
| 52px portrait, pre-repair 11px label (`.pc-portrait-fallback` rule removed) | **FAIL** | `360/real: 360: fallback text clipped {"clipped":true,"clip":"pc-portrait","content":{"left":19,"right":69,"top":321,"bottom":371},"text":{"left":19,"right":74.65625,"top":332.25,"bottom":358}}` |
| 56px portrait, 9px fallback (final) | PASS | full 3-width × 2-mode matrix passes with the self-check |

The detector therefore fails on the defect-bearing 52px state and on the deliberate 24px control, and passes only when the text is genuinely contained. Re-running the final harness produced byte-identical captures.

## Browser-mount captures

Fixtures use the actual `VenueShellFeatures` → production sources/adapters → siteplan loader → Paradise City Map panel. Session, community clock, Location snapshot, reviewed landmark list and friend profile are deterministic; this is not a live service, device or physical-venue claim.

- `real-capability-{360,390,430}.png` — verified siteplan plus a current server Location snapshot: MAP/FRIENDS/NOTES tabs, real landmark controls, server-granted check-in rights, and phone port idle.
- `real-friends-{360,390,430}.png` — the same mount on FRIENDS, showing the authorized friend identity and a truthful “Location unavailable” physical coordinate state (WGS84 is never projected onto the illustration). The 360 capture now uses the same 56×56 portrait box as 390/430; “Character unavailable” wraps fully across two lines with side margins instead of clipping to “Characte/unavailab”.
- `removed-port-{360,390,430}.png` — no participant/friend authority source: the shell does not publish the venue input and the panel shows “The venue map is unavailable until current access and siteplan inputs are supplied.” It never shows the normal map/tab state.
- `report.json` — PASS and per-width assertions for both states.

## Capture hashes (SHA-256)

Baseline = `cb44bb77c`; first repair = `d890b7755`; final = `6f98473fe`; harness fix = `627f62b75`. The final commit changes only `real-friends-360.png`; the 390/430 and capability/removed captures are byte-identical to `d890b7755`, and re-capture after the harness fix reproduced the same bytes.

| Capture | Before fix (`cb44bb77c`) | First repair (`d890b7755`) | Final (`6f98473fe`) |
| --- | --- | --- | --- |
| `real-capability-360.png` | `b1a54c95f62e4750123bf71d89e291c8247717e68a48b7b23dcdf9b66dc238c0` | `b1a54c95f62e4750123bf71d89e291c8247717e68a48b7b23dcdf9b66dc238c0` | `b1a54c95f62e4750123bf71d89e291c8247717e68a48b7b23dcdf9b66dc238c0` (unchanged) |
| `real-capability-390.png` | `17b7c7049db5eb2e6c76e3771b64b8803acb28f6319f6d9d3fd61f0b4d4ab759` | `17b7c7049db5eb2e6c76e3771b64b8803acb28f6319f6d9d3fd61f0b4d4ab759` | `17b7c7049db5eb2e6c76e3771b64b8803acb28f6319f6d9d3fd61f0b4d4ab759` (unchanged) |
| `real-capability-430.png` | `9029ad7a0ac04d6ecd5fbd535a1736f45726b0a2909fc82a3af8419008a32bd6` | `9029ad7a0ac04d6ecd5fbd535a1736f45726b0a2909fc82a3af8419008a32bd6` | `9029ad7a0ac04d6ecd5fbd535a1736f45726b0a2909fc82a3af8419008a32bd6` (unchanged) |
| `real-friends-360.png` | `983f905e76aebde98ee5b8196352a5ecb5721cb87a6e7530334baafc94e0dfef` | `5afdf4b23169a6b806987202dc6ce5c63009819652efaf9c869d8b777071f323` | `7d33c8187043e48ae0171a5a3cf81470fd415c4ea0223af33f660e6c99a88c04` |
| `real-friends-390.png` | `af62b5cbd513e812e04299abbe7ce5ac500f56040c9bf3d0af6dfd584ee97976` | `f3a3b8bd07ce7294c939d9c810c244fa49f098c37c101feaad680b989191cddc` | `f3a3b8bd07ce7294c939d9c810c244fa49f098c37c101feaad680b989191cddc` (unchanged) |
| `real-friends-430.png` | `b3b99e0240fd3ea53f8e2b7be6e4b2f63a6cefb3d42e29758eeb803440656a3d` | `8bff02cc03702899eddf7fd70781b9a210c853d4f1ced496b449c588a92a1b1c` | `8bff02cc03702899eddf7fd70781b9a210c853d4f1ced496b449c588a92a1b1c` (unchanged) |
| `removed-port-360.png` | `6c27cec321de7a00bb3a9873f87078173691e3a645a072846764661830e16c7b` | `6c27cec321de7a00bb3a9873f87078173691e3a645a072846764661830e16c7b` | `6c27cec321de7a00bb3a9873f87078173691e3a645a072846764661830e16c7b` (unchanged) |
| `removed-port-390.png` | `74f8a029f815753c835a7d399b097a7f254a2a54a1471253e8caa312669908c5` | `74f8a029f815753c835a7d399b097a7f254a2a54a1471253e8caa312669908c5` | `74f8a029f815753c835a7d399b097a7f254a2a54a1471253e8caa312669908c5` (unchanged) |
| `removed-port-430.png` | `b4fa5262d3c829a29da6a69df882fc8586ce61c25605c59d84cfe82fb88c0ec9` | `b4fa5262d3c829a29da6a69df882fc8586ce61c25605c59d84cfe82fb88c0ec9` | `b4fa5262d3c829a29da6a69df882fc8586ce61c25605c59d84cfe82fb88c0ec9` (unchanged) |

The captures are preserved as `.omo/evidence/mcr-finish-project/task-24-shell-ports/before/real-friends-{360,390,430}.png` (from `cb44bb77c`), `after/real-friends-{360,390,430}.png` (from `d890b7755`) and `final/real-friends-360.png` (from `6f98473fe`). The before 360 image shows the clipped `Characte/unavailab` fallback; the final image shows both words fully wrapped with side margins.

## Contract note

The shell publishes on a current `capabilities.read()` snapshot, not on landmark success. The `loadLandmarks` result is consumed only for landmark-dependent projections; returning false is not a publication precondition. Landmark denial removes only the landmark-dependent rights and meeting contexts; the `keeps the independent location capability when reviewed landmark authority is denied` test pins that behavior. The removed-port capture is a separate state with no participant/friend authority, so no capability snapshot exists and the panel is not published.

## Limitations and risks

- The frozen L policy requires `VENUE_FRIEND_V1` (bilateral Friend follow edges + blocks). The policy freeze says the venue friend gate returns 503 until the Friend owner supplies the bilateral read; a live server may therefore correctly return unavailable, which the shell now surfaces instead of a normal state.
- Landmark denial is not a panel-level outage. It removes only landmark-dependent actions and meeting contexts; current Location/Notes authority remains visible. This is the intended independent-lease contract, not a claim that landmarks gate the panel.
- The default friend directory resolves names via the existing event-scoped public-profile read. A profile rejection omits that friend; no name, avatar or placeholder is fabricated. Profile completions do not force a second Location snapshot.
- `sharingAudience` has no production provider yet; without an explicit viewer selection, `beginPhoneSharing` rejects. This is fail-closed, not a sharing grant; the friends/selection UX remains task 25.
- Note creation rights are `can_note`-filtered; the Notes adapter still requires an explicit notes read before `rights.postNote` is populated, so “+ NEW NOTE” is disabled until a place’s notes are loaded.
- The creates are “AUTHORITATIVE_ADAPTER” classifications; browser evidence is a deterministic fixture, not a live multiuser run. VENU-IN01 and device/live gates remain separate.

## Cleanup

- Vite dev server and headless Chrome were closed by `native.mjs`; its cache is `/tmp/mcr-venue-shell-ports-cache`.
- The downloaded Node 24 tarball and extracted runtime are under `/tmp/mcr-node24-dl` and `/tmp/mcr-node24`; they are outside the repository and can be removed.
- Worktree `node_modules` and `tsconfig` build info are ignored; no generated artifact was modified.
