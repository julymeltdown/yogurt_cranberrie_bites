# FE15 application input delivery

## Original task disposition

Original FE15 client/application criteria are **INTEGRATED** with native browser qualification below. The remaining application journey identified by Root reconciliation `df0842a6` and `10784b26` is closed: actual application routes, WorldShell, production ports, composition, input controller and default Phaser engine are exercised together. This is not IN01, IN05, QA06 physical-device, QA07 accessibility or release acceptance. Their existing owners retain those decisions. No unrelated backend/operator/device prerequisite is added to FE15.

| Original FE15 requirement | Current evidence |
| --- | --- |
| Pointer-safe joystick/tap movement | Trusted Chrome touch input, pointercancel and lostpointercapture; neutral wire output and stationary post-cancel self; actual canvas tap route completion at 360/390/430. |
| Keyboard control | Focused canvas arrows and WASD move and stop; real browser blur stops; profile keyboard events cannot create movement/interact intents. |
| Collision prediction | Actual application taps into observed visible blocked map geometry are rejected without self movement; existing movement simulation/whole-foot collision and route tests pass. |
| Recenter and selection | Actual DOM nearby selector and Interact emit one intent; actual Recenter control and keyboard R restore follow mode. |
| Blocking-panel pause | Opening the real profile route while moving pauses input, inerts the world and retains the incumbent socket/game/camera. |
| Keyboard opening and panel isolation | Actual focused profile textarea, sequential WASD/E/R/arrows, visualViewport resize 844→430→844, then route removal and enabled world return. This emulates keyboard viewport resizing; it does not open an iOS/Android OS keyboard. |
| Lifecycle/privacy | One total/peak game, same camera, one socket across panel navigation; privacy suspension removes controls/canvas/socket and final disposal revokes all blob URLs and clears source-cache bytes. Input listener ledger retains only test logger and Playwright hit-target interception. |

## Defects and scoped implementation

Before-fix receipt `frontend/src/world/__tests__/fe15-application-input/evidence/before-fix.json` and screenshot preserve the actual 390px failure: after removing a focused profile textarea, the world remained inert/joystick disabled even though the route and replication were live. The full blocking panel also replaced the room connection (two total sockets).

* `WorldShell.tsx`: reread committed `document.activeElement` on location changes. Removing a focused field need not emit blur; stale typing state must not block the next route.
* `composition/index.ts`, only `setVisible`: panel coverage hides/pauses the engine without withdrawing replication. Actual document backgrounding still withdraws presence and resumes using a fresh ticket.
* Only the existing composition test “hides by disconnecting/evicting…” was revised to distinguish covered-panel retention from document-hidden withdrawal, fresh-ticket resume and the same game.

Initial exclusive assignment: new `frontend/src/world/__tests__/fe15-application-input/**` and this document. WorldSL `01a1a1fe-c276-7eb3-a36e-2f383c0e81b8` explicitly transferred the two named production hunks before edits; Root153 and FE07 owner confirmed no competing writer. WorldSL subsequently allocated only the named existing composition regression hunk. No whole-file ownership is implied. A separate, explicit FE07 support allocation covers only PanelFrame's scroll capture/restore effect; that support is tracked separately below and does not transfer original FE07 qualification ownership.

## Provenance and reproducibility

Assignment base: `4b327722aaa5697a10bbf8ecc3d1c4c82ecde9bc`; isolated branch `codex/mcr-fe-FE15-application-input`. `preview.tsx` adapts the MCR-owned Root source fixture from `10784b26` into the new owned test prefix; original fixture files remain unchanged. It imports the frozen Root `data.ts`, `qualified-themes.ts`, source packet and authentic selected-character resources. Artist source remains `bc731ed8` spatial-v3/quality-v6; original top-level provenance is preserved. Backend-derived engineering theme qualification is used only by the explicit source-only test resolver. It does not admit the original artist pack into Product or bypass production admission.

The fixture runs real auth/session/controller, application routes, production ports, WorldShell, default Phaser, input, renderer, camera, cache and privacy owner. HTTP/Auth/C05 peers are explicit test fixtures. The C05 peer advances a fixture self from actual received Input using the injected local collision policy and sends validated Delta frames; it is not evidence of live server physics, authorization, networking, concurrency or physical arrival. No live endpoint or device was used.

Retained dependencies/lockfile are unchanged: Node24.22.2, Phaser3.90.0, existing React/Vite/Playwright stack. Browser version, SHA256 source pins, frame samples, trusted events, viewport measurements, target bounds and capture hashes are in `evidence/native.json`. The report candidate identifies the pre-commit base; its SHA256 pins identify the actual tested changed sources. All images are native browser captures of the application. No artwork was generated or replaced. `docs/source/`, backend, public runtime, contracts, root routing, manifests and lockfiles are unchanged by this patch.

From repository root, with the retained Node24.22.2 toolchain and installed dependencies:

```sh
node frontend/src/world/__tests__/fe15-application-input/native.mjs
cd frontend
MCR_FE14_PYTHON=<contract-validation-python> node_modules/.bin/vitest run src/app/__tests__ src/app/world-entry/__tests__ src/world/__tests__/movement.test.ts src/world/__tests__/runtime.test.ts src/world/network/__tests__ src/world/mural-entry-lifecycle/__tests__ --maxWorkers=2 --reporter=dot
MCR_FE14_PYTHON=<contract-validation-python> node_modules/.bin/vitest run src/world/composition/__tests__ src/app/__tests__/shell.test.tsx src/app/__tests__/reference-root/root.test.tsx src/world/__tests__/movement.test.ts src/world/mural-entry-lifecycle/__tests__/assembly.test.tsx --maxWorkers=2 --reporter=dot
node_modules/.bin/tsc -b --pretty false
node_modules/.bin/vite build
node_modules/.bin/oxlint src/app/WorldShell.tsx src/world/composition/index.ts src/world/composition/__tests__/composition.test.ts src/world/__tests__/fe15-application-input/preview.tsx
```

Validation: native Chrome156.0.8078.80 all three widths PASS; focused six files/168 tests exit0; broader24 files/742 tests exit0; TypeScript exit0; scoped lint exit0; Vite production build exit0 (existing chunk-size advisory); planning check exit0 with13 source archives unchanged. The first unbounded parallel broad test attempt timed out seven async UI queries under simultaneous browser/build load; the bounded two-worker rerun passed all742 without changing those tests. Before-fix native failure remains preserved; intermediate driver failures are not product acceptance evidence. Vite/browser/cache finally cleanup is required by the runner.

## FE07 support and publication

FE07 owner `01a1a1fe-1ab7-79b0-adbc-d4c025701acb` independently reproduced native390 gift-list scroll1894→detail→Back POP→0 while opener focus restored. Exact PanelFrame effect ownership was accepted before edits. Original FE07 owner retains its full journey, final criteria and task disposition; this lane owns only the allocated scroll fix and regression support.

Publication receipt and scroll support results will be appended after the scoped checkpoint and final checks. Direct nonforce master publication preserves peer history; there is no SL/king approval queue.
