# FE32 mural editing surface

Current original FE32 client criteria: **SATISFIED**, qualification **MOCK_READY**,
2026-11-04. Owner 204 requalified the actual current editor, corrected three-plus-
contact touch ambiguity and delayed-ack cooldown shortening, and passed 136
current affected tests plus all three native widths. Full criterion dispositions,
provenance, current source pin `21ceef60`, 15 captures, visual comparison and precise
integration limits are in [the current delivery report](fe32-client-delivery.md).
No original client behavior remains identified as unfinished. Live producer,
FE33/IN04, physical devices and whole-reference acceptance remain separate.

The following original owner report is preserved as historical provenance. Its
uncomposed-route statements describe its old base, not current app composition;
current native results certify the component/HTTP fixture boundary explicitly.

## Historical original owner report

Status: **MOCK_READY**, 2026-11-04 (Asia/Seoul). This is an independently usable
editor with deterministic command-port fixtures. Production composition, HTTP
receipt recovery, FE33 synchronization/world texture, real collaboration and
IN04 acceptance are **NOT_RUN**. No application route or production mock was
installed.

## Ownership and provenance

- Writer: `mcr(fe) FE32 mural editor`, chat `01a1a213-61d6-7a20-a8e7-476649312f8c`.
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/fe-mural-editor`.
- Branch: `codex/mcr-fe-FE32`.
- Base: `20503f4b698a28105e718c36e38cfa8fd8fb4a44`.
- Assigned changes: `frontend/src/features/mural/editor/**` and this document.
  The commit containing this document records the exact delivered paths and
  ancestry; its SHA is supplied directly to delivery in the committed handoff.
- Requirements: `MCR_IMPLEMENTATION_PLAN_V2_EN.md` §§5.1, 6.6, 8.4 and FE32;
  `generated/TASKS_EN.json` FE32 and the separately owned FE33 task.
- C04 input: `contracts/domain/v1/mural.schema.json`, `operations.json` and
  `policy.json` at this base. The mural schema last changed at
  `b615e35bb07fb5797e6d7445a93aa6e1eeab7152`; its Git blob is
  `6a7a117fda4bbe3b260a144e341536bb842832be`. The port follows
  `placeConcertPixel` and the exact 128×128 / 16-color DTOs. No contract file
  changed; this record does not certify wider C04 acceptance.
- Canonical privacy input: `frontend/src/shared/query/privacy.ts`, last changed
  at `4207792d7f2ae1b1b6676e724e1712e43d72de43`, blob
  `5af53399b632c8b3a73c719a20ee68cb1e8d461b`. Production imports only its types;
  deterministic fixtures instantiate that actual privacy coordinator and the
  shared in-memory canonical session implementation.
- All editor logic, UI, CSS, tests and geometric pixel fixtures were newly
  authored. No Bigyeting rendering/headless modules were copied. No character
  images, original source-pack files, source archives or generated planning
  files changed. Existing React, React DOM, Zod, Vitest, Vite and Playwright
  dependencies were used; manifests and retained lockfile resolutions changed
  neither in this worktree nor the shared installation.

## Behavior

`createMuralEditor` owns one actor/event/privacy-generation scope. Its snapshot,
preview, selected color, command and deadline remain in memory. The React
`MuralEditor` renders a fixed 512×512 canvas backed by a 128×128 offscreen raster,
at most 256 grid lines, 16 palette controls and ordinary coordinate inputs.
There is no per-pixel DOM, texture subscription, socket or new Phaser instance.

A tap selects a preview; a drag of at least six CSS pixels pans and never
selects, even when dragged back to its starting point. Multitouch, long press,
pointer cancellation, lost capture, window blur, page hiding and Escape cancel
the gesture. Normal automatic capture release after a tap preserves the preview.
Zoom is explicitly controlled from 1× to 8×; fit and pan are clamped. Browser
page zoom is available outside the canvas. Pinch-to-zoom inside the canvas is
intentionally treated as ambiguous input; use the zoom buttons.

Arrow keys and coordinate fields provide DOM alternatives. Enter on the canvas
or the coordinate form only previews. Editing a coordinate clears the old
selection. Placement requires activating **Confirm pixel**, with the selected
coordinate/color and current revision visible. All button/input targets are at
least 44×44 CSS pixels. Palette indexes, pressed state, coordinates, confirmed
color and text status communicate state without relying on color alone. New
styles use MCR paper/ink tokens, ordinary symmetric control borders and necessary
focus outlines; there are no decorative left accents, artwork fallbacks or
motion dependencies.

The controller sends one frozen `PixelCommand` with operation, idempotency key,
exact body and privacy scope. It changes no confirmed pixel or revision while
the command is sending, pending or unknown. An acknowledgement must pass the
closed C04 DTO schema, match the event/coordinate/color and have exactly the
command's expected revision plus one. Only a contiguous acknowledgement updates
the local confirmed raster. A newer supplied snapshot is never overwritten by
a delayed acknowledgement. FE33 owns reconciliation beyond this boundary.

Confirmed timestamps establish a conservative, monotonic cooldown display; the
server still owns cooldown, quota, authorization, locks and durable revision.
Snapshot `PAUSED`/`FROZEN` and explicit loading/read-only/error availability
disable selection and confirmation. Terminal server rejection discards the
preview. Changed permissions, locks, cooldown and conflicts require a verified
refresh before editing again. Unrecognized reasons, idempotency expiry/conflict,
5xx-style uncertainty and invalid acknowledgements retain the original command.

The command port has a 15-second default deadline (configurable from 1–60,000 ms).
A timeout aborts local work and marks the outcome unknown; abort never promises
that a server mutation was cancelled. **Check original command** invokes only
the read-only `check` port. It never replays a placement, creates a new key or
unlocks merely because a refreshed snapshot arrived. A pending/unknown result
continues to block new placements. No automatic retry/reconnect mutation exists.

Canonical privacy cleanup aborts outstanding work, clears previews, raster,
color, commands and scope-bound state synchronously, and fences late results.
The existing controller becomes unusable and must be recreated after
revalidation. Timer/abort listeners are removed even when the port ignores
abort. Unmount removes ResizeObserver, page/blur listeners and the UI timer;
the shell must explicitly dispose its controller when its lifetime ends.
Detached raster canvases have no external resources and become collectible.

## FE33 handoff port

Use the exports from `features/mural/editor/index.ts`; do not import a fixture.
The shell constructs one controller with its existing canonical privacy
coordinator and a real `MuralCommandPort`, then passes it to `MuralEditor`.

1. FE33 must fence asynchronous loads with the original captured `PrivacyScope`
   and call `acceptSnapshot(scope, verifiedSnapshot)`. Stale scopes/events and
   decreasing revisions are rejected. The schema validates bounded, closed DTOs
   and copies/freezes the arrays. The scope supplied must be the scope of the
   request, never a newly captured identity attached to an old response.
2. On restore or another required reset, discard obsolete deltas in FE33 and
   pass `acceptSnapshot(scope, newSnapshot, { reset: true })`. This clears local
   preview while preserving unresolved commands. Do not infer reconciliation
   completion or a command's outcome from pixel equality in a snapshot.
3. Use `setAvailability(scope, 'loading' | 'read-only' | 'error')` for external
   admission/event closure, sequence gaps and transport failure. A read-only
   event gate survives later snapshots and transport error/refresh transitions.
   Only explicit `setAvailability(scope, 'writable')` reopens that external gate,
   after fresh verified authority; snapshot PAUSED/FROZEN still prohibits writes.
4. `place(command, signal)` performs exactly one canonical authenticated
   `placeConcertPixel` request with the command key/body. The adapter must verify
   response route/status and actor/event/generation binding, then return
   `acknowledged`, `pending`, a verified terminal `rejected`, or `unknown`.
   Transport failure, abort and timeout do not prove no write occurred.
5. `check(command, signal)` reads the exact actor/event/operation/key status
   through `getConcertCommandStatus`, checks the entire receipt binding and
   current authorization, and never POSTs. C04 status metadata is not a
   `PixelOperation`: after a successful receipt FE33 needs an independently
   verified matching original operation/result before returning `acknowledged`.
   It must not manufacture a pixel result from the local preview, or use a
   snapshot's color alone as proof. Keep unknown/pending safely blocked if that
   evidence is unavailable. Retired/expired receipt does not authorize replay.
6. FE33 owns ordered snapshot/delta recovery, reset/lock-control handling and
   world-wall texture publication. This editor creates no world connection and
   does not claim that its local raster represents multiuser convergence.

The shell should retain this controller across ordinary panel navigation while
an outcome is unresolved. Disposing it or reloading the document intentionally
scrubs in-memory command metadata; durable cross-reload receipt recovery is a
separate composition/FE33 obligation, not a persistent editor draft. No tokens,
command bodies or private state are stored in browser persistence or telemetry.

Proposed next ownership, requiring delivery assignment: FE33
`frontend/src/features/mural/model/**`,
`frontend/src/world/interaction/mural.ts`, and an explicitly assigned
`docs/mcr/frontend/mural-synchronization.md`. Root routes, shared APIs/contracts,
manifests and composition remain delivery-owned. This FE32 handoff does not
acquire any of those paths.

## Verification

Commands ran in `/Users/lhs/Desktop/mcr-worktrees/fe-mural-editor` using Node
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node` (24.22.2).
Delivery authorized a temporary, untracked `frontend/node_modules` symlink to
the qualified `/Users/lhs/Desktop/mcr/frontend/node_modules`; it changed no
dependencies. Substitute the absolute pinned Node path for `NODE` below.

| Command | Exit | Evidence |
| --- | --- | --- |
| `python3 scripts/mcr/check-plan.py` | 0 | Planning provenance/exporter checks PASS; unchanged archived source hashes and 112-task/61-required-IA accounting. Does not certify application behavior. |
| `NODE frontend/node_modules/vitest/vitest.mjs run frontend/src/features/mural/editor/__tests__/controller.test.ts frontend/src/features/mural/editor/__tests__/input.test.ts` | 0 | 55 tests in two files: selection/confirm separation; invalid bounds/colors; server-relative cooldown; pause/freeze/availability; read-only gate persistence across snapshots; rejection rollback; unknown/pending/read-only recovery; exact acknowledgement context; privacy changes; late response fencing; uncooperative-port deadline and resource cleanup; pointer cancellation/disambiguation and bounded transforms. |
| `NODE frontend/node_modules/typescript/bin/tsc --project frontend/tsconfig.json --pretty false --incremental false` | 0 | Whole frontend strict typecheck including the new surface, fixtures and tests. |
| `NODE frontend/node_modules/oxlint/bin/oxlint frontend/src/features/mural/editor` | 0 | No scoped warnings/errors. |
| `NODE frontend/src/features/mural/editor/__tests__/browser-smoke.mjs` | 0 | Actual installed Chrome 156.0.8078.80, headless Chromium engine, widths 360/390/430 and height 844 with touch/mobile/reduced-motion emulation. |
| `git diff --check` | 0 | Whitespace check. Delivery also checks staged path ownership before integration. |

Browser evidence is under
`frontend/src/features/mural/editor/__tests__/evidence/browser/`:
`report.json` records the tested base, source SHA-256 hashes, engine version,
widths and assertions. Each width has `preview`, `unknown`, and `frozen` PNGs.
All three widths passed real browser mouse drag out-and-back, Chromium CDP
multitouch/touch cancellation, automatic capture release, Escape, arrow input,
keyboard confirmation, coordinate edge, server cooldown/rejection, unknown
recovery with unchanged placement count, pause/freeze/read-only and privacy DOM
scrubbing. All controls measured at least 44×44, no horizontal overflow, fewer
than 130 editor DOM nodes, and no browser runtime errors. The 390px preview was
also visually inspected.

Initial browser harness runs were corrected for touch protocol sequencing and
ambiguous status locators; the final report binds the final source. A browser
probe also exposed automatic lost-capture cancellation after a successful tap;
the input controller now distinguishes an ended pointer from unexpected lost
capture, with a dedicated regression test.

Production bundle/root route composition, actual HTTP/server commands, live
cooldown races, region moderation, restore convergence, two-client flows,
iOS Safari/Android Chrome hardware, screen-reader hardware, memory/FPS/load and
IN04 remain **NOT_RUN**. This bounded evidence establishes **MOCK_READY**, not
INTEGRATED or ACCEPTED. Only delivery integrates/pushes the committed handoff to
`julymeltdown/mcr/master`.
