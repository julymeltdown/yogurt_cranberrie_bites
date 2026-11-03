# FE07 / FE14 / FE18 production root wiring

Status: **MOCK_READY** for this bounded root-composition handoff. Real-service world, multiuser, selected-art canvas and IN01 acceptance remain **NOT_RUN**.

## Authority, provenance and scope

Delivery explicitly transferred five root paths for this assignment: `frontend/src/main.tsx`, `frontend/src/app/ports.ts`, `frontend/src/app/WorldShell.tsx`, `frontend/src/app/router.tsx`, and `frontend/src/app/WorldHost.tsx`. New implementation and tests are confined to `frontend/src/app/world-entry/`; this document is the only new documentation path. All code is newly authored glue over committed canonical Auth, FE02 privacy, C03 bootstrap, FE14 composition and the previously delivered entry resolver/FE18 bridge. No backend implementation, server movement constants, external checkout, copied rendering, manifest/lock, shared contract, runtime/avatar implementation, or source archive changes are included.

Assigned isolated checkout: `/Users/lhs/Desktop/mcr-worktrees/fe-world-entry`, branch `codex/mcr-fe-world-entry`. Required integration baseline: `22042ddd1e4d28a8165ba98c11ced4067d4978b0`. Baseline merge `7a29f3869aee6f1f7e2643f6c453151c5cc12188` restored the incoming master versions for two pre-existing add/add composition/document conflicts; its resulting tree was identical to the required baseline. The root implementation patch is its next commit. Delivery should cherry-pick only that patch, not replay the accepted entry/composition prerequisites or the merge. Previous entry candidate `9c3e9f2d0ea5d037094d0022427bdd24ea324acd` was already integrated as `b7a05b5`.

Delivery confirmed FE12 renderer prerequisite `c2149dae63d21324179be3dca7df4bc79a003881` is integrated as `1db1680`, but explicitly directed this bounded handoff to omit renderer registration until an actual approved binding exists. It is not a prerequisite of this independent patch and was not imported. Existing dependencies and retained lockfile resolutions are unchanged.

## Resulting ownership

`main.tsx` creates one canonical Auth controller, one production ports object and one router. The production world facade imports the actual `createWorldComposition` and `createWorldEntryResolver` only on an authorized world mount with valid configuration. It caches one composition; each actual mount has one independently revocable lease. It never constructs another Auth, socket, game or mock transport. Existing composition owns the actual game/transport when authorized resources become available. Public routes do not mount the world/provider or load Phaser. HMR application cleanup unmounts React, disposes router/ports and finally disposes Auth.

`WorldHost` supplies its actual mount teardown signal, the original captured shell privacy scope, and the bootstrap that scope authorized. A replacement bootstrap now also replaces this lifetime. Panel navigation and same-identity memory-token refresh do not replace it. The facade publishes that exact bootstrap and copies only primitive scope fields; it never captures a newer privacy scope to relabel old data. Its derived lifetime is aborted synchronously by mount teardown, Auth/FE02 invalidation, logout or application disposal. A retained replication adapter from an old binding cannot read or switch through a replacement binding. Delayed imports cannot mount against a retired lifetime. Source forwarding subscriptions detach with the mount.

`root-features.tsx` keeps a stable nullable provider tree while the actual `WorldHost` publishes or withdraws its binding. One bridge serves both default `/world/rooms` and `/world/nearby` panels. Provider publication does not conditionally wrap/remount the world host. FE18 still verifies the immutable C03 document, actor/event/privacy lease, capabilities and current filtered replication. Room catalogs and switch dispatch use canonical existing contracts; dispatch is not admission, and virtual presence is not physical arrival.

New authored files:

- `frontend/src/app/world-entry/configuration.ts`
- `frontend/src/app/world-entry/production.ts`
- `frontend/src/app/world-entry/root-features.tsx`
- `frontend/src/app/world-entry/__tests__/production.test.tsx`
- `frontend/src/app/world-entry/__tests__/root-browser.mjs`

## Deployment input boundary

`VITE_MCR_LOCAL_INPUT_CONFIG` is an explicit JSON string with exactly `revision`, `reviewedBy`, `movementPolicy`, and `zoom`. `movementPolicy` has exactly `speedPxPerSecond`, `footWidthPx`, `footHeightPx`, and `interactionRadiusPx`. The revision and reviewer must be nonempty strings of at most 128 characters; the JSON string is bounded to 4096 characters. Configuration is detached and frozen. Numeric policy and zoom bounds match the existing local FE15 validation envelope; the resolver additionally checks foot dimensions against the authorized map. Invalid, incomplete, extra-key, nonfinite, accessor or absent configuration fails closed. There are no built-in movement values or inferred server parameters.

A deployment must supply its reviewed local configuration. No deploy-approved policy is supplied by this patch; values in test harnesses are explicitly TEST ONLY. Review metadata records a deployment decision, not cryptographic approval or authoritative backend physics. This configuration contains no credentials and is visible in the client bundle.

Without valid configuration, the facade does not import composition or allocate game/socket resources and displays a safe unavailable notice. With valid configuration but unprepared or synthetic shipped resources, the actual resolver/composition fails closed. No original fixture, previous character image, generated design, fake atlas, replacement map, default avatar binding or renderer fallback is added. If future resources pass review but no renderer is registered, existing composition truthfully reports unavailable avatar artwork.

## Validation and evidence

Run from `frontend/` with Node `v24.22.2` from `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin` and pnpm `11.7.0`:

| Command | Exit | Evidence |
|---|---:|---|
| `pnpm exec vitest run src/app/__tests__ src/app/world-entry/__tests__ src/world/composition/__tests__ src/features/world-rooms/__tests__ --maxWorkers=1 --reporter=json --outputFile=/tmp/mcr-root-final-tests.json` | 0 | 266 tests / 12 files; `/tmp/mcr-root-final-tests.log` and JSON |
| `pnpm run build` | 0 | `/tmp/mcr-root-build.log`; existing large-chunk warnings remain |
| `pnpm run lint` | 0 | `/tmp/mcr-root-lint.log` |
| `node src/app/world-entry/__tests__/root-browser.mjs` | 0 | `/tmp/mcr-world-root-browser/report.json`, two 390px captures, `/tmp/mcr-root-browser.log` |

The 33 new focused tests cover configuration, lazy allocation, one owner, cached visibility/input, synchronous invalidation, delayed loading, safe failures, default route dispatch, StrictMode peak ownership, preserved host/binding through navigation, privacy/logout DOM withdrawal, public isolation and replacement-bootstrap teardown. The existing entry/resolver/bridge/composition/FE18/root suites provide the adjoining contract and lifecycle checks. An initial parallel run had two unchanged FE18 fixture timing failures (`transport.sockets.at(-1)` not ready after its fixed microtask flush); the isolated 21-test rerun and final serial combined run pass. No shared fixture was edited, and the separate parent timing-fix commit was not imported.

The native Chrome runner uses the actual shipped `main.tsx`, canonical browser Auth/sign-in, actual C03 decoder, default root routes and actual default composition/resolver. Only HTTP responses and test-only deployment input are mocked; supplied bootstrap art metadata is left unprepared. It verifies public lazy scripts, safe missing-config and rejected-art states, one host through rooms/nearby, no Phaser request/canvas/ticket/world asset/world socket, one bootstrap request, no horizontal overflow, clean page errors, and production-preview public bundle isolation. Vite's HMR connection is not counted as a world socket. Its JSON report includes source SHA-256 hashes. It does not qualify a real backend, approved art, actual Phaser canvas, multiuser admission, real privacy-event delivery, mobile device or release readiness.

Run from repository root:

- `python3 scripts/mcr/check-plan.py` — exit 0; 13 source archive checksums unchanged; planning/provenance only (`/tmp/mcr-root-plan.log`).
- `node scripts/mcr/check-ui-origin.mjs` — exit 0; static origin/import scan only, no reused UI or decorative accent styling introduced.
- `python3 scripts/mcr/check-boundaries.py --base 7a29f3869aee6f1f7e2643f6c453151c5cc12188 --lane frontend --allow-doc docs/mcr/frontend/world-root-wiring.md --allow-composition frontend/src/main.tsx --allow-composition frontend/src/app/router.tsx` — exit 0; exact two centrally classified paths were explicitly transferred; the other three transferred roots remain frontend lane paths.
- `git diff --check` — exit 0.

Local `/tmp/` receipts and screenshots are execution evidence, not committed product resources. The committed browser runner reproduces them. Handoff carries the exact patch commit and changed-path receipt; worker does not push. Real reviewed local input policy, prepared selected-pack avatar/map/theme resources, renderer binding and authorized backend integration remain external prerequisites for a rendered connected-world gate. No task or IA scope is removed and no IN01/IN05 acceptance is claimed.
