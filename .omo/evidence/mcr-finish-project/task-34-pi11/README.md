# Task 34 - PI11 Rooms, Nearby and How to Play

Status: PASS on the responsive layout scope. Synthetic controller/roster state in the
retained shell frame; this is not live-service, physical-device, production-art or
whole-reference acceptance. W04 party invitation remains separately open.

## Source

- Worktree `/Users/lhs/Desktop/mcr-worktrees/exec-w6-pi11`, branch `codex/mcr-exec-w6-pi11`.
- Product changes: short-height compaction in `world-access.css`; Nearby portrait
  fallback text fits its 68px control without mid-word breaking.
- Changed paths: `frontend/src/features/world-nearby/WorldNearby.tsx`,
  `frontend/src/features/world-rooms/ui/world-access.css`,
  `frontend/src/features/world-help/__tests__/world-help.test.tsx`.
- No `frontend/src/world/reference-scene`, map geometry, camera, renderer, contract,
  lockfile, root router or backend path changed.

## Layout qualification

`layout-qa.mjs` copies its page files into the frontend test directory, runs Vite and
Chrome, and removes them afterward. It renders the actual `WorldRooms`, `WorldNearby`,
`WorldHelp` and `PanelHeader` components inside the retained `world-shell` /
`shell-panel` frame at:

- 390x844 full panel
- 430x844 full panel
- 390x420 full panel (short height)
- 360x844 full panel (legacy layout regression)

For each Rooms, Nearby and Help case it asserts no horizontal overflow, every visible
button/link is at least 44x44 with unclipped text, and no interactive control
rectangles overlap. On short height every control is focused in turn and must be fully
inside the viewport and hit-testable. At 390x844 the Rooms case performs the mandatory
room switch and verifies the confirmation, target membership and focused status region.
Nearby opens the real avatar popover at 390 and 430; Help verifies the final link is
fully visible after focus.

Result: `layout/report.json` status PASS, 12 cases, 15 captures.

Command:

    cd frontend && node /Users/lhs/Desktop/mcr/.omo/evidence/mcr-finish-project/task-34-pi11/layout-qa.mjs

## Product checks

- `tests.log`: targeted Vitest run, 6 files / 84 tests passed, exit 0.
- `typecheck.log`: `corepack pnpm@11.7.0 run typecheck`, exit 0.
- `lint.log`: scoped oxlint on changed files, exit 0.

## Limitations

- Synthetic room/roster state and the retained shell frame; no live backend socket,
  real device, software keyboard, production avatar rendering or physical-location claim.
- `fixture-drift/native-report.json` records the pre-existing PI11 app-fixture run that
  stops before layout because the pinned canonical overlay no longer reaches a live
  world socket at this HEAD. It is diagnosis only, not a product failure claim.
