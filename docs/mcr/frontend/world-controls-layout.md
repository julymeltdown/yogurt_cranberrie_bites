# FE15/UIV-WORLD compact movement controls

Date: 2026-11-04. Status: **MOCK_READY** presentation handoff. Actual default-shell production modules and Phaser were exercised with test-only resources and HTTP/MockSocket. No full reference, artwork, real server, two-user, device or movement-prediction acceptance is claimed. The constrained-height root scene clipping described below remains a separate FE07 integration gate.

## Scope, ancestry and source

Isolated worker: `codex/mcr-fe-world-controls-layout`, base `8c0e7d39a1d33073e1a37a537567f10347dd64b4`. Previous Social checkout remains stopped and unchanged. Exact existing writable files are `frontend/src/world/input/controller.ts` and `controls.css`; new tests/evidence are `frontend/src/world/__tests__/controls-layout/**` and `controls-layout.test.ts`, plus this document. Main owns integration/publication; FE07 owns root/composition. No other application file, model, simulation, camera, network, assets, contracts, manifest or lockfile is modified.

Authority: implementation plan §§5/8, FE15 and QA07; assessment/execution plans informed allocation only. The user's exact ten-screen reference was inspected at `docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg`, SHA256 `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`, as was the existing qualified default-world capture. Original supplied/reference/source assets are unchanged. All new layout and fixture markup is authored here, with no legacy rendering or new art.

Baseline commit `a58fa90` preserves actual default-shell before captures at three widths and height 430, plus the original source-hashed native report/script. Its report binds production sources to base `8c0e7d`; the script itself is recoverable from `a58fa90`. Later native harness refinements add explicit clipped-scene measurement, standalone fixture pins and a guard rejecting `--before` if either production file differs from the original base. Before receipts are never regenerated from compacted controls.

Runtime imports stay on the existing `createMovementModel`, camera control and attachment interfaces. New tests consume existing synthetic FE15 terrain fixtures and actual default-shell qualified fixture read-only. Test geometry/PNG and the yellow local marker are explicitly synthetic, not approved character/venue art or server positions. Existing Node 24.22.2 dependencies were borrowed through a temporary `frontend/node_modules` symlink; no install, dependency/cache expansion or lockfile update occurred. The symlink is removed before handoff.

## Presentation change and preserved behavior

Closed presentation consists of a 72 px joystick, primary Interact/Recenter actions, and a native 44 px **Controls & help** disclosure. Zoom, accessible nearby selection and full keyboard/tap instructions remain available inside that disclosure. Its content scrolls within a bounded height and stays in normal document flow; it does not overlay or capture scene input. Button names, joystick/map accessible names, nearby selection and all keyboard/DOM alternatives are retained. Long target labels wrap rather than being silently truncated; errors and unusually long content may intentionally increase height.

Routine readiness/paused status remains a polite screen-reader observation without consuming a visible paragraph. Input/prediction failures and unreachable-tap feedback remain prominent outside the disclosure, including when it is closed. The physical-arrival sentence was removed from movement help at Main's explicit direction: FE07 retains the single root-owned virtual-position-does-not-confirm-arrival notice.

The patch rearranges DOM and styling only. Pointer ownership/capture/cancel, focus/blur/visibility, neutralization, RAF scheduling, input pause, one-shot interaction, model/camera calls and teardown remain the same. No speed, collision, interaction radius, physics, prediction duration or server receive-time assumption was introduced. The separate FE17 finding remains: ack sequence is not movement duration, and local FE15 tuning cannot become a qualified server movement profile.

## Measured default-shell space

Measurements use actual default routes, production lazy composition/resolver, canonical synthetic Auth, actual Phaser and deterministic C05 MockSocket. Both captures use the same root source and approved test harness; only the assigned controls changed. Width/normal height pairs are 360×800, 390×844 and 430×932.

| Width | Before controls | After closed controls | Before scene | After visible scene |
| --- | ---: | ---: | ---: | ---: |
| 360 | 341.95 px | 136 px | 148 px | 354 px |
| 390 | 323.56 px | 136 px | 211 px | 398 px |
| 430 | 323.56 px | 136 px | 299 px | 486 px |

All primary and disclosure targets meet 44×44 px. No horizontal overflow was observed at any width. The reference's scene-first direction improves measurably; full 70%-usable-height or reference matching is not asserted from this bounded patch. Before/after world, closed/open help, short-height and failure captures plus SHA256 artifact receipts are in `controls-layout/evidence/`.

**Short viewport limitation:** at viewport height 430, the current root ancestors clip the scene to **0 visible pixels**, although the intrinsic canvas still reports height 120. The final report computes ancestor/viewport intersection explicitly; raw canvas height is not usable scene evidence. Controls remain compact and the secondary controls are reachable in the independently scrolling local-Phaser fixture, but the default short root is not visually accepted. FE07 was notified of the minimal hook: make constrained world content scroll while reserving a nonshrinking minimum stage/scene; current inline `flex: 1` / `minHeight: 0` and ancestor clipping collapse it. This worker does not escape component CSS or edit root/composition to mask that defect. Main/FE07 own combined verification after their disjoint compact-root patch.

## Commands, results and evidence

From `frontend/`, with retained Node on PATH:

```text
MCR_FE14_PYTHON=/Users/lhs/Desktop/mcr-tools/contracts-validation-venv/bin/python \
  node node_modules/vitest/vitest.mjs run src/world/__tests__/movement.test.ts \
  src/world/__tests__/controls-layout.test.ts src/world/__tests__/runtime.test.ts
node node_modules/typescript/bin/tsc --noEmit --incremental false
node_modules/.bin/oxlint src/world/input/controller.ts src/world/__tests__/controls-layout.test.ts src/world/__tests__/controls-layout
```

All exit **0**: **227 tests across three files**, including existing movement/runtime regressions and five new presentation/accessibility cases; strict whole-frontend TypeScript and scoped lint. The retained Python environment supplies the required asset-format checkers without an install or weakened validation.

From repository root:

```text
node frontend/src/world/__tests__/controls-layout/native.mjs
python3 scripts/mcr/check-plan.py
git diff --check
```

All exit **0**. Native Chrome `156.0.8078.80` exercises 360/390/430 default routes plus height 430: 136 px closed controls, actual canvas, keyboard disclosure/zoom/focus return, panel pause/resume, persistent single game/socket and disposal. The secondary actual-Phaser fixture exercises positive nearby DOM selection, one-shot interaction, pointer cancellation/neutralization, pause without stale movement, visible consumer failure with help closed, short-height secondary-control focus/scroll, and disposal. No browser page errors. Normal/default, local short-help and closed-help failure screenshots were visually inspected. Native fixtures run through Vite development serving; they are not a live backend or mobile-device journey. The final native report is PASS for these bounded checks and explicitly records the default short-root limitation.

The actual root Vite production build also exits **0**, using a temporary output directory removed after checking; its existing large-chunk warning remains. This proves compatibility/buildability, not delivery of approved art or root short-height acceptance. Planning validation checks provenance only. `docs/source/`, the reference image and retained package files remain byte-identical.

Early development failures are preserved in this record: the first native invocation resolved the frontend root one directory too high, then was corrected before baseline capture. One command used repository-relative paths from `frontend/`, so its edits/new-test creation did not execute; it was rerun from the correct checkout root. The initial broad runtime run used system Python and failed the two asset-format checks for missing `date-time`/`uri` checkers, with 212 passes and 13 skips; the retained qualified Python environment rerun passed all 227. None was an application fix or acceptance pass. The committed before/after reports supersede intermediate after observations; the immutable before checkpoint remains intact.

## Delivery

The final committed handoff contains this document, two production files, the new unit test, local native preview/harness and source/artifact-hashed captures/reports. Compare its changed paths against base `8c0e7d` before serial integration. No worker push, root ownership transfer or additional review gate is implied. The worker stops after committed handoff and responds to concrete fixes in this chat; FE07 root-space and live/device/art gates remain with their existing owners.
