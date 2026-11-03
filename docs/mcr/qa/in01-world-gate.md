# IN01 real two-user world gate harness

**Real IN01: NOT_RUN.** This patch supplies real-only harness source, offline helper regression and an explicit failing preflight. It does not accept IN01, approve art, qualify two users, or claim a rendered movement flow. Missing prerequisites never become a skipped or passing real test. No scope was removed.

## Ownership and provenance

Assignment: new isolated `/Users/lhs/Desktop/mcr-worktrees/qa-in01-world`, branch `codex/mcr-qa-IN01`, base `8bb878c979138d0f6b3a4b880edb394f41cf99be` (integrated production root handoff). Old frontend/root checkout is stopped and root ownership released. Exact writable scope is `frontend/e2e/real/world.spec.ts`, new files under `frontend/e2e/real/world-gate/`, and this document. No app, backend, renderer, configuration, manifest, lock, root script or source archive edits are included. No uncommitted or external checkout is imported.

New files are the spec and `world-gate/{config.ts,observer.ts,decoder.ts,runner.ts,helpers.test.ts}`. The test imports generated C05 types and loads the existing canonical C05 decoder from this same committed checkout. The non-listening Vite module runner supports the app's JSON modules under Node 24; it neither serves the app nor supplies responses. Helpers' deterministic protocol data is used only in offline unit tests and a standalone decoder check, never by the real spec. No legacy UI or art is copied. Existing dependencies and lockfile are unchanged.

Source inspection covered the specification IN01 row and world acceptance requirements, `app/{routes.ts,router.tsx,WorldShell.tsx,WorldHost.tsx}`, `app/world-entry/production.ts`, `features/auth/ui/SignIn.tsx`, `features/theme/ui/{ThemeSelector.tsx,ThemeScope.tsx}`, `world/input/controller.ts`, `world/composition/index.ts`, `world/scenes/MapScene.ts`, the C05 transport and source-pack registry. At this pin the source-pack production atlas is `NOT_PREPARED`; default production composition intentionally omits the avatar renderer. Delivery confirmed there is no available valid isolated product release or pair of provisioned live accounts.

## How to run against an existing isolated release

Use Node `v24.22.2` and pnpm `11.7.0`. From `frontend/`:

```sh
MCR_IN01_CONFIG=/absolute/private/in01-config.json pnpm exec playwright test --config=e2e/real/world-gate/runner.ts
```

The runner starts no backend or app server, seeds no data, registers no account, migrates nothing and stops no external service. Provide an already running browser origin whose real HTTP and `/world-ws` routing reaches the declared isolated backend and reviewed product resources. The app must already have its event and reviewed local input configuration. Both provisioned accounts must be eligible, authorized for the same event/default room, mutually visible and able to move from their real spawn. The gate uses separate empty browser contexts and the shipped email sign-in form; it never injects cookies, storage state or access tokens. A failed login is not replaced by fabricated identity.

`MCR_IN01_CONFIG` must name an absolute, regular, nonsymlink file owned by the current user, mode 0600 or stricter, at most 16 KiB. Its closed JSON shape is:

```text
version: 1
isolated: true
origin: exact HTTPS origin, or loopback HTTP origin; no path/query/credentials
eventId: canonical event UUID
releaseRevision: declared 40-character source revision
accounts: exactly two { email, password, userId } objects for existing distinct identities
serviceReceipt: { path: absolute receipt file, sha256: exact file SHA-256 }
artReview: { path: absolute review receipt file, sha256: exact file SHA-256 }
```

No sample passwords, product account IDs, approved movement constants or art are supplied. Origin/revision/isolation/account declarations must be made by the operator; file-hash verification binds the supplied receipts but does not independently attest deployed binaries or confer art approval. The report labels the revision `declaredRelease`. Preserve deployment source/binary/database provenance in the service receipt and reviewed source-pack/map/theme/binding provenance in the art receipt. Manual current screenshot review remains separate.

Optional `MCR_CHROME_EXECUTABLE` selects an installed Chromium executable; default is `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`. This uses ephemeral contexts, never the user's browser profile. No Playwright network routing, synthetic response, service worker, art replacement, constructor patch, frame injection or application-global test control is used. Browser offline mode is the actual reconnect fault, not a simulated server frame. It must result in an observable closed connection; otherwise reconnect fails and cannot be credited.

The default output is `/tmp/mcr-in01-world-results`; use Playwright's `--output` for a fresh evidence location. `in01-result.json` is persisted mode 0600 and attached even on a normal prerequisite failure. It contains safe status codes, source/contract hashes, declared release and receipt hashes, and socket counters, without credentials, tokens, tickets, raw HTTP/frame bodies or roster names. Traces, videos, automatic screenshots and storage export are disabled. Actual world screenshots are attached only after two live reviewed-art worlds have opened; treat them as private gate evidence. Cleanup closes only the two harness contexts/browser and clears retained private observations. Application logout/session revocation is not claimed from browser-context cleanup.

## Required checks and honest limits

| Required boundary | Implemented observation or explicit prerequisite |
|---|---|
| Independent real Auth | Separate contexts; actual `/login` email/password controls; actual successful login response identity must match each different configured account. No token enters a test report. |
| Same authorized live world | Both C05 snapshots bind configured self/event; both permitted actors present; same actual room; exact connected renderer status and one canvas. Canvas is a presentation check only. |
| Movement in both directions | Actual focused joystick arrow input, bounded cardinal attempts; nonzero real client input plus changed peer C05 position for each actor, converging local/peer positions. This is wire observation; displayed-avatar motion still needs visual evidence. |
| Persistent shell/panels | Actual six navigation tabs; identical host/canvas nodes, unchanged socket-open count and peak one socket; focused Feed panel arrow input creates no new nonzero movement. This does not prove game object count. |
| Theme | Actual shell era link; if the shipped three radios are composed, change selection and compare room/map/avatar/position/socket invariants. Persisted preference, runtime skin replacement and retained geometry are still required, explicitly `NOT_RUN` until an approved observation/interaction adapter is reviewed. |
| Customization | Actual Me tab and Dress up link. At this pin it reaches `Dress up is not available yet`; gate records `AVATAR_EDITOR_NOT_COMPOSED`. Future save/preset controls are deliberately not invented. A reviewed source-derived adapter must save via UI, verify actual server revision/config change, same updated authorized avatar in both peers and profile/world agreement, and test cancel persistence. |
| Reconnect | Actual browser offline/online; require old socket close, real Reconnect world button, new server snapshot/stream with both authorized users, active/peak socket one and no invalid/gapped frames. No forged snapshot or hidden controller call. |
| One actual game | Always `NOT_RUN: READ_ONLY_GAME_LIFECYCLE_SEAM_REQUIRED` until an approved lifecycle observation is integrated. No canvas/global pointer is equated with game count. |
| Actual art visual | Capture actual world viewports only after live renderer checks; always require review of these current captures. A supplied receipt or screenshot by itself does not pass visual acceptance. |
| Teardown | Close actual contexts and require observed world sockets zero; withdraw in-memory frame data. Existing services remain untouched. |

`observer.ts` passively observes only the configured same-origin `/world-ws`. It decodes through the canonical C05 validator, discards authentication bodies, tracks overlapping socket opens, and applies only ordered same-stream/epoch snapshots/deltas. Gaps, foreign epochs and revoke/resync messages withdraw retained membership. Invalid frames are failures, not sanitized into success. Real observations and fixture-driven helper tests are kept separate.

All checks remain required in a single gate result. Any `FAIL` makes the result `FAIL`; any missing prerequisite makes it `NOT_RUN`; only every required check passing could make it `PASS`. The current missing UI adapters, lifecycle seam and screenshot review intentionally prevent a full green result. No command in this handoff executes a real two-user world or approves product art.

## Precise missing observability proposal (not implemented)

The integrator would need to grant an exact separate patch for:

- `frontend/src/app/ports.ts`: a read-only diagnostics port with `read()` and `subscribe()` returning immutable mount-scoped lifecycle counts/phase/renderer state, or null after withdrawal. Proposed values: active/peak games, games created/destroyed, active/peak sockets, sockets opened/closed and opaque mount generation. No session credentials, ticket, raw participant profile, private roster, mutator, reconnect command or configuration setter.
- `frontend/src/world/scenes/MapScene.ts` and `frontend/src/world/runtime.ts`: record game creation at the actual Phaser constructor and release only on actual destroy/closed completion, including delayed boot. Do not infer game count from a mounted canvas or `PHASER_GAME` pointer.
- `frontend/src/world/network/transport/engine.ts` / `frontend/src/world/network/websocket.ts` (subject to the owning transport review): record actual native socket construction/close, including opening/authenticating sockets, not just live replication. The harness already passively observes page sockets; a port would corroborate full ownership/teardown.
- `frontend/src/app/world-entry/production.ts` and `frontend/src/main.tsx`: forward the exact current mount's diagnostics through one explicitly reviewed, opt-in local gate observation adapter. Any browser exposure needs an agreed name/shape/build boundary; none is assumed or read by this patch. Privacy/identity invalidation must synchronously withdraw it. Production credentials and arbitrary control must never be exposed.

After that separate grant, the harness can bind an actual agreed API and assert active/peak one before/through panels/theme/customization/reconnect, same game generation, and zero after teardown. Theme/customization composition must also land through their respective owners before deriving selectors and persistence/render assertions. This proposal grants no ownership or authorization to edit those files.

## Validation performed

All source checks ran in the assigned isolated checkout. Local dependency install: `pnpm install --offline --frozen-lockfile --ignore-scripts`, exit 0; no lockfile changes.

| Command (from frontend unless noted) | Exit | Result |
|---|---:|---|
| `pnpm exec vitest run e2e/real/world-gate/helpers.test.ts` | 0 | 19 offline helper checks; private config/receipt pins, invalid declarations, C05 ordering/revocation, passive overlapping-socket callback accounting. Not live IN01. `/tmp/mcr-in01-helpers.log` |
| `pnpm exec tsc --ignoreConfig --noEmit --strict --target ES2022 --module ESNext --moduleResolution bundler --resolveJsonModule --esModuleInterop --skipLibCheck --jsx react-jsx --types node e2e/real/world.spec.ts e2e/real/world-gate/config.ts e2e/real/world-gate/observer.ts e2e/real/world-gate/decoder.ts e2e/real/world-gate/runner.ts e2e/real/world-gate/helpers.test.ts` | 0 | Targeted harness strict type check; root app build does not include e2e. `/tmp/mcr-in01-types.log` |
| `pnpm exec oxlint e2e/real/world.spec.ts e2e/real/world-gate` | 0 | Targeted lint. `/tmp/mcr-in01-lint.log` |
| `pnpm exec playwright test --config=e2e/real/world-gate/runner.ts --list` | 0 | Exactly one real gate discovered. `/tmp/mcr-in01-discovery.log` |
| `env -u MCR_IN01_CONFIG pnpm exec playwright test --config=e2e/real/world-gate/runner.ts` | **1 expected** | Explicit `IN01 NOT_RUN`, `MCR_IN01_CONFIG_REQUIRED`, no browser/network allocation, no skipped/pass result. `/tmp/mcr-in01-preflight.log` and output JSON |
| Standalone Node 24 decoder check | 0 | Exact app C05 decoder accepts committed protocol fixture and rejects invalid JSON; module runner closed. Offline, not service proof. `/tmp/mcr-in01-decoder.log` |
| Root `python3 scripts/mcr/check-plan.py` | 0 | Planning/source archive integrity only. `/tmp/mcr-in01-plan.log` |
| Root `python3 scripts/mcr/check-boundaries.py --base 8bb878c979138d0f6b3a4b880edb394f41cf99be --lane frontend --allow-doc docs/mcr/qa/in01-world-gate.md` | 0 | Exact lane/document paths only. `/tmp/mcr-in01-boundaries.log` |
| `git diff --check` | 0 | Whitespace check. |

The standalone check imports `loadDecoder` using `node --experimental-strip-types --input-type=module`, reads only `contracts/fixtures/realtime/movement-heartbeat.json`, calls it with the snapshot and invalid JSON, and closes the non-listening runner. Running nested Vite module loading inside a Vitest worker caused that worker to exit; this adapter check was therefore separated into Node, while the 19 pure helper tests remain in Vitest. The real spec's discovery and missing-config path run under Playwright/Node, not Vitest.

Integration requirement reported to delivery: the first Playwright `*.spec.ts` matches Vitest's default discovery. A centrally owned Vitest exclusion for `e2e/real/**` (preserving other default exclusions), or an app unit command scoped to `src`, is needed. This worker does not change central configuration or hide the spec with an environment-dependent skip/pass. Use the explicit targeted commands above until the integrator composes that boundary.

Application build, configured service preflight, actual login, live world, movement/render, customization/theme persistence, reconnect, game counts, visual approval, multiuser and mobile-device execution are **NOT_RUN** for this handoff. Commit/source hashes and the missing-config report are delivered in the local handoff receipt. The writer does not push and does not wait for a main/SL approval gate.
