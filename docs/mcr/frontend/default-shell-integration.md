# FE07 default shell integration

## Checkpoint 1: actual root chrome

Base: `e869480395b70b4aae6cfcf50e904b079a59a0e7`. Newly authored root integration consumes the published FE07 chrome from author `d6d7d935aafbc5b03db72e1e66bea357e1b0a76f`; no external rendering code copied. Dependencies and retained lockfiles are unchanged.

`WorldShell.tsx` now renders ShellHeader, EraControls, BottomNavigation and PanelHeader inside its existing NavigationMemory and canonical shell providers. Existing visualViewport sizing, safe areas, panel Back/focus/scroll, peek/half/full, input blocking and single WorldHost remain owned by the shell. The absent qualified theme producer is explicitly unavailable; no era is represented as selected.

Validation (all exit 0):

- Node v24.22.2 Vitest `run --root frontend --configLoader runner --no-cache src/app/__tests__/default-shell/chrome.test.tsx`: 3 actual createAppRoutes default tests passed, with no panel override.
- TypeScript `--noEmit --incremental false --project frontend/tsconfig.json`: passed.
- `node frontend/src/app/__tests__/default-shell/chrome-native.mjs`: three Chrome widths (360/390/430), all six navigation destinations, Settings, Inbox Back focus, full panel coverage, short viewport, disposal and overflow checks passed. Six captures and JSON/log under the test evidence directory.

Native checkpoint uses the instrumented shell fixture, not actual Phaser/canonical Auth/C04/C05. Map/place/Wall/theme qualified producer integration, actual-engine native validation, lint and production build remain IN_PROGRESS. Live two-user, approved product art, device and load acceptance remain NOT_RUN. This checkpoint is not IN01/IN05 acceptance.
