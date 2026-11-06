# Task 43 — FE24/FE33/FE34 real-producer closure

- Plan row: 43 (`Close FE24, FE33 and FE34`)
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w8-fe`
- Branch: `codex/mcr-exec-w8-fe`
- Base: `1c8d9d9e1` (`master`)
- Scope: frontend verification harness and evidence; no backend, contract, lockfile or router change.
- Commit: `feat(frontend): close FE24, FE33 and FE34`

## Status

| Task | Status | Basis |
| --- | --- | --- |
| FE33 mural synchronization | **CLOSED / PRODUCER-BACKED** | Real development stack; snapshot/delta 200 at revision 1; live Wall renders synchronized at 360/390/430 and fails closed on 401 |
| FE34 profile and preferences | **CLOSED / PRODUCER-BACKED** | Real development stack; PATCH 200 rev 1→2, stale PATCH 409 `VERSION_CONFLICT`; live Settings/Profile save and conflict at 360/390/430 |
| FE24 gift media/motif binding | **NOT CLOSED / PRODUCER BLOCKED** | BE24's own delivery record says end-to-end Gateway HTTP upload/read with real Auth/Friend is `NOT_RUN`; a probe against the real stack returned 403 `PERMISSION_DENIED` for media metadata/content and no approved production motif mapping exists. The client correctly renders the truthful unavailable/no-photo states. |

FE24 is not marked complete. The historical MOCK_READY closure and the reopened G03 delta are unchanged; the new evidence records exactly which producer remains missing. No fixture is presented as a production media or motif binding.

## What was fixture-only before

- FE33's prior evidence was the controlled C04/C05 fixture and synthetic HTTP. The current check runs the actual `createMuralHttp`/`createMuralSync`/`MuralSyncPanel` in the real default application against the real `api-gateway` and `concert-service`, with the real seeded Wall and real bearer session.
- FE34's prior evidence was the original-fe34 synthetic HTTP. The current check runs the actual `createMeController`, `SettingsPage` and `ProfileForm` against the real participant producer.
- FE24's protected detail tests use a controlled media reader. The current probe confirms that the real media read producer does not yet authorize an HTTP caller (`403 PERMISSION_DENIED`), matching the BE24 record `End-to-end Gateway HTTP upload/read with a real canonical Auth/Friend stack: NOT RUN`.

## Producer-backed behavior proven

### FE33

Real stack (`frontend/e2e/fe-closure/stack.py`) with fresh binaries built from this checkout:

```
GET  /api/concert/events/{event}/mural/snapshot      200, revision 1, EDITABLE
GET  /api/concert/events/{event}/mural/operations    200, from 1 to 1
```

The browser mounts the real sync controller and reports `Wall synchronized at revision 1.` at 360/390/430. With an invalid bearer token the adapter receives 401 and the UI reports `Wall synchronization unavailable. Reconnect the world and refresh before editing.` — no local preview or fabricated revision is shown. The shipped placement pins were checked against the authoritative server release map (`mcr_reference_scene_r3` revision 4, content hash `sha256:736afd27…`, geometry hash `sha256:899aca9b…`); they match, so no configuration change was needed.

### FE34

Real stack:

```
GET   /api/concert/events/{event}/me                200, revision 1
PATCH /api/concert/events/{event}/me                200, revision 2, preferences.low_power true
PATCH /api/concert/events/{event}/me (stale rev 1)  409, details.reason VERSION_CONFLICT
```

The browser saves the real preference, renders `Preferences saved. Sound only starts after you explicitly apply it in this session.`, and on the stale revision keeps the draft and renders `Your profile changed elsewhere. Your draft is kept. Discard it and reload before saving.` at 360/390/430. No automatic replay.

### FE24

The real stack returned `403 PERMISSION_DENIED` for `GET /media/{mediaId}` (metadata and content) and the recorded product behaviour is authoritative: the media owner fence is not HTTP-composed for a real caller. `GiftDetail` therefore has no production media or motif input to bind. Its truthful states remain `No photo` for a null `image_media_id` and `Image access unavailable.` when no usable port or authorized read exists; it never invents a URL or artwork. The art record `reference-gifts-content-v1.md` still records `productionBinding: null` and `NOT_READY`, and task 11 records HTTP qualification `NOT_RUN`. Closing FE24 would require BE24 HTTP composition plus the approved production media/motif mapping.

## Browser checks

Harness: `frontend/e2e/fe-closure/` (`stack.py`, `native.mjs`, `preview.tsx`). The Node harness proxies `/api` to the real gateway; the browser uses the real canonical `createSessionController` with a real bearer token. No HTTP response is synthesized.

12 captures at 360/390/430:

| State | Captures |
| --- | --- |
| Wall synchronized (real revision 1) | `mural-happy-{360,390,430}.png` |
| Wall synchronization unavailable (real 401) | `mural-unavailable-{360,390,430}.png` |
| Preferences saved (real 200) | `profile-happy-{360,390,430}.png` |
| Preferences conflict (real 409) | `profile-conflict-{360,390,430}.png` |

Machine report: `browser-report.json` — `result: PASS`, 12 cases, zero page errors, no horizontal overflow, no sub-44 control, no decorative left accent. The screenshots were inspected: the Wall editor, synchronized status, unavailable notice, saved preferences and conflict banner are legible with no clipping or accent rail. The only sub-44 element in the page is the harness-only `Harness save` control, excluded from the product-control assertion.

## Commands and exit codes

| Command | Exit | Result |
| --- | --- | --- |
| `corepack pnpm@11.7.0 install --frozen-lockfile --offline` | 0 | dependencies reused; no manifest/lock change |
| `corepack pnpm@11.7.0 run typecheck` | 0 | clean |
| `corepack pnpm@11.7.0 exec vitest run src/features/gift-discovery src/features/me src/features/mural/sync` | 0 | 35 files, 375 tests passed |
| `corepack pnpm@11.7.0 exec oxlint e2e/fe-closure/preview.tsx e2e/fe-closure/native.mjs` | 0 | no warnings |
| `python3 -m py_compile frontend/e2e/fe-closure/stack.py` | 0 | clean |
| `cargo +1.88.0 build -p concert-service -p auth-service -p friend-service -p profile-service -p post-service -p notification-service -p api-gateway` | 0 | fresh development binaries |
| `python3 -B frontend/e2e/fe-closure/stack.py --bin-dir …` | 0 | `result: PASS`, cleanup `PASS`, owned stack removed |
| `node e2e/fe-closure/native.mjs` | 0 | `{"result":"PASS","cases":12,"screenshots":12}` |

Pre-existing failures: none observed in the targeted suites. The full repository suite was not run in this task; the task asked for the targeted suites.

## Residual limits

- FE24 remains open. The media owner fence is not HTTP-composed for a real caller (403) and no approved production media/motif mapping exists. The client's unavailable state is truthful; it is not acceptance of a media producer.
- The development stack is not release, IN03/IN04, device or QA08 qualification. It uses one event and two admitted principals on a private cluster.
- The gift/motif path was not browser-captured as a happy state because the producer cannot currently authorize it. Capturing a fixture would contradict the task's no-fixture-only rule, so no such capture is claimed.
- FE33's world-wall Phaser texture publication was not re-qualified here; the IN04 backend gate at `.omo/evidence/mcr-finish-project/task-31-in04.md` remains the multiuser authority, and this task proves the real HTTP synchronization and panel state.
