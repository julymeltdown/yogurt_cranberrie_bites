# FE14/FE16/FE17 world lifecycle composition

Status: **MOCK_READY**, pending integrator review and production world-entry wiring.
This is a concrete lifecycle adapter with actual Phaser/native WebSocket fixture
evidence. It does not accept IN01, FE12 artwork, real backend physics, device/load
behavior, or the full FE14/FE16/FE17 product scope.

## Delivery and provenance

Worker: `codex/mcr-fe-world-composition`, isolated checkout
`/Users/lhs/Desktop/mcr-worktrees/fe-world-composition`. Original assignment base:
`73d9b490a32ec7f5b92953b758d21e6004786a8f`.

The integrator explicitly authorized these prerequisite cherry-picks:

| Original integrator commit | Local prerequisite commit | Purpose |
| --- | --- | --- |
| `9919b794f78940b39a858121762623035b83471b` | `f4e376d54e46feb4ecb55543139d046a48bde2f4` | Forward the exact captured bootstrap privacy lease through WorldHost/mount context. |
| `b3ffa137b5b13408477ea10d5517e6c9671b8f56` | `df6e7772c1c15b53d5410bbbea66644d76b4fa98` | Retain the immutable validated C03 body as `ShellBootstrap.document`. |

The worker patch is based on `df6e7772c1c15b53d5410bbbea66644d76b4fa98`.
Integrate only the worker delivery commit, since the integrator already owns the
original prerequisite commits. Worker-owned paths are exclusively
`frontend/src/world/composition/**` and this document. The commit parent records
the prerequisite ancestry; the handoff supplies the full delivery commit.

| Source at original assignment base | Reuse / destination |
| --- | --- |
| `frontend/src/world/runtime.ts`, `map/`, `scenes/MapScene.ts` | Imported FE14 runtime/resource validation and actual Phaser factory; no copied rendering implementation. |
| `frontend/src/world/network/{websocket,replication}.ts`, `network/transport/`, `entities/replicated.ts` | Imported FE16 socket/FE17 authoritative replication and immutable projection. |
| `frontend/src/world/input/`, `camera/port.ts` | Imported FE15 controls/pathfinding; a composition camera proxy suppresses predicted camera positions. |
| `frontend/src/features/auth/model/{session,bootstrap,contract}.ts`, `shared/query/privacy.ts`, `shared/api/http.ts` | Canonical Auth, retained C03 validation, exact privacy lease, and HTTP policy. |
| `frontend/src/world/network/__tests__/replication-browser.mjs` (itself adapted from FE16 `browser-smoke.mjs`) | Minimal native WS fixture framing adapted into composition `__tests__/native.mjs`; test-only server logic. |
| `frontend/src/world/__tests__/fixtures/movement-data.ts`, `contracts/fixtures/{core,realtime,assets}/` | Imported synthetic terrain and contract metadata into composition test fixtures only. No product artwork. |

No dependency, manifest, lockfile, contract, backend, root router, or retained
source archive changes are part of the worker patch. Existing retained packages
are sufficient: Node 24.22.2, pnpm 11.7.0, Phaser 3.90.0, Vitest 4.1.9,
Playwright, Vite, TypeScript and Oxlint. The isolated install used the frozen
retained lockfile offline, exit 0.

## Public integration surface

`createWorldComposition({ auth, resolveRelease, renderer? })` returns a
WorldLifecyclePort-compatible `mount`, lifecycle state `read/subscribe`,
`dispose/settled`, and a stable `.replication` release:

```ts
interface WorldReplicationRelease {
  read(): ReplicationView | null
  subscribe(listener: () => void): () => void
  switchRoom(roomId: string): Promise<boolean>
  reconnect(): Promise<boolean>
}
```

`null` synchronously withdraws previous membership/presence. FE18 can retain this
release across shell lifecycle changes. A fresh active owner is required for a
non-null view. Room switching requires a room in the same-lease validated C05
room-list snapshot. C05 tickets and the server make the final admission decision;
an ordinary full/unavailable target rejection retains the current membership.
A room added after that snapshot needs a newly resolved release rather than an
unreviewed caller-selected target.

The integrator's `resolveRelease` receives the captured scope, exact privacy
lease, immutable `ShellBootstrap`, and abort signal. It supplies:

- That same lease and **exact retained `bootstrap.document`**, without a second
  bootstrap request or restamping old data with a fresh lease.
- An explicit reviewed `themeId`, `roomId`, local intent `movementPolicy`, and
  zoom. There are no default product IDs, URLs, policy values or asset releases.
- C05 `WorldRooms` read under that lease, plus the reviewed `AssetResources`
  path/hash allowlist. The adapter wraps resolution in `auth.privacy.run` and
  validates the C03/C05/C06 bindings before loading resources.

For example, central world-entry wiring constructs the resolver with its
reviewed selection and resource dependencies, injects `auth`, assigns the result
to `ports.world`, and passes `.replication` to FE18. This worker does not own that
registration. The optional `importEngine` is FE14's existing testing seam;
production omits it and uses the actual dynamic Phaser factory.

The optional `WorldRendererPort` receives the loaded map, real camera, prepared
release (including validated avatar metadata), abort signal and `sample()`.
Its existing frame loop may sample the FE17 interpolation view. Composition
adds no rendering RAF and creates no fallback avatar or second Phaser instance.
Renderer `replace` receives authoritative projections; its `dispose` must clear
its visual ownership. Without FE12's reviewed renderer, state explicitly reports
`renderer: 'unavailable'`, displays that limitation, and renders no characters.

## Authority and resource behavior

- One active owner is claimed per canonical SessionController, including across
  adapter instances. Re-entry waits for the old FE14 engine's `closed` promise
  before allocating another game. A second live mount is rejected.
- Mount rejects absent/stale/suspended privacy authority, incompatible scope,
  aborted context, unavailable world capability, and disallowed event phase.
  C03 document equality and manifest/hash/schema checks precede resource use.
- C05 ticket POST uses canonical `auth.request` with core errors, protected
  access, browser intent and `cache: no-store`. It adds no Auth store, refresh
  layer, idempotency key, credential persistence or automatic POST replay.
  Response event/room/map/avatar/session bindings are checked. Qualified Auth
  GET refresh behavior cannot silently refresh/replay this mutation.
- Abort, logout, policy revocation and terminal C05 authority/sink failure
  synchronously release the game, socket, roster, renderer, input listeners and
  owned DOM. Late resolver/resource/renderer results cannot restore ownership.
  Renderer disposal is attempted even if its projection sink throws.
- Hidden documents or a hidden world panel withdraw socket membership and
  entities while retaining the game. Reveal obtains a fresh ticket. Ordinary
  disconnection exposes an explicit **Reconnect world** button; there is no
  automatic reconnect loop.
- Resource/map-version/runtime failure releases partial owners and exposes
  **Try opening world again**. Retry uses the same still-current captured lease,
  a new abort controller and a generation fence. Lifetime revocation listeners
  stay active while unavailable. Visibility/input-block settings received while
  unavailable survive retry. A revoked lease cannot retry.
- FE15 controls generate C05 intents through the existing replication owner.
  FE15 local pathfinding does not establish server physics: its predicted foot
  cannot move the real camera or entities. The real camera centers only on the
  C05 self position; remote interpolation remains FE17's bounded server-sample
  behavior. The fixture intentionally keeps server positions scripted. Virtual
  movement never proves physical arrival.

No product character artwork, browser storage, backend implementation imports,
decorative left accents, additional Auth identity, or additional world socket
owner is introduced.

## Verification and selected evidence

Commands ran from the isolated frontend directory unless noted. The pinned
runtime was selected as follows; shell variables below are documentation
convenience variables, not additional dependency resolutions:

```sh
export PATH="/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin:$PATH"
MCR_NODE=/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node
MCR_PNPM=/Users/lhs/Desktop/mcr-tools/pnpm-11.7.0/package/bin/pnpm.cjs
"$MCR_NODE" "$MCR_PNPM" exec vitest run src/world/composition/__tests__/composition.test.ts
"$MCR_NODE" "$MCR_PNPM" exec tsc -b --pretty false
"$MCR_NODE" "$MCR_PNPM" exec oxlint src/world/composition
MCR_WORLD_COMPOSITION_EVIDENCE=/Users/lhs/Desktop/mcr-worktrees/fe-world-composition/frontend/src/world/composition/__tests__/evidence/native-final-02 \
  "$MCR_NODE" /Users/lhs/Desktop/mcr-worktrees/fe-world-composition/frontend/src/world/composition/__tests__/native.mjs
```

| Check | Result / limits |
| --- | --- |
| Focused composition suite | Exit 0, 25/25. Covers canonical ticket policy, rejected/stale authority, synchronous teardown, late completions, renderer failures, duplicate/re-entry fences, hide/resume, room admission, authoritative camera and explicit resource retry. Unit fixture substitutes the engine, PNG decoder and socket; it is not native evidence. |
| TypeScript | Exit 0. |
| Scoped Oxlint | Exit 0. |
| Native composition fixture | Exit 0, actual Phaser 3.90.0, Chrome 156.0.8078.80, 390×844 and 1280×900, canonical Auth HTTP and actual native WebSocket. TEST ONLY C03/C05/C06 HTTP/WS/terrain fixture. |
| Planning checker, repository root | `python3 scripts/mcr/check-plan.py`, exit 0. Planning provenance only; no application acceptance implied. |
| Boundary checker, repository root | `python3 scripts/mcr/check-boundaries.py --base df6e7772c1c15b53d5410bbbea66644d76b4fa98 --lane frontend --allow-doc docs/mcr/frontend/world-composition.md`, exit 0, 14 paths; exact assignment paths and imports also reviewed manually. |

Selected native artifacts are under
`frontend/src/world/composition/__tests__/evidence/native-final-02/`:
`report.json`, `world-390.png`, `world-1280.png`, `reentry-390.png`,
`reentry-1280.png`. The report records 280 SHA-256 source/contract inputs, the
prerequisite HEAD plus working-tree source bytes, browser/runtime versions,
sanitized request metadata, active/final resource counts and errors. Every
reported hash must match delivery source bytes; the worker commit's ancestry
binds those bytes without claiming that its own future SHA existed at run time.
Superseded preliminary run artifacts are not selected for delivery.

The native fixture verifies real FE15 keyboard intent delivery/acknowledgement,
one game/socket across shell panels, hide/resume with fresh tickets, full target
ticket rejection preserving membership, gap resync evicting ghosts, explicit UI
reconnect, room switching, synchronous privacy withdrawal, StrictMode
logout/login re-entry with peak one game and authorized socket, and final
canvas/game/socket withdrawal with balanced mount/disposal counts.

Screenshots show synthetic terrain and unavailable avatar art. They are not
product artwork or final responsive-layout approval; the existing controls and
notices consume substantial space at 390px. No physical device, live backend,
database, load, production build, release or complete multiuser IN01 flow was
run by this worker. Full IN01 remains blocked on reviewed FE12 rendering and
real integrated backend/physics/multiuser evidence. The production resolver,
WorldAccessPort bridge and root wiring are separate integrator/assigned-entry
work. No task or IA scope was cut.
