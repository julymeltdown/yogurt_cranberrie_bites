# FE07 integrated shell review

Date: 2026-11-03. State: **MOCK_READY**, integrated; no real world or IA acceptance.
Parent candidate: `d7909c0`; worker implementation `d7c720c`, review fix
`18740f2`, handoffs `8ba0081` and `e4b50fa`. Integration equivalents are
`f51dd06`, `024e006`, `6d4872c` and `d7909c0`.

## Scope and review

All 18 changed paths match `frontend/src/app/**`, `frontend/src/main.tsx` and
`docs/mcr/frontend/world-shell.md`. No backend, shared implementation, manifest,
dependency, contract or character asset changed. Root composition in main,
app/router and app/routes was explicitly delegated in the assignment.
The exact delegation guard introduced at `45a6897` passes this candidate against
`69a5624`; default central ownership and cross-lane import/symlink checks remain.
Its regression suite ran 43 tests: 42 passed, one filesystem capability skip.

Parent review found a stale identity dispatch window between rendering and the
passive effect, plus an unbounded panel scroll map. Worker regressions first
reproduced both. The fix checks current identity before binding and dispatching
bootstrap/admin work; it bounds scroll history to 100 recent destinations.
Worker self-review additionally fixed query-only navigation leaving a minimized
panel hidden. Parent inspected the fixes and all assigned paths before integration.

## Independent integration verification

Commands ran in the integration checkout with Node 24.22.2 and pnpm 11.7.0.
All commands below exited 0.

| Check | Result |
|---|---|
| `corepack pnpm@11.7.0 --dir frontend test` | 276 tests across eight files; includes 65 shell tests |
| `bash scripts/mcr/check.sh frontend` | Frozen install, strict TypeScript/Vite build, lint and source-origin scan pass |
| `MCR_FE07_EVIDENCE=/tmp/mcr-fe07-integrated-d7909c0 node src/app/__tests__/browser-smoke.mjs` from frontend | Installed Chrome 156.0.8078.80; 360, 390, 430 and 1280px viewports plus built-production entry pass |

The [browser receipt](world-shell/browser-results.json), screenshots and
[file hashes](world-shell/candidate-hashes.json) are retained beside this record.
Parent visually inspected the 360px world and 1280px panel screenshots: quiet
surfaces, clear navigation and no decorative left accent. The fixture is visibly
labeled; it is not a live world. All four runs show instrumented peak owners of
one, six allocations and six disposals, and zero remaining owners on exit.
Production ignores `?fixture=ready`, contains no fixture identity/host and opens
no native WebSocket. The driver closes its owned browser and servers in finally.

## Remaining acceptance

This proves the shell seam, history/focus/panel behavior, guarded scope lifetimes
and production fixture isolation. Actual Phaser and socket ownership, camera and
position preservation, real authentication/bootstrap, avatars, mobile devices,
screen readers, multiuser flows and release performance are **NOT_RUN**. These
remain required through the corresponding feature owners and integration gates.
FE07 is not counted as a fully accepted task until real world behavior is tested.

The assigned shell increment and its concrete review fixes are delivered. Its
worker goal may complete; a new scoped goal must be created for the next task.
