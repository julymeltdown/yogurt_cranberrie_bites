# FE15 local movement, camera and interaction intent

Status: **MOCK_READY**. This bounded increment runs against validated, synthetic C06 resources in installed Chrome. It is ready for integration review; it does not certify a real multiplayer flow, server authority, selected character artwork, physical device or arrival. FE17 transport/stream sequencing/reconciliation, BE12 authoritative movement, production shell wiring and real-flow gates remain **NOT_RUN** here.

## Assignment and provenance

Worker branch: `codex/mcr-fe-FE15`. Assignment base: `b1a19c425a30641410d71986714d72b7101f43e8`. Only the integrator publishes to master. The delivery commit is a descendant of this base; the accompanying hash-bound receipt and Git history identify its exact revision. This worker makes no production composition or contract changes.

All movement controls, simulation, selection, camera adapter, new tests and fixture UI are newly authored. No Bigyeting rendering or character images are copied. The sole existing application files changed are `frontend/src/world/runtime.ts` (optional typed attachment, visibility/block lifecycle) and `frontend/src/world/scenes/MapScene.ts` (actual camera projection). Existing map/resource validation and prior FE14 test fixtures are consumed read-only. `movement-data.ts` derives explicitly synthetic terrain from `__tests__/fixtures/synthetic.ts`; its three-swatch 96×32 PNG is test terrain, never approved product art. No dependency or lockfile changes.

The reviewed FE14 runtime/scene source revision is `77ae6bdf0c8b47d9468727befcea75a9af64234c`. Its inherited-close ownership exclusion, reentrant-generation guards, disabled Phaser input plugins, global-handler containment and owned listener cleanup remain in place. The retained native runtime suite validates these after FE15.

Read-only source fingerprints at the assignment base/current unchanged sources:

| Source | SHA-256 |
|---|---|
| `contracts/realtime/v1/policy.json` | `3e77e21b036386803c7d5bff0145c434f111ba51b861219a211e853a43d0628b` |
| `contracts/realtime/v1/world.schema.json` | `1732e627037fe91533ac6f7fe74e36683ce9b846bfbfc020b3cce298ae84df36` |
| `contracts/assets/v1/map-manifest.schema.json` | `ed01d26b2f561c9885ad3fc5fe66912ee8175334393e49a85652756e301d6800` |
| `contracts/assets/v1/theme-manifest.schema.json` | `7e4b0d9a8e2e21ffe76172870ea5ef44b5282e8f7852671862889c492f38b1d5` |
| `frontend/src/world/map/manifest.ts` | `575b89163952959bdc20760a9ab19cafe6c950e7ffeb48e3a00bf3305ac8b4a2` |
| `frontend/src/world/map/resources.ts` | `829e8e8c901223307e5ba8bcf4a306e01e12f7d09f7b08bc9c4f884c0d0ead4a` |
| `frontend/src/world/__tests__/fixtures/synthetic.ts` | `7f0326dba49112d7297cfed23bd38ef5262605cbd7babc90fef47b4835dd96d7` |

## Behavior and boundaries

`attachMovement` receives an owned canvas/camera, already validated `LoadedMap`, abort signal, controls container, injected local policy and a consumer of `Pick<C05 Input, axisX | axisY | buttons>`. Its optional immutable prediction callback exposes the bottom-center foot and selection state for a future approved renderer. No stream metadata, epochs, sequences, credentials, socket, authoritative position or domain mutation is constructed. C05's existing 20Hz simulation target drives bounded 50ms prediction steps; diagonal intent length is capped at one. At most 100ms is accumulated per frame, without background catch-up. Neutral safety notifications may occur immediately outside the cadence; FE17 must coalesce/rate-limit actual wire sends under C05's 30/s cap. This module never calls `sendInput`.

C06 has no interaction-radius field. `docs/mcr/assets.md` leaves speed, foot and range to BE12/FE15. They are required injected values, not schema amendments. The synthetic browser policy is **120px/s, 8×6px foot, 42px interaction radius**, camera zoom 1 initially. Validation bounds speed to (0,1024], each foot dimension to [1,tile dimension], radius to (0,512], and zoom to [1,8]. These are local implementation safety bounds, not frozen server tuning. Policy values are copied before use.

The whole rectangular foot stays within world bounds and clear collision cells. Sweeps advance by at most half the smallest tile/foot dimension, preventing tunneling; axis separation permits wall sliding. Four-neighbor BFS tap routes check whole-foot sweeps on every edge and reject blocked, outside or unreachable destinations without cutting corners. Route traversal uses normalized/fractional axes and stops at its target. Geometry is validated before input allocation through existing C06 schema, hashes, normalized Tiled agreement and resource checks; the local collision helper also checks numeric/grid shape constraints.

Nearby targets come from the loaded C06 interaction/place lists, require an OPEN place, injected distance and unobstructed whole-foot route segment, and sort deterministically by distance and ID. Selection is cleared/replaced as eligibility changes. E and the action button queue at most one `interact`, consumed once only while still eligible. Client eligibility does not grant server authorization or establish real-world presence. The selected local ID is available in the prediction snapshot; C05 intent contains only permitted axes/buttons.

Keyboard movement requires exact canvas/joystick focus. WASD/arrows move, E interacts, R recenters, Escape clears. Modifier combinations, composition, repeated initial keydown, editable fields, selects, ordinary buttons and dialog descendants cannot start movement. Key repeat cannot repeat one-shot interactions. Pointer capture owns one pointer across joystick/map input; other fingers cannot steal it. A tap must remain within 10px and 500ms. Joystick uses a 12% dead zone. Pointer up/cancel/lost capture, blur, document visibility changes, blocking, abort and disposal clear held axes, routes and queued actions. Resuming requires fresh input.

DOM controls own their listeners, pointer capture, RAF, attributes and styles. Buttons/select have at least 44px targets, visible keyboard focus, labels and status. Short screens scroll to controls without horizontal overflow. Native tests scroll each button fully into view at 420px height, then exercise actual touch joystick movement/cancel and a touch Recenter activation. No decorative left accent lines or substitute rails are used.

The existing Phaser instance remains the sole renderer. No Phaser input plugin is enabled. The camera port uses the actual canvas CSS rectangle, physical canvas dimensions and Phaser camera `getWorldPoint` after public `preRender` updates; inverse projection is valid for the retained unrotated camera. Follow tracks local predicted foot, look-at suspends follow, Recenter restores it, zoom and resize preserve the selected center within map bounds. No real character artwork or sprite implementation is supplied.

Runtime controls attach only after engine readiness; pre-start block/visibility persist. Abort immediately clears input, then controls dispose before engine teardown. Attachment factory/enable callbacks are guarded against stop/dispose/restart. A monotonically increasing visibility revision also prevents an older same-owner hide/show continuation from overriding a newer reentrant change. Throwing enable/disposal cannot strand runtime resource release. Existing no-controls callers retain FE14 behavior.

## Verification

Environment: Node 24.22.2, pnpm 11.7.0, installed Chrome 156.0.8078.80, Phaser 3.90.0, Python environment `/tmp/mcr-fe14-format-venv/bin/python` with registered contract dependencies. Test browsers use ephemeral owned contexts/ports; no user profiles or external services. Commands run from the worker root unless specified; logs and screenshots are local evidence, not tracked product assets.

| Command | Exit / evidence |
|---|---|
| `corepack pnpm@11.7.0 --dir frontend install --frozen-lockfile --offline` | 0; `/tmp/mcr-fe15-install.txt` |
| `MCR_FE14_PYTHON=/tmp/mcr-fe14-format-venv/bin/python corepack pnpm@11.7.0 --dir frontend test` | 0; 940 tests / 22 files, including 69 FE15 tests; `/tmp/mcr-fe15-full-tests-final.txt` |
| `corepack pnpm@11.7.0 --dir frontend build` | 0; `/tmp/mcr-fe15-build-corrected.txt`; retained 544.82kB main chunk warning |
| `corepack pnpm@11.7.0 --dir frontend lint` | 0; `/tmp/mcr-fe15-lint-corrected.txt` |
| `MCR_FE15_EVIDENCE=/tmp/mcr-fe15-evidence/final node src/world/__tests__/movement-browser-smoke.mjs` (frontend cwd) | 0; all four widths PASS, same-owner native loop awake/asleep PASS; `/tmp/mcr-fe15-browser-final.txt`, final report/screenshots |
| `MCR_FE14_EVIDENCE=/tmp/mcr-fe15-evidence/retained-fe14 node src/world/__tests__/runtime-browser-smoke.mjs` (frontend cwd) | 0; `/tmp/mcr-fe15-retained-runtime.txt`, retained report/screenshots |
| `python backend/scripts/docs/generate-mcr-core.py --check` | 0; `/tmp/mcr-fe15-core-generation.txt` |
| `python scripts/mcr/validate-core.py` | 0; 33 fixtures, producer runtime NOT_RUN; `/tmp/mcr-fe15-core-validation.txt` |
| `python scripts/mcr/validate-assets.py` | 0; metadata CONTRACT_READY only; `/tmp/mcr-fe15-assets-final.txt` |
| `python -B -m unittest discover -s scripts/mcr/tests -p test_assets.py` | 0; 27 tests; `/tmp/mcr-fe15-assets-regressions.txt` |
| `node scripts/mcr/check-ui-origin.mjs` | 0; C08 PASS; `/tmp/mcr-fe15-origin-final.txt` |
| `python scripts/mcr/check-plan.py` | 0; `/tmp/mcr-fe15-plan-final.json` |
| `python scripts/mcr/check-boundaries.py --base b1a19c425a30641410d71986714d72b7101f43e8 --lane frontend --allow-doc docs/mcr/frontend/world-movement.md` | 0; 14 changed paths PASS; exact manual assignment/import review completed |
| `git diff --check` | 0; final staged diff also checked |

Unit coverage includes bounded/finite prediction, whole-foot collision and corner rejection, routing, C06 closed/range/occlusion selection, one-shot clearing, actual C05 consumer schema validation, keyboard scope/composition/modifiers, pointer ownership, cleanup, failure latching, prediction/consumer reentrancy, attachment late readiness and same-owner visibility races. The visibility tests first failed in both directions (67 pass / 2 fail); `/tmp/mcr-fe15-visibility-regression-red.txt` retains the reproduction. Full 940-test success includes the fix.

Native coverage at 360×800, 390×844, 430×932, 1280×800 and each width×420 includes actual touch cancel/multitouch, focus/keyboard release, blocking panel typing, no held-input resume, queued-action clearing, sustained collision against a wall and a tap route through its opening, once-only E/button interactions, actual camera zoom/resize/project/unproject, repeated owner mount/stop, invalid map/policy visible errors, precise listener/RAF/observer/blob/capture cleanup and production isolation. At 390px it additionally exercises consumer-triggered stop/dispose/restart and reentrant show/hide against the real Phaser loop. Public build routes load no world controls, fixture, Phaser or API requests. Pixel/foot markers remain conspicuously synthetic.

Development failures are preserved, not counted as acceptance: first native invocation had the wrong runner path; second had a duplicate test variable; third attempted a touch on an offscreen joystick and timed out without movement (`/tmp/mcr-fe15-browser-third.txt`). The harness now scrolls the actual control into view and asserts full visibility/44px before short-screen touch. Fourth passed mobile sizes but assumed a desktop map that already fit at 1× would scroll; the corrected test zooms before checking bounded recentering. Runs fifth/sixth/eighth passed their then-current sources; seventh was interrupted with no report and no remaining process, so is not evidence of success. The final run and hash-bound receipt supersede those exploratory runs. Early unit parse/floating-point assertions and an intermediate TypeScript tuple-spread error remain in separate logs; the final full suite/build replace them.

No claims of IN01/IN05, real multiuser flow, BE12 collision agreement, real backend input/interaction authorization, selected artwork acceptance, touchscreen hardware or production enablement follow from these tests.
