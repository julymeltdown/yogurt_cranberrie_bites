# FE17 client recovery and QA05 native fault journey

**MOCK_READY: the assigned client/application fault journey is implemented and validated. Whole FE17 and QA05 remain incomplete.** This increment connects the actual canonical Auth controller, application routes, WorldShell, production FE16 native WebSocket transport, FE17 projection, FE18 Rooms/Nearby and default Phaser/selected-character renderer to an explicit local fault peer. It closes the missing application/rendering proof beyond the published headless183 tests. It does not substitute a local peer for the live Rust service.

## Original criteria and disposition

The unchanged specification is `MCR_IMPLEMENTATION_PLAN_V2_EN.md`, §7.5, §12 FE17 and §13 QA05. Its exact FE17 implementation criterion includes “Snapshot baseline, interpolation, bounded prediction/reconciliation, epoch reset and visibility-driven reconnect.” The original acceptance also requires gap resync, restart ghost removal and no old pickup/reservation replay. No task or IA count is amended here.

| Original behavior | Delivered evidence and remaining boundary |
| --- | --- |
| Snapshot baseline and missing-delta resync | Actual renderer and Nearby clear on a delivered-stream gap; one resync; queued old mutations cannot change baseline/roster; a request-correlated complete replacement restores only current members and authoritative self coordinates. |
| Interpolation | Actual Phaser sprites traverse intermediate remote positions and reach the target without extrapolating. The original bounded projection/unit cases remain intact. The world view is visible for this check; Nearby intentionally pauses scene frames. |
| Bounded prediction/reconciliation | Native keyboard input demonstrates delayed ACK bookkeeping, bounded pending inputs (at most64), recovery at the bound and replacement-baseline coordinate correction. **Authoritative spatial prediction/replay remains unavailable**, for the precise public-contract gap below. Local intent prediction is not accepted as authoritative reconciliation. |
| Epoch reset and restart ghost removal | A new epoch/full roster replaces old sprites/DOM; actual peer socket termination closes the shell, clears sprites and requires the real Reconnect button before a fresh ticket/baseline. A Rust process restart and server ghost-lease expiry are NOT_RUN. |
| Visibility-driven reconnect | Genuine trusted native Chrome visibility events withdraw the socket/renderer while backgrounded; foreground obtains one fresh connection. Its first input restarts at1 and all observed resumed inputs are neutral without stale interaction buttons. No `document.hidden` override or fabricated visibility event is used. |
| No old pickup/reservation replay | World input/interaction replay is covered by this native resume plus the published strengthened FE17 regression. Pickup/reservation are durable HTTP/domain operations outside C05. Their real recovery/idempotency acceptance remains NOT_RUN; this harness does not create those commands. |
| QA05 delay/gap/slow client | Native input, delayed ACKs,64-frame client bound, render/input gating and eventual full-baseline convergence pass. The peer holds/reorders delivery deliberately; this is not Rust queue, throughput or capacity qualification. |
| QA05 ticket replay | A deliberately reissued consumed ticket is rejected by the canonical client before any new native socket; a fresh retry succeeds. Actual backend atomic ticket consumption across principals/processes is NOT_RUN. |
| QA05 room switch | Real Rooms buttons exercise rejection preserving current membership, followed by an accepted full-baseline switch on the same socket. Backend transactional membership/capacity races and duplicate leases are NOT_RUN. |
| Single owners, privacy, cleanup | Every sampled recovery retains one game/camera/composition factory and peak one native socket. Actual privacy suspension removes controls/game/socket; disposal empties caches and revokes all observed blob URLs. |

## Exact authoritative movement remainder

Authoritative movement **already exists in published backend code**. This worker read the Git objects at fetched `origin/master` checkpoint `d73296f582a459eb94d55123f8480e78b52a0d21`; sparse checkout does not expose these files on disk. `checks.json` pins each inspected blob and last source commit. World20f supplied the same inventory, and Root1fc confirmed the public-contract gap directly.

* `backend/crates/concert-domain/src/world/mod.rs`, source commit `2d3faaa6e07e0ccce0ad23682cc9aff1b7bd633f`:50ms steps; float64 virtual foot coordinates; normalized intent; authoritative C06 geometry; bounded supercover segment collision including corners/grid edges; blocked movement preserves the exact old point without wall sliding.
* `backend/services/concert-service/src/world/simulation.rs`, source commit `8a716f5546418b60aeee77c7ff5d70292cbc74d1`: receipt timestamps queue intent; fresh consumed inputs advance ACK; held axes repeat until expiry; at most four latest catch-up steps execute while serverTick counts all due steps, including discarded steps. ACK is not a duration or one simulation step per input sequence.
* `backend/services/concert-service/src/config/world_runtime.rs` selects production `Limits::new(160.0,48.0,250)`:160 virtual pixels/second,48-pixel interaction range,250ms input hold. Synthetic FE15 values and backend test tuning are not this public production profile.
* Frozen C05 schema source `69a5624cd153a216f2398cebabf062400dce23eb`, current policy `FROZEN_V1_0`, exposes positions/resource pins/serverTick/ackInputSeq and intent-only input. It does not negotiate a versioned motion profile or publish effective apply/held-duration/dropped-step timing sufficient to replay unacknowledged movement. BE15 is the separate domain outbox/command-receipt task, not a public motion model.

Minimum remaining interface: an explicitly versioned, negotiated motion profile tied to geometry/hash and resource revision, coordinate precision/collision outcome, fixed step/speed/hold/catch-up rules, and ACK/apply timing that distinguishes receipt, consumption, held intervals and discarded ticks. Choosing an additive capability versus a protocol revision and the exact timing representation belongs to the producer/contract owners. Shared language-neutral vectors must cover straight/diagonal/analog movement, stops, coalescing, latency, hold expiry, dropped catch-up, wall corners, bounds and resync.

Root1fc (`01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08`) owns that versioned public contract/capability decision; World20f (`01a1a20f-8d37-71c3-a429-8a55bb6d6c49`) owns producer semantics/emission and vectors; spatial19f83 (`01a19f83-12cc-7f21-8ad3-55749f012d9b`) supplies pinned geometry, not motion authority; FE202 consumes/reconciles after an exact source assignment. Root confirmed a follow-up assignment will follow current startup wiring. No frozen contract, production hunk, backend import or physics authority was acquired by this test-only assignment. The existing `spatialPrediction: 'unavailable'` remains truthful.

## Fixture ports, provenance and limits

Assignment base: `bb7d21b3e931a9dc88056c1c1c436eb9817292da`, isolated branch `codex/mcr-fe-FE17-QA05-client-recovery`. All writes are new under `frontend/e2e/realtime/world/qa05/`, this document and `docs/mcr/evidence/realtime/world-client/`. Former production/existing-test leases remain returned. Shared dependencies, manifests, locks, art, contracts, backend and root composition are unchanged.

`preview.tsx` adapts the existing FE15 application fixture at this base (SHA256 `5788a165ceb0c2b16cc43343209e3d8db552fb9e06420009e61e9a8bc0367db3`); its latest source commit is `977b773deec35226e899bd017d4d946c200fd05f`. It retains explicit fixture Auth/bootstrap/rooms/places HTTP responses, source-only validated resource resolution, application memory routing and genuine source-bound selected artwork. The production resource guard is not admitted or weakened; `productionAdmitted` stays false. No legacy fallback character or new artwork is introduced.

The old MockSocket replacement is removed. The canonical ticket fetch reaches the local HTTP authority, WebCrypto hashes the real opaque fixture ticket and production `WebSocketWorldTransport` creates the native same-origin socket. The local server receives a validated source-bound initial snapshot; only server frames/delivery/connection faults change state. The observer reads existing public projection/camera/privacy ports and actual Phaser scene sprites. It never injects client projection/render state or replaces the renderer.

`peer.mjs` adapts native framing from published `replication-browser.mjs` (`79e45560b9fff757f4e4caf541cf5308d4856587`, source SHA256 `5176e98a8ff930e2e8344bed479af91559f0bd0a6874d0dab38fb47417e4d255`). Native masked client frames, exact message fields, fresh streams, request correlation, full rosters, unchanged connection access expiry and opaque one-use test tickets preserve the frozen client protocol. ACK timing, stationary self positions, absolute remote interpolation vectors and deliberate delivery bursts are controlled fixtures, not the BE12 simulator or server cadence/load proof. Credentials are excluded from recorded wire traces. The60-second fixture access expiry is not the server's separate5-second authorization recheck lease.

`browser.mjs` uses a new temporary Chrome profile and public Playwright `connectOverCDP(...,{noDefaults:true})` on its existing default context. Ordinary Playwright-created contexts force focus/visibility emulation, which prevented genuine backgrounding in the initial driver. The helper owns and removes only its temporary profile/process. `check-types.mjs` generates temporary compiler configuration outside the repository using retained frontend options; no central tsconfig is added. No dependency is installed.

Current selected source pack, source modules, C05 fixtures/schema/policy, harness files, resource/theme qualifications and captures are hashed in `native.json`/`checks.json`. The run pins971 TS/TSX/CSS/JSON and harness inputs. This source-only fixture keeps prior map/theme/art provenance limits; it cannot admit product art, actual backend behavior, two-account IN01, QA08 load measurements or IN05. Those statuses remain NOT_RUN where this delivery supplies no evidence.

## Validation and handoff

Use retained Node24.22.2, frontend dependencies, installed Chrome and Playwright1.61.1. From repository root:

```sh
node frontend/e2e/realtime/world/qa05/native.mjs
node frontend/e2e/realtime/world/qa05/check-types.mjs
python3 scripts/mcr/check-plan.py
node scripts/mcr/check-ui-origin.mjs
```

The native runner writes `docs/mcr/evidence/realtime/world-client/native.json`, with captures under the owned frontend prefix. `MCR_QA05_WIDTH=390` selects one diagnostic width; the default executes360/390/430. It uses a temporary Vite cache, an8GiB disk floor and closes its browser/socket/server in `finally`. From `frontend/`, run the exact six-file Vitest command in `checks.json`, `node_modules/.bin/tsc -b --pretty false`, and `node_modules/.bin/oxlint e2e/realtime/world/qa05`.

| Check | Result |
| --- | --- |
| Native Chrome156.0.8078.80 application journey | Exit0; all3 widths pass;42 recorded behavior observations plus3 privacy/disposal receipts. |
| Existing replication/transport/composition/presentation/Rooms UI and controller regressions | Exit0;242 tests in6 files. |
| Fixture TypeScript and full frontend TypeScript | Both exit0. |
| Scoped lint, UI source-origin and planning/archive checks | Exit0; no origin violations;13 original archives unchanged. |
| Exact assigned-path/import boundary and whitespace | Recorded in `checks.json` after final receipt staging. |

Driver corrections were confined to this new prefix: ordered delayed ACK delivery, actual visibility without Playwright focus override, non-vacuous resumed-input assertions, and interpolation on the visible scene rather than the intentionally paused Nearby panel. No production defect was established or fixed. The prior broad historical NOT_GREEN result remains unchanged; no unrelated full-suite/release claim is made.

Root153 (`01a1a153-1227-7e72-801f-a2a6b91981e1`) is the designated frontend publisher, using its isolated delivery checkout. This worker supplies a committed, source-pinned handoff and addresses concrete fixes; it does not push master or ask SL/main for review approval. The integrator's publication reply and independently verified remote ancestry are required before this bounded delivery goal can close. Exact live input needs are gateway/origin/cookie capability and C05/resource pins, event/room IDs, two independent qualified Auth principal references in private0600 files, and safe server fault controls. No credential values belong in chat or evidence. World20f/Root1fc remain the live producer/startup owners; no IN01 test path is implicitly allocated here.


## Verified publication

Root153 actually published this bounded packet at master `980fc4d85020f297b36fee8195e1ca92079ff33f`. Worker implementation `47db7bf556f9bb85b33186ad832301575e69740a` maps to `3c2c1b17040d3ceb0efe37a5e31a3d94beb0200a`; receipt correction `fa33781d766392f4aba4c74baf6dc0ce20d83aff` maps to `d398f556`. Independent remote/Git checks confirm both mapped ancestors, exact binary patches and all24 original file payloads byte for byte.

The integrator reran the full native journey against the integrated `3c2c1b17` candidate because shared scene inputs changed: all3 widths,42 observations and3 cleanup receipts pass. This worker inspected that actual report (SHA256 `6d40bf3a4db40f09decf71da7f02126bba62997434daebf0aa9ca0ddbd4bccc8`) and independently matched all991 input hashes to published `980fc4d8` Git bytes. `publication.json` records compact observations, trusted visibility, capture checksums, source deltas and provenance. Full integration report/captures are retained by the integrator locally; the original971-pin report and captures remain published unchanged. The integrator additionally reports274 combined tests and fixture/full TypeScript/lint passing, with temporary resources closed.

This publication and its compact receipt do not accept the remaining authoritative FE17 prediction criterion, actual live QA05, IN01 or IN05. No new IN01 goal or source lease follows implicitly.
