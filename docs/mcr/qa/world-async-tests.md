# QA-WORLD-ASYNC: bounded world test readiness

Handoff: **READY for Main review**. Evidence status: **MOCK_READY** for the scoped test corrections. No FE16/FE18/QA09 acceptance or release status is promoted by this handoff.

Base and parent: `dda0439dae9dbcb9186eb2f656b5c22f2df5fb6e` on `codex/mcr-qa-world-async`; worker checkout `qa-world-async`. Delivery repository is `julymeltdown/mcr`, `master`; only Main publishes. The delivery commit is the commit adding this report and the three test paths below; it is discoverable with `git log -1 --format=%H -- docs/mcr/qa/world-async-tests.md`. No copied application code or dependency change.

## Original evidence and diagnosis

UIV04 evidence at `frontend/src/features/me/__tests__/visual-adoption/evidence/validation.json` binds its original full run to `11d7cb00165f78b2ec9c564dd93669edad104e88`. That historical run reported 96 files, 2204 passed, five failed, and 14 skipped. The transport epoch recovery `socket.open` undefined and room confirmation `requestId` undefined passed its targeted comparison, which correctly did not claim a fix. Original evidence and `docs/source/` remain byte-for-byte unchanged.

Source inspection of `transport/engine.ts` shows ticket acquisition and `crypto.subtle.digest` complete before factory socket creation and the public `opening` state. The transport test helper previously returned after twelve microtasks and 5 ms regardless of socket existence, so a held ticket returned a ready-looking result containing no socket. Separately, `roomSwitch` publishes `switching` before awaiting ticket validation and sending its frame. The room UI test previously treated the visible Switching label as permission to invoke `reject()`, whose synchronous fixture lookup could have no outbound request.

These are test readiness assumptions. No production defect was established by the controlled probes. The two adapter lanes both use `MockSocket`; the lane named native exercises `WebSocketWorldTransport` through an injected factory, not a real server or native socket connection.

## Changes and preserved behavior

- Transport test `opening()` captures the next socket index before connecting and uses bounded `vi.waitFor` (2 s timeout, 1 ms polling) for that actual socket and public `opening` state. Related cancellation/opening/authentication/reentrant-live tests use the same readiness condition. Original assertions remain intact. The original connect promise is still returned, preserving its rejection assertions; the attached catch only prevents an unhandled rejection while awaiting readiness.
- A controlled deferred-ticket test in each adapter lane holds ticket authorization across the former flush interval, asserts no socket and no completed readiness, then releases authorization and performs ordinary socket open and initial snapshot admission.
- The room fixture accepts an optional ticket gate and exposes `waitForSwitch(roomId)`. It captures the request ID actually allocated by this fixture's replication request-ID supplier and current baseline stream, and requires exactly one real sent switch frame matching request ID, target room and stream. Existing synchronous `reject`/`admit` methods and fixture-consumer signatures remain compatible.
- The original confirmation test retains every capacity, explicit confirmation, old-room, rejection-message and no-art assertion, running with immediate and held target tickets. Held mode proves that visible Switching and an unchanged confirmed room precede dispatch. The rejection is injected only after the matched outbound frame.
- A second-switch probe rejects the first request, holds the next ticket, verifies one historical frame cannot satisfy current readiness, then checks the new request ID, unchanged stream and unchanged confirmed room before ordinary admitted-snapshot handling.

No production state is forced, frames invented, assertions removed, tests skipped/quarantined, or sleeps increased. Existing flush helpers are retained for negative held-gate checkpoints and unrelated settle observations, not used as proof of positive socket/frame readiness. Existing duplicate/admission/revocation/epoch/private-scope/no-replay checks remain in the affected suites.

## Controlled pre-fix evidence

The first held-ticket run with the legacy transport readiness and UI rejection sequence produced Vitest exit **1**, three failures: both adapter lanes reported `AssertionError: expected true to be false` while their socket arrays were empty; the UI reported `TypeError: Cannot read properties of undefined (reading 'requestId')` in fixture `reject`. This establishes the transport assumption before an undefined `.open` is dereferenced, and reproduces the original UI error exactly.

A final negative control temporarily replaced only `opening()` with its base implementation and removed `release(); await sent;` before the original UI rejection. New controlled gate assertions remained enabled. The test sources were restored in a Python `finally` block. Vitest again exited **1**: three failed, one passed, 162 excluded by the explicit test-name filter. Both adapter lanes again returned before authorization, and held UI again produced the undefined `requestId` error. The wrapper checked that the nested command returned 1 and exited 0 only because the intended negative control was reproduced.

Negative-control command (from the checkout root with the temporarily restored legacy readiness):

```sh
node frontend/node_modules/vitest/vitest.mjs run --root frontend --configLoader runner --no-cache src/world/network/__tests__/transport.test.ts src/features/world-rooms/__tests__/ui.test.tsx -t 'waits for a newly authorized|shows capacity'
```

To repeat that negative control from the delivered source, replace the body of test setup `opening()` with `const connected = transport.connect(resources); await flush(); return { connected, socket: all().at(-1)! }`, and replace `release(); await sent; h.reject()` in the parameterized UI test with `h.reject()`. Restore both files afterwards. These are intentional failing test-only mutations, never production changes.

## Validation and finite repeat matrix

Runtime: retained Node **v24.22.2**, Vitest **4.1.9**, oxlint **1.72.0**. Retained dependencies were exposed through a temporary removable `frontend/node_modules` symlink to the integration checkout. No install or dependency/lockfile edit; config runner and `--no-cache` avoid config bundling/cache writes. The symlink is removed before committed handoff.

The commands below run from checkout root, with `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin` prepended to PATH. Lint runs from `frontend/`.

| Check | Result | Exit |
| --- | --- | --- |
| Affected transport + complete world-rooms directory, including world-access fixture consumer | 3 files, 187 passed, zero skipped | 0 |
| Five finite repetitions of the same affected set | Each 3 files / 187 passed; 935 repeated test-case executions | 0 each |
| Combined frontend strict TypeScript | No diagnostics | 0 |
| Combined frontend lint | No diagnostics | 0 |
| `git diff --check` and exact changed-path audit | Passed | 0 |

```sh
node frontend/node_modules/vitest/vitest.mjs run --root frontend --configLoader runner --no-cache src/world/network/__tests__/transport.test.ts src/features/world-rooms/__tests__
node frontend/node_modules/typescript/bin/tsc --noEmit --incremental false --project frontend/tsconfig.json
# From frontend/:
node node_modules/oxlint/bin/oxlint src vite.config.ts
```

The five repetitions are a bounded regression check for the observed intermittent behavior. Each includes both controlled adapter connection gates, immediate/held UI target-ticket modes, and a held second request with a historical frame present. Gates release on explicit test actions, not an arbitrary longer sleep. The unmodified world-access test has 21 cases, transport has 160, and UI has six. Fixture import search found only UI and world-access source consumers. No full 96-file suite was run.

## Source and evidence bindings

Test-source SHA-256 bindings (base versus reviewed delivery content):

| Path | Base SHA-256 | Delivery SHA-256 |
| --- | --- | --- |
| `frontend/src/world/network/__tests__/transport.test.ts` | `2475980d0e2419a864c6e18c7a89214dd54654e5cc286bacb36c8f926a64de69` | `6ddf89bba4b5c46857ee3e9dbb16c22fd8d0c0588c90e8f27a2510c8158003e0` |
| `frontend/src/features/world-rooms/__tests__/ui.test.tsx` | `6188674a2c8b83d50c047f08e2263c7d4e04c1fe067698d251625088827c94fa` | `3f86f3e176fde80bd7177ca53d14f1dde64712e6919eafd537c334912291ef4f` |
| `frontend/src/features/world-rooms/__tests__/fixtures/harness.ts` | `89170257075a376cab23f9497891239f78fc6c27c6d6d50ebb9ff8f348763a2e` | `2a502b513c4ab51734393dee64f2b46983b14bb7c01dd4c9e32b55d8d9b0653d` |

Read-only production, contract and original evidence pins (unchanged from parent):

| Path | SHA-256 |
| --- | --- |
| `frontend/src/world/network/transport/engine.ts` | `0ec4c5f8132cfa1c86838ea6c0eabb6005736750f2e1e5870579fb098ddfbbac` |
| `frontend/src/world/network/replication.ts` | `568dda7c23f6d66f9cf5c8239bacf1f58ff1744b0746e07d6cc72812f76f1dd2` |
| `frontend/src/features/world-rooms/model/controller.ts` | `a2e676f2623ef7cf6c07420312a0bd76adcd02108ad4348f262ead8520358ebc` |
| `frontend/src/world/network/mock.ts` | `20a021ee8948bbc8ef1d8910c6ab3b0e12919833ec1faa6822e2337a53b40368` |
| `frontend/src/world/network/websocket.ts` | `f7c9a89e7545f9459f2d71eab8b0f5ea696db620d3685b0cc27c092b5ae75226` |
| `contracts/realtime/v1/world.schema.json` | `1732e627037fe91533ac6f7fe74e36683ce9b846bfbfc020b3cce298ae84df36` |
| `contracts/realtime/v1/policy.json` | `3e77e21b036386803c7d5bff0145c434f111ba51b861219a211e853a43d0628b` |
| `contracts/realtime/v1/operations.json` | `400d28e861ca5a87c54fde447f77b1680f7ea58e3047ee43b46db25816af6da0` |
| `contracts/fixtures/realtime/ticket-response.json` | `8162f1ac19b4caf28ca810d9c565386cf8f532b7e282b356999e06fcec4c1c54` |
| `contracts/fixtures/realtime/movement-heartbeat.json` | `fba34f3b7cba90286ae3b1a1ecd6578a79da496e79d88b920b89bd7d783499dd` |
| `contracts/fixtures/realtime/rooms.json` | `d04837604d57a40106e823596bc7f7c9d6eb07d8d8829ef0675849348d332529` |
| `frontend/src/features/me/__tests__/visual-adoption/evidence/validation.json` | `41c19562b1cfe2cc05b99f90401ecad3f1bc20c885d41264f7e5b552a0c92890` |

Transient command log bindings are recorded below; full logs remain local in `/tmp` and are not a durable-delivery dependency. The report preserves the durable result counts, commands, failure details and limits above.

| Local log | SHA-256 |
| --- | --- |
| `/tmp/mcr-world-async-before.log` | `be28adbc31940784a384c88f2beb639a8276c2cf91565aa2727bfc6943285ada` |
| `/tmp/mcr-world-async-negative-control.log` | `361647c7b346b1a50e80ecc1bb4df17ec59d095317a952abe2e85d4bef5e0c8a` |
| `/tmp/mcr-world-async-after.log` | `a7e3a48566bcad2e8251738255234f750ddd250c03946d47242d401e0a84c350` |
| `/tmp/mcr-world-async-repeat-1.log` | `6f3937db162fb91a452b692c08ffe094dc260c4f6e791847e83e6ccfb473f192` |
| `/tmp/mcr-world-async-repeat-2.log` | `bcfcad4cae8be05748cc5e1024ea70efeb7638e09f232a4dbb10b546af4e832d` |
| `/tmp/mcr-world-async-repeat-3.log` | `36cfa4ee54d63505f01cc6ef2a99132fdc53c1e0714adf0c69c3da8484a0df4d` |
| `/tmp/mcr-world-async-repeat-4.log` | `cb832e45c95d8e4c9e39d831d0bf36cfdca69f9038ba91052e5e825988495a26` |
| `/tmp/mcr-world-async-repeat-5.log` | `d1f8c55410314b8d8a3c87cdfa15f18a5330edb0e6c6d0e6b602884c4ee5d99f` |

## Limits and ownership

The unrelated direct-beacon Back navigation failure remains **unresolved / NOT_RUN here**: historical evidence records expected `/world`, observed `/meet`. Its owner remains the meeting-entry/Main integration lane. No router or meeting-entry file was changed. Historical missing Python format-checker failures also remain outside this test-only scope. A focused green run does not certify the full application, a real multiuser flow, browser/device behavior, database/load checks, or release acceptance.

The granted writable set is exactly the three test-source paths in the hash table and this new report. No backend, rendering, production transport/controller, mock implementation, shared contracts, root configuration, manifests, lockfiles, artwork, generated artifacts or original UIV04 evidence changed. All new imports are test-library imports within frontend tests; no cross-directory implementation import was added.

A fresh actual `get_goal` read during this assignment returned **active** for chat `01a19f83-2c2a-7fa0-a98d-1c4f5f65cb3c`, with an aligned QA-WORLD-ASYNC objective covering these exact files, controlled reproduction, validation, committed clean READY handoff and concrete review fixes; `remainingTokens=null` confirms no token budget. The aligned goal was reused. Main reviews/integrates this committed handoff; the writer stops after handoff and handles concrete scoped review fixes if assigned. This is not a new approval gate.
