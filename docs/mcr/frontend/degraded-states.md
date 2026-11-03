# FE41-STATES — degraded-state presentation and observer

## Delivery boundary

**MOCK_READY** for the assigned reusable implementation and deterministic browser
composition. FE41 as a global production flow is **not ACCEPTED**: default root
insertion, validated service-decision producers and real service/device checks
remain open. No task or IA scope is removed.

Base: `6c018d622e805290a5fdc84c29004e65587f688c` on
`codex/mcr-fe-FE41-states`, in the isolated `fe-degraded-states` worktree. The
delivery commit is the direct child of this base; resolve its full SHA from the
committed handoff/Git history. Only new files under
`frontend/src/features/connection-status/`, `frontend/src/app/errors/`, and this
document are delivered. Existing application, contract, manifest, lockfile,
backend, art and source-document bytes remain unchanged. Main/Delivery owns
integration and publishing.

## Implemented behavior

- `features/connection-status/model.ts` and `index.tsx`: fixed offline,
  reconnecting, resync, paused, unavailable, maintenance and version copy; stale
  notice; explicit reconnect/reload; real world/home navigation. Ready state
  adds no redundant panel. Shared concert-window composition supplies black
  title bars, ivory surfaces and restrained red actions. Borders are symmetric;
  no decorative left rail, replacement art or character fallback is introduced.
- `app/errors/RouteState.tsx`: safe 404, unavailable, maintenance and version
  regions. The owner selects a typed category; thrown errors, remote text, URL
  parameters, stack traces and credentials cannot become component text.
- `app/errors/observer.ts`: observes the existing `WorldAccessBindingPort` and
  its current `ReplicationReader`. Captured release identity, abort signal,
  canonical session/privacy, event and world capability fence every read and
  callback. One replication subscription follows the active release. Policy,
  user, event and release loss synchronously clear stale/protected projection.
  Old notifications and late retry completions cannot cross releases.
- Pending display has one deadline across ticket/open/authentication/resync
  phase churn: 8 seconds by default, configurable within 100–30,000 ms for the
  owning composition/tests. Expiry becomes an actionable unavailable view,
  never another automatic attempt. An explicit retry delegates only to the
  captured existing FE17 owner. An opening/resync timeout offers reload when
  reconnect would be meaningless in the current FE17 phase. A hung delegate
  does not leave the UI pending indefinitely; disposal still fences its result.
- `app/errors/connectivity.ts`: optional root-owned browser online/offline hint
  adapter, installed once and fully disposed. An online hint does not establish
  server reachability. It has no visibility listener or recovery operation.
- Stale is sticky after disruption. A live-phase notification, reconnect
  promise, delta sequence advance, or returning online cannot clear it. A new
  authoritative snapshot identity (stream/epoch/room/request/cause) must be
  visible and live. No roster, coordinates, private response or bearer is
  copied into the presentation state. A valid original room remains live during
  room switching and pre-send rejection; room selection remains with FE18.
- A map-version problem from the actual FE17 reader is reload-only, even when
  the browser is offline. Authority/sink failures cannot offer reconnect.
  Maintenance and application-version states require an explicit validated
  producer decision for the exact current release. Neither navigator state nor
  a transient request error is promoted into an operational decision.

The observer is not an authorization or input owner. It does not block actions
on behalf of the server, queue commands, fetch, create Auth, open a socket/game,
run a retry/resync loop, or add rendering/visibility timers. WorldHost continues
to own document visibility and coverage; FE17 owns input suspension, bounded
resync, explicit reconnect and foreground recovery; FE14 runtime owns renderer
visibility. Returning online alone deliberately does not initiate a mutation.

## Exact reserved root and producer seams

These are owner handoffs, not claims of default reachability:

| Owner/path | Required insertion |
|---|---|
| `frontend/src/main.tsx`, root lifecycle owner | Construct one `createBrowserConnectivity(window)` and one observer alongside the existing canonical Auth/ports, start each once, and dispose both before ports/Auth teardown. Inject `() => location.reload()`; no URL-bearing reload input is needed. |
| `frontend/src/app/WorldShell.tsx` | Render one `ConnectionStatus` inside the existing Router context in a root-owned accessible status region while preserving panel/world layout and existing WorldHost. Do not mount a second WorldHost, add a visibility listener, or remount the controller on panel navigation. |
| `frontend/src/app/router.tsx` | Select `RouteState` for the reviewed catchall/error routes; retain safe navigation. New 404 is presently exercised through the existing `createAppRoutes(..., {panels})` composition seam only. |
| `frontend/src/app/ports.ts`, `frontend/src/app/world-entry/production.ts` | Expose a narrow release-scoped `RecoveryAction` producer delegating to the existing composition's `replication.reconnect`. `world/composition/index.ts` already exposes this action; the production `ReplicationReader` wrapper currently drops it. Do not use `switchRoom`, recreate composition, or instantiate a connection to simulate reconnect. |
| Validated operational producer + root policy owner | Supply a `ValuePort<ServiceDecision|null>` whose `release` is the exact captured world release. Only select maintenance/version after existing canonical decoding/validation and trusted operational policy. Withdraw obsolete decisions on lifetime change. This local view projection is not a new wire schema. No operational maintenance/application-version producer is exposed by the current public world binding. Explicit version/maintenance decisions must also stop prohibited actions through their existing owners; the notice alone cannot enforce read-only mode. |
| Production world/runtime owner | A null public replication view cannot distinguish slow configuration/import/load from a failed runtime. Until a safe explicit unavailable projection is supplied, the observer waits once and then offers reload. Do not infer maintenance from this condition or reach into the private runtime. |

The existing `WorldHost` already presents its own unavailable/mount-failure
message and remount retry. The integrator must reconcile that placement with
the new global region without duplicating retry controls. Missing operational
producer semantics should be resolved by the responsible owner, not fabricated
in this feature. Removing a maintenance/version decision does not certify fresh
world data: the existing producer/recovery owner must release a fresh baseline.

Suggested root ownership sequence: create observer once with canonical `auth`,
`world: ports.worldAccess`, optional scoped `service`/`recovery`, and the network
hint; call `start()`; pass that same controller to the rendered status region;
on teardown call observer `dispose()`, network hint `dispose()`, then dispose
existing ports/Auth. Root code must handle an absent worldAccess port explicitly
as unavailable rather than casting an absent producer into a valid one.

## Validation

Pinned Node: `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node`
(`v24.22.2`). Reused retained dependencies temporarily through a worktree-local
symlink to `/Users/lhs/Desktop/mcr/frontend/node_modules`; no installation,
manifest/lock edit, shared cache or incremental buildinfo write. Symlink is
removed before commit.

Commands run from the author checkout (Node directory prepended to PATH):

| Command | Exit / result |
|---|---|
| `python3 scripts/mcr/check-plan.py` | 0; planning baseline only |
| `node frontend/node_modules/vitest/vitest.mjs run --root frontend --configLoader runner --no-cache src/app/errors/__tests__/observer.test.ts src/app/errors/__tests__/connectivity.test.ts src/features/connection-status/__tests__/status.test.tsx src/features/connection-status/__tests__/pickup-no-replay.test.ts src/features/gift-pickup/__tests__/pickup.test.tsx src/world/network/__tests__/replication.test.ts src/world/composition/__tests__/composition.test.ts` | 0; 103 tests in 7 files |
| `node frontend/node_modules/typescript/bin/tsc --noEmit --incremental false --project frontend/tsconfig.json` | 0; full frontend strict types |
| `node node_modules/oxlint/bin/oxlint src vite.config.ts` (from frontend) | 0; no diagnostics |
| `node frontend/src/app/errors/__tests__/native.mjs` | 0; Vite production build of existing app plus fixture into `/tmp/mcr-degraded-build`, then native Chrome 156.0.8078.80 at 360/390/430; 42 captures, zero browser errors |

Focused tests cover offline versus server disconnection, deadline phase churn,
missing producer, explicit retry failure/hang, no automatic recovery, stale
live-before-snapshot/delta behavior, fresh authority, scoped operational input,
map mismatch, suspended/foreground behavior, release changes, abort, policy,
identity/event changes, late results, one subscription and disposal. Actual
React external-store presentation, route states and browser hint cleanup are
also exercised.

The new no-replay regression imports the **actual unchanged gift-pickup
controller and validated contract fixture**. An owner claim loses transport
while offline, remains uncertain with no success receipt, and retains only safe
intent metadata. Online notification, subsequent load/claim and reconstruction
from the journal never send a second POST. This demonstrates no silent claim
replay, not a general guarantee that unknown server outcomes succeeded.

The native fixture uses canonical Auth, existing `createAppRoutes`, WorldShell,
WorldHost, actual FE14 loader/Phaser runtime and FE17 replication with the existing
MockWorldTransport. Synthetic terrain is explicitly marked test-only; it is not
approved map/character product art. Assertions verify one Phaser game, one active
mock socket, one observer subscription, one resync request, bounded wait, stale
reset only after snapshot, keyboard Enter reconnect, actual document reload,
world/home links and complete teardown. WorldHost alone receives synthetic
`visibilitychange`: hidden pauses rendering and disconnects, foreground opens
one replacement socket without another game or observer. Repeated same-value
visibility events do not duplicate ownership. This is not real OS backgrounding.

The 360px half panel legitimately scrolls. Additional `*-actions-*` captures and
viewport assertions show the fallback controls remain reachable above the fixed
bottom navigation. All feature actions are at least 44×44 CSS px; no horizontal
document overflow occurs. Manual inspection included offline/action captures at
360, maintenance at 390 and map-version at 430. Existing outer shell chrome and
synthetic terrain remain visible and are not described as full visual acceptance.

Evidence: `frontend/src/features/connection-status/__tests__/evidence/` contains
raw command logs, `native.json`, captures and `validation.json`. The latter pins
new source hashes, existing source/contract/reference hashes and evidence hashes.
Staged whitespace verification excludes raw `*.log` files to preserve their
terminal output, including Vitest's trailing blank line; all authored files pass.
The exact ten-screen reference was viewed; its unchanged SHA256 is
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
Source reuse is limited to current MCR public APIs/shared components; no legacy
Bigyeting rendering or visual package was copied. No new dependencies.

Limitations: default production route/global insertion and missing operational
producers are **NOT_RUN**; real backend/multiuser/reconnect/pickup, physical
devices, actual network outages and OS backgrounding, load and release gates
are **NOT_RUN**. Full frontend suite was intentionally not run for this bounded
assignment. Vite retains its existing large-chunk warning; the production build
exits 0. No server or privacy invariant is certified by screenshots or mocks.
