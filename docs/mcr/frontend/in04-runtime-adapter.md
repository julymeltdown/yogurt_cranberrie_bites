# Real IN04 frontend runtime adapter

Implementation owner: Root FE153. This owns only
`frontend/e2e/runtime-adapters/in04-wall-b/**` and this document/evidence prefix.
The separately qualified QA runner is published at `2b2d4de6`; its helper,
production application, contracts, manifests and active feature leases remain
unchanged. The persistent IN04 delivery objective is ACTIVE.

## Current boundary

The implementation at `76bcbbd8b3ae8e07f2d9232eef1e58261dfc90ae`
adds the in-process QA05/Vite attachment and pins contract JSON imports to the
selected Git candidate. The fresh `ROOT-QA05-COALESCED-ASSEMBLY-SOURCE-20261104T2315Z`
round qualified these selected postimages and generated the separate
`qualified-runtime-restart.mjs`. All seven new assembly cases, five syntax checks
and scoped lint passed. Actual Vite/SSR attachment, managed runtime and live
IN04/QA05 acceptance remain **NOT_RUN**. The first coalesced allocation started
no validation child and expired before work;
its late administrative release is not a successful, deadline-compliant round.
The original accepted controls and explicit NOT_RUN disposition are preserved.
The new artifact embeds backend source `7d74b7fbf397688f3ae41dd7fa5347d3bcc50aa4`
(runtime SHA256
`40fddf11e1520452b73966c8043939cbc35d65ced0e2935515018ab1051cf350`).
Historical artifacts and previous component results remain unchanged.

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

### Prepare source before the live window

`deferred.mjs` prepares the same self-contained adapter without requiring a live
frontend handoff yet. It extracts the genuine backend module from an exact Git
source and verifies its SHA256 before embedding it with the frontend adapter and
owned-file reader. The backend protocol-qualified source is
`2653376e5d8d68f65e05c644e1397a38c6bc02a4`, path
`backend/scripts/mcr/in04_wall_runtime/runtime.mjs`, SHA256
`7d29fee99b5eab4b3e3320ba5a4513f803d6d2b450da33246fa467767deba6ad`.
This establishes source provenance and protocol behavior only. Actual managed
runtime attachment and live acceptance remain **NOT_RUN**.

```text
node deferred.mjs REPO EXACT_BACKEND_SOURCE SHA256 NEW_PRIVATE_ADAPTER.mjs
```

At QA import time this artifact requires `MCR_IN04_FRONTEND_HANDOFF` to identify
the new private handoff written by the actual frontend server. The backend
module independently requires its actual `MCR_IN04_BACKEND_DESCRIPTOR`. Missing,
unsafe or mismatched inputs fail closed. Neither private input is bundled or
committed. Supply the generated artifact's absolute path and SHA256 through the
same QA adapter environment variables described above. Source preparation does
not start Vite, provision accounts, inject faults or restart a service.

The deferred artifact, its two integration source cases and the optimizer
regression passed the fresh `ROOT-IN04-ADAPTER-BUNDLE-SOURCE-20261104T2010Z`
round. `deferred.test.mjs` imports the actual pinned
backend bytes through the QA data-URL mechanism, using a labelled synthetic
frontend observation to check missing managed-runtime refusal. It makes no live
backend claim. The next source round omits the already completed FE41 tail checks.

The paired FE10 owner found that including every manifest dependency in Vite's
browser optimizer incorrectly includes `@tailwindcss/vite` and its native Oxide
binary. This server now excludes both Tailwind packages from browser prebundling
while retaining the actual server plugin and explicit lazy browser dependencies,
including Phaser. Its focused canonical-manifest regression passed alongside
the deferred source cases; actual IN04 Vite startup is still **NOT_RUN**.

The concrete artifact is
`frontend/e2e/runtime-adapters/in04-wall-b/qualified-runtime.mjs` (14,316 bytes),
SHA256 `7b1807be10b7a6a717bb10e66b1395ef12a6c06fa047db8ccb0a53059408283c`.
Set `MCR_IN04_B_RUNTIME_ADAPTER` to its absolute owned-file path and
`MCR_IN04_B_RUNTIME_ADAPTER_SHA256` to that hash, together with the genuine
private frontend handoff, backend descriptor and QA configuration. The artifact
contains pinned implementation bytes only. Its missing-input refusals are source
checks, not evidence of an available deployment.

The new round accepted at 20:10:02.756841 UTC and fully released at
20:10:16.426831 UTC. All 13 commands exited zero: artifact generation, 10 syntax
checks, 11 focused cases and scoped deny-warning lint. The original work/hard
cutoffs were 20:15:02.756835 and 20:16:02.756835; neither was extended. Sampled
raw whole retained bytes peaked at 2,596,754,848 and positive new bytes at 22,422;
the separate nonshrinking carry of 2,604,455,423 plus positive growth charges
2,604,477,845 bytes. Sampled RSS was 35,766,272 bytes and aggregate process count
two. All descendants drained and scratch was absent before the direct successor
handoff. The earlier 1958 offer was declined before acceptance because its fresh
reserved-space margin failed; that record is retained. No workload ran on that
declined offer. Exact source readset, accepted clocks, commands, generated-byte
metadata and release remain under this document's evidence prefix.

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

The original source helper is published at `d0f27b108b1ab8e9d66afdb1faf0b6826dc0dff7`.
The concrete deferred artifact is published at
`f897ef3ada6528bee6e2193f3aae6af5fb660329`. The next implementation step is actual-runtime validation
when the isolated coherent service window/three actors/event/current operator
routes are available. Any resulting frontend correction remains part of this
objective. Existing unrelated owner matrices are reused; they are not replayed
to create a new receipt. No legal/provider/physical-place prerequisite is added
for the virtual Wall.

## QA05 transport attachment source

`qa05-relay.mjs` provides `installQa05WorldRelay` for the same Vite HTTP server,
with Vite's ordinary `/world-ws` proxy omitted by the future actual caller. It
forwards the real upstream upgrade and preserves request Host, Origin, target,
query, authentication and original server WebSocket frame bytes. It creates no
actor, ticket, role, fault HTTP endpoint, replacement backend or second App.
Only a validated canonical C05 snapshot can associate an existing connection
with either of the two expected principals and event. The caller supplies the
actual canonical C05 decoder, qualified opaque-frame fault factory, current
managed-window guard and abort signal. Binary, fragmented, compressed, masked
server frames and messages over 65,536 bytes are refused. Control frames bypass
the fault queue unchanged. The attachment caps live pairs at two, lifetime
connections at four, and socket writable bytes at 262,144; uncertain writes
close the same pair without retry. Evidence reports sink handling, not peer
receipt, membership or convergence.

The `ROOT-FE-QA05-RELAY-SOURCE-20261104T2050Z` round accepted at
20:44:04.788030 UTC and fully released at 20:44:41.368911 UTC. Five new component
cases, two syntax checks and two-file deny-warning lint passed. The controlled
loopback case used two ephemeral HTTP listeners and actual raw sockets to prove
upgrade/header preservation, observed-principal binding, one-delta drop, byte
preservation of the next delta and abort cleanup. Its decoded messages and
backend are explicitly synthetic source fixtures. The existing 19-case fault
factory qualification was reused through exact Git bytes at `74475934a0e53a937b9a0620cf49c9584872979a`;
those cases and the existing IN04/FE41 matrices were not replayed. Attempt one
passed all five cases but failed lint on an unnecessary Set spread. That raw
failure is retained; the cleanup iteration was corrected and qualified within
the original clock and caps. Both live guardians passed, descendants drained,
scratch was removed and both test listeners closed before the direct release.

Actual `server.mjs` wiring, canonical decoder binding, managed checkpoint
composition, ticket-replay probe, World replacement, real principals and shared
browser execution remain **NOT_RUN**. The existing IN04 server and private
seven-field frontend handoff are unchanged. This source attachment is not an
executed QA05 or IN04 acceptance result. Accepted clocks, exact per-attempt
readsets, raw failures, success logs and guardian releases are retained under
`docs/mcr/evidence/in04-runtime-adapter/ROOT-FE-QA05-RELAY-SOURCE-20261104T2050Z/`.

## Private consumed-ticket replay attachment

The relay source above was published at `4f32121dec01fdbf1af21b69d2cc055052fa358f`.
The subsequent private capture/probe implementation passed its own fresh
source-component qualification. Actual managed execution remains **NOT_RUN**.
Existing relay/fault-factory results were reused without relabeling their scope.

`installQa05WorldRelay` optionally accepts `decodeClient` alongside the required
`decodeServer`. The actual caller must bind both to the same source-pinned
canonical C05 decoder, using its client/server directions. A first browser
Authenticate is observed from original masked bytes without changing the
forwarded bytes. A validated upstream snapshot must match its event, room, map,
avatar manifest and self principal before private one-shot release through
`takeConsumedAuthentication(principalId)`. Resync or room-switch snapshots never
repopulate a released authentication. Pair retirement destroys capture custody;
no public endpoint, serialized receipt or debug log exposes the ticket.

`createConsumedTicketProbe` receives the closed noncredential configuration used
by owner checkpoints, that factory's five-field binding, relay, actual owner
control, current window guard/signal, actual World origin and canonical server
decoder. Its returned callback has the existing
`probeConsumedTicket({config,requestId,signal})` interface. It inspects the two
actual connected members, takes one consumed authentication, and makes one raw
upgrade directly to the actual World service with the frontend Host and Origin.
It acquires no new ticket, creates no member and never retries an uncertain
effect. The raw socket has a 2.8-second deadline. Only one Authenticate, no
application snapshot, close code 4401 and actual socket closure can succeed;
4408, application data, negotiated compression, cancellation or changed member
rows refuse the result. A second owner inspection must show the same two
connected member rows. The result is the checkpoint factory's closed count-only
`CONSUMED_TICKET_REPLAY` proof, without raw frames, tickets, user IDs or PIDs.
The surrounding checkpoint facade's timeout does not cancel an owner-control
socket call; Root retains actual handle/guardian cleanup responsibility.

The direct probe is separate from the relay's two active App pairs. The
foreground group permits at most three App relay connections plus this one
direct attempt, four total. Relay limits remain two active pairs and four
lifetime connections. Browser extension offers are preserved verbatim; actual
upstream extension negotiation and compressed server frames are refused.

The new focused source cohort uses explicitly synthetic decoders, authentication
and owner observations with actual controlled loopback HTTP/raw WebSocket I/O.
It checks one-shot custody, original-byte preservation, extension-offer
forwarding without negotiation, 4401/EOF, wrong close codes/application data,
abort, foreign/disconnected members and changed membership. Its result cannot
establish a real consumed App ticket, canonical decoder binding, Vite wiring,
private managed window or any of the seven actual QA05 cases. Those remain
separate runtime requirements. The IN04 seven-field frontend handoff and its
historical qualified artifact are unchanged by this attachment.

The fresh `ROOT-FE-TICKET-PROBE-SOURCE-20261104T2115Z` round accepted at
21:16:35.105171 UTC and fully released at 21:17:01.221581 UTC. All eight new
component cases, four syntax checks and four-file deny-warning lint passed.
Both independent live controls passed; complete retained inventory included the
full Python framework and Node/dependency trees, checkout and actual shared Git
store. Peak retained bytes were 3,606,483,386, positive new bytes at most
2,243,928, aggregate processes four and RSS 141,672,448 bytes. All descendants
drained, controlled HTTP servers closed and scratch was removed before direct
release to the allocator and named successor. The 180/240-second clocks,
4 GiB whole/64 MiB new caps and 6-process/512 MiB RSS caps were unchanged.
Exact source hashes, commands, raw results and both live releases are retained
in `ticket-source-qualification.json` and the round directory. No Vite, browser,
backend, install or dependency link ran.

## Same-server QA05 assembly and restart-aware artifact

`startProductionFrontend` now accepts an optional private `qa05` attachment.
It verifies the same candidate, environment, event, origin and finite window,
loads the seven qualified component bodies through verified data URLs, obtains
the selected application's canonical decoder with Vite SSR, and inspects the
actual owner descriptor before installing the relay on that Vite HTTP server.
Only this mode omits Vite's ordinary World proxy. Its returned private handle
exposes relay/checkpoint/binding/guard/signal/close alongside the existing
frontend origin and optimized Phaser URL. Consumed-ticket capture is explicit
and optional, so the gap/delay/slow packet does not require it. Existing default
startup and the seven-field frontend handoff retain their interfaces.

`pinned-source.mjs` now loads contract JSON from the selected Git tree as well
as application source. Previously, resolved contract imports could fall back to
the physical checkout. The new regression checks both canonical core and World
schemas against exact selected-candidate bytes.

The new self-contained `qualified-runtime-restart.mjs` is 15,580 bytes, SHA256
`40d7c9bb35079b9ff4c93cb8f95fe51bd3680fd3b249f16a14f0f51c6717c3b4`.
It embeds the qualified backend's private baseline-notification protocol; the
separately prearmed owner loop still owns the actual restart. Import requires
the genuine private frontend handoff, and execution requires the current backend
descriptor. The import test uses an explicitly synthetic seven-field frontend
file and performs no observer or service I/O. The historical 7b18 artifact is
retained byte for byte and is not relabeled as restart-aware.

The fresh round accepted at 23:17:46.311310 UTC on November 4 and fully released
at 23:18:01.839548 UTC. All eight commands passed. Both live controls passed;
peak whole inventory was 3,611,795,144 bytes, positive new bytes at most 168,206,
four aggregate processes and RSS 171,196,416 bytes. Descendants drained and
scratch was removed before the direct allocator/FE41 release. No Vite, backend,
socket, browser, listener, install or target ran. The actual SSR/relay join,
production catalog, genuine accounts and managed window remain unexecuted.
Exact hashes, raw logs and both releases are in
`coalesced-assembly-source-qualification.json` and the fresh round directory.
