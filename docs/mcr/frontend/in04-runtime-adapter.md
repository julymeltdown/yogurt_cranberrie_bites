# Real IN04 frontend runtime adapter

Implementation owner: Root FE153. This owns only
`frontend/e2e/runtime-adapters/in04-wall-b/**` and this document/evidence prefix.
The separately qualified QA runner is published at `2b2d4de6`; its helper,
production application, contracts, manifests and active feature leases remain
unchanged. The persistent IN04 delivery objective is ACTIVE.

## Current boundary

Source is qualified at `6d5863097c46d7ecd3d1633fd1423500e17b8867`: all eight
focused source cases, all six module syntax checks and deny-warning scoped lint
passed in the actual allocated round. Actual Vite launch, real HTTP,
two-member/operator flow, Phaser raster,
moderation and external restart acceptance are **NOT_RUN**. No configuration
assertion, controlled fixture or old service window establishes those results.
The BE Root owner supplies actual deployment qualification and restart observation
from its managed runtime; Operations205 and Wall202 supply their actual routes.

## One canonical frontend

`server.mjs` serves the exact published Git candidate's `frontend/index.html`,
`src/main.tsx`, application imports and public assets. `pinned-source.mjs` adapts
Profile213's qualified `frozen-source.mjs` from publication `28701b24`, retaining
read-only Git extraction and SHA256 receipts, adding canonical ancestry and
fail-closed absent imports. A stale physical source tree cannot silently supply a
missing application module. PostCSS nested relative imports and physical source
assets must exactly equal the candidate; otherwise startup/loading fails and the
implementation owner resolves that concrete discrepancy.

The server uses the incumbent dependency tree with exact package/lock equality,
explicit package aliases and an allocated cache. It does not install dependencies,
create another checkout, replace fetch/WebSocket, publish browser globals, fake
World admission or launch a backend stack. `/api` and `/world-ws` proxy to the
actual supplied loopback services with `changeOrigin: false`. Canonical Auth,
enrollment, C03/C05 and production composition still determine access.

The required private owned mode0600 settings JSON has exactly these fields:

```text
version: 1
candidate: exact published 40-character source commit
environmentId, eventId: actual runtime UUIDs
origin: exact http loopback frontend origin with explicit port
gatewayOrigin, worldOrigin: actual incumbent http loopback service origins
localInput: qualified production LocalInputConfiguration
mural: actual-map-bound production MuralProductionConfiguration (128 by 128)
```

Both configuration validators are loaded from the exact candidate source through
Node's built-in TypeScript stripping. Placement identifies virtual product art;
it does not grant a role or prove a physical place. If the actual C05 map differs,
its real placement must be supplied and validated. No guessed movement policy,
accounts, event, seed snapshot or permission is provided as a default.

Inside a fresh allocated and independently guarded runtime window:

```text
node server.mjs REPO CANDIDATE PRIVATE_SETTINGS EXISTING_NODE_MODULES ALLOCATED_CACHE NEW_PRIVATE_HANDOFF
```

The single frontend starts on its supplied exact port, optimizes the actual same
Phaser dependency and extracts its exact versioned import URL from Vite's
transformation of the canonical MapScene. Explicit dependency discovery keeps
that hash stable across lazy routes. The private allocated cache has an actual
same-origin `/@fs/.../deps/phaser.js?v=...` URL; an unversioned guess could create
a second Phaser instance. A new private handoff is written only after this exact
module is available. The nonce-protected loopback observation endpoint returns this running
server's candidate, settings hash, event/environment and placement with a short
expiry. It reports frontend binding only, never backend readiness or actor roles.
Signals close the frontend; the runtime owner retains normal process/cache/file
custody and must report actual release, rather than infer it from a saved file.

## QA handoff

BE Root provides a self-contained `.mjs` exporting `runtimeQualification` with
genuine `inspect(config)` and `observeRestart(config, before)` observations. Relative
imports are unsupported because QA loads verified bytes through a data URL.
Node built-ins and embedded verified modules work. The backend implementation must
not initiate a restart, rotate Auth on each inspection or synthesize ready flags.

`prepare.mjs` verifies that module's exact SHA256/owned-file permissions, reads the
live frontend private handoff, and emits one new mode0600 self-contained adapter:

```text
node prepare.mjs REAL_BACKEND_MODULE SHA256 PRIVATE_FRONTEND_HANDOFF NEW_PRIVATE_ADAPTER.mjs
```

Its sanitized output supplies the final adapter SHA256. QA uses
`MCR_IN04_B_RUNTIME_ADAPTER` and `MCR_IN04_B_RUNTIME_ADAPTER_SHA256` together with its
separate private `MCR_IN04_CONFIG`; no real credentials enter Git or logs. The
composed adapter checks the current frontend observation before passing genuine
backend proof through unchanged. It never turns GateConfig fields into proof.
`muralBinding` returns the observed production placement and same-app optimized
versioned optimized module URL. QA's observer needs the corresponding narrow
same-origin allocated-cache URL allowance; the original fixed conventional-cache
pathname cannot represent a private cache. QA compares the actual
C05 map and actual one-game/one-canvas/one-image texture, anchor/depth and all
65,536 RGBA bytes against the real snapshot. It does not use a simulated texture.

## Focused checks and remaining work

`source.test.mjs` covers source pinning/no physical fallback, exact loopback
targets, private-file/symlink/hash/size rejection, reuse of canonical validators,
unchanged proof delegation, missing/stale/mismatched frontend rejection, backend
source/capability refusal and self-contained QA-style loading. Its tiny HTTP and
backend modules are explicitly synthetic **source fixtures**, not deployment
acceptance. `validate.py` starts as the independent source guardian only after
an actual fresh accepted scheduling offer. It counts whole checkout, shared Git,
dependencies, Node and scratch, retains failures, and never starts Vite or a browser.

The actual `ROOT-IN04-FE41-COALESCED-SOURCE-20261104T1927Z` round was accepted at
19:26:47.066479 UTC and fully released at 19:27:12.412989 UTC. Its original work
and hard cutoffs were 19:31:47 and 19:32:47, without extension. Peak whole retained
bytes were 2,597,872,125; positive new bytes 453,327; RSS 88,817,664; and aggregate
process count two. All observed descendants drained and scratch was removed.
No Vite, browser, backend, build, install, dependency link or runtime cache was
started. The same round performed only FE41's two requested final harness
lint/syntax checks on SHA256 `c8ee10c427f023a9a7622dd6320729b6ff5488d6a97c1df52d7ed3e3f8878122`,
both exit zero with the input hash unchanged. Its existing source/native results
were reused. Exact commands, source hashes, guardian and release are retained in
`docs/mcr/evidence/in04-runtime-adapter/`.

The next implementation steps are exact publication,
composition with BE Root's genuine module, and relevant actual-runtime validation
when the isolated coherent service window/three actors/event/current operator
routes are available. Any resulting frontend correction remains part of this
objective. Existing unrelated owner matrices are reused; they are not replayed
to create a new receipt. No legal/provider/physical-place prerequisite is added
for the virtual Wall.
