# FE16 original transport parity reconciliation

Assignment: **FE16-ORIGINAL-PARITY-01**, original task **FE16**. Source baseline:
`db661820d6af1770839cfda8d0dc6b9ada1ece1b`. This reconciliation addresses the
original task's acceptance sentence: “Identical scenario suite runs with both
transports; mock code excluded from production mode.” It adds no production
behavior, test cases, dependencies, migrations, contracts or product scope.

Disposition: **ACCEPTED for the original FE16 criteria**, through published owner
evidence and completed current source binding. Canonical publication of this
reconciliation is the remaining delivery step. Historical
**MOCK_READY** labels remain historical. This record does not relabel fixture
tests as real-service, device, capacity, IN01 or release acceptance. Those are
separate tasks, not prerequisites silently added to FE16.

## Criterion disposition

| Original obligation | Canonical implementation and scenario | Published evidence and current binding |
| --- | --- | --- |
| Same interface for mock and real transport | `frontend/src/world/network/WorldTransport.ts` defines the typed interface. `mock.ts` and `websocket.ts` both extend the same `TransportEngine`; the latter defaults to browser `WebSocket`. No alternate identity or product roster is introduced. | Both adapters and the interface are unchanged from original integrated source `5863f915eae1526786859876d22a781b48ca5f7c`. The source-binding manifest compares current bytes with the later qualified checkpoint as well. |
| Ticket/authenticate and typed messages | Shared `engine.ts` captures the existing FE01 session generation, acquires a fresh one-use ticket through `AcquireWorldTicket`, checks bindings/TTL and sends authenticate first. `contract.ts` reads canonical C05/C03/C06 schemas; generated `wire-types.ts` records their hashes. The shared suite checks deferred tickets, wrong bindings, expiry, unknown/oversized/binary/invalid frames, unsafe origins, cancellation and fresh-ticket reconnect. | Original 158 transport cases are included in the published 568-test run. Readiness repair `4b1f8c4fa646eca3b6c93d8c4e673fd820ac69a3` retains assertions and adds one deferred-ticket case per adapter, making 160. Later owner run `ROOT-SOCIAL-WIRE-SOURCE-202611041103Z` executes this same common suite against current runtime/schema changes. |
| Sequence and close handling | The identical suite checks gap/epoch freeze, one resync notification, queued-delta discard, correlated replacement, input/ack/tick ordering, room-switch acceptance/rejection, pre-send denial versus post-send uncertainty, heartbeat, explicit reconnect, revoke, stale generations, bounded buffers/rates and listener/timer cleanup. | Original engine fixes are included in `5863f915`; later Social metadata changes were qualified with the common transport, replication and policy-wire suites, 195 passing cases across three files. No original scenario is deleted or skipped by the readiness repair. |
| Identical scenario suite with both transports | `transport.test.ts` encloses the same fixture/scenario definitions in `for (const lane of ['mock','native'])`. The mock lane constructs `MockWorldTransport`; the native-adapter lane constructs `WebSocketWorldTransport` with an injected `MockSocket` factory. Both use canonical fixture JSON, clock and assertions. | The 158 and later 160 counts are totals for the complete transport file, including shared schema cases; they are not counts per adapter. The later 195 total includes transport160 plus adjacent suites. This is adapter parity, not proof that all 160 cases used actual sockets. Separately, the installed-Chrome runner replays all five canonical traces using actual native WebSockets. |
| Mock code excluded from production mode | Production composition imports `../network/websocket`; the adapter imports only the shared engine. `mock.ts` is an explicit test/development import. The shipped `frontend/index.html` selects `src/main.tsx`, with ordinary Vite production configuration. | Historical built preview passes no test entry/global and no product socket, at a time when FE16 was not yet imported by the application. That result alone cannot establish today's closure. Current source-only qualification follows all literal static/re-export/type/dynamic edges from the shipped entry, checks that the native adapter is reachable and `mock.ts`/test modules are not, and rejects unresolved relative imports or computed imports. It does not claim a fresh built-preview execution. |

## Evidence lineage and immutable inputs

The original owner report is [world-transport.md](world-transport.md). The
published integration record is
[world-transport-integration.md](../evidence/world-transport-integration.md).
Its [inventory](../evidence/world-transport/hashes.json) binds all twelve original
paths to integrated source `5863f915`. The source-only qualification checks those
historical bytes against the recorded hashes, rather than treating the old
inventory as a current-source assertion. Original evidence was published in
`3c9d2f75a0827d29809bc4ccbe55e8879156089d`.

| Responsible-owner result | Exact durable evidence | Meaning and limits |
| --- | --- | --- |
| `vitest run`, exit 0, 568 tests in 15 files, including transport158 | `docs/mcr/evidence/world-transport/tests.log`; integration report and original source inventory | Historical common-suite qualification on `5863f915`; not today's test count. |
| Strict `tsc -b && vite build`, lint, exit 0 | `docs/mcr/evidence/world-transport/build.log`, `lint.log`; integration report | Historical build, with existing large-chunk warning; no performance assertion. |
| Installed Chrome 156.0.8078.80 native fixture run, exit 0 | `docs/mcr/evidence/world-transport/browser.log`, `browser-results.json`; unchanged `frontend/src/world/network/__tests__/browser-smoke.mjs` | Five traces: movement-heartbeat, resync, room-switch, epoch-replacement and revoke. Ephemeral loopback server is test infrastructure, not BE10/BE11. Canonical trace driver and browser fixture remain unchanged. |
| Readiness correction, affected set187 and five finite repetitions, exit 0 each | `docs/mcr/qa/world-async-tests.md`, source `4b1f8c4f` | Replaces fixed-flush readiness assumptions; controlled prior failures remain documented. Transport160 includes both adapter lanes. No production change. |
| Latest relevant generated drift, tests195, scoped types/lint, exit 0 | `docs/mcr/evidence/social-policy-wire/source-01/release.json`, `tests.log`, config `frontend/src/world/network/__tests__/social-policy-wire.config.ts` | Run source `2d7a42e7d9fc645a3d956cd552304539be602be7`; published generated postimage and receipt `7a94c5e7a4754ee178405be6682cf61218f61bcf`. Config explicitly includes common transport, replication and policy-wire tests. |

FE01 remains the single canonical `SessionPort`/`SessionScope` and
`TransportError` boundary in `frontend/src/shared/api/{http,errors}.ts`; see
[its port contract](http-transport.md). C05 authority is
`contracts/realtime/v1/{world.schema,policy,operations}.json` plus canonical
fixtures, with C03 core and C06 avatar references. These are read-only inputs.
No historical schema hash is presented as today's frozen version: the manifest
records exact current hashes and compares them with the latest owner run.

The original schema registry later gained the reviewed-avatar documents, and
the wire union gained explicit Social policy metadata. The published
[reviewed-avatar boundary](reviewed-motion-v2-boundary.md) and
[Social wire qualification](social-policy-production.md) document those changes.
Current engine, registry, interface, adapters, tests, schemas, fixtures and retained
dependency manifests match `7a94c5e7`. The one subsequent shared API generated-file
delta updates a source-hash comment only; executable bytes are unchanged.
`wire-types.ts` is the output generated by the recorded owner workflow, not the
stale input version at `2d7a42e7`. The manifest records both versions explicitly.
That worker run-source object is not an ancestor of the canonical base
(`merge-base --is-ancestor` exits 1); the publication checkpoint `7a94c5e7` is an
ancestor. Exact source comparisons establish the connection instead of inventing
worker ancestry.

## Current source-only qualification

Named allocation: `FE16-PARITY-SOURCE-B-DIRECT-NEXT-202611041700Z`, accepted after
FE209's actual SourceB FULL release. Root corrected the whole accounting to a
conservative **2 GiB**, including the retained checkout and private Git carry;
it was not described as a 128 MiB checkout. New allocation stayed within 8 MiB,
outputs within 5 MiB, with at most a Python process and sequential Git child.
The guard records exact source40, tool hash, custody, UTC/deadlines and fresh
capacity before the first child. It enforces a conservative 16 GiB start floor,
8 GiB live floor and 9 GiB aggregate margin.

Command from the exclusive checkout root:

```sh
python3 evidence/fe16-original-parity/qualify.py --allocation FE16-PARITY-SOURCE-B-DIRECT-NEXT-202611041700Z --whole-limit-mib 2048
```

Final exit **0**, **SOURCE_ONLY_PASS**: 72 source bindings and a conservative
454-module import closure. All five original trace fixtures, adapters and the
native runner/fixture remain byte-identical to the original integrated version.
Generated schema source hashes match current canonical inputs. No production
mock or test module is reachable; the native WebSocket adapter is reachable.

The durable [report](../../../evidence/fe16-original-parity/qualification.json)
records source/evidence SHA-256 values, canonical ancestry, worker-run
input/generated-postimage distinctions, closure edges and resource facts.
Two referenced PNGs are tracked but unmaterialized by this sparse checkout;
the checker reads their canonical Git objects without restoring files. The
closure is a conservative lexical source check, including type-only imports;
its external-token catalog is not an asserted runtime dependency inventory.
It is explicitly separate from historical built-preview/native results.

Two checker failures remain in `evidence/fe16-original-parity/failure-*.json`,
with their corresponding guard records: the first incorrectly required worker
run-source ancestry; the second assumed every referenced asset was materialized.
Neither establishes a production defect. Both assumptions were corrected, then
qualification completed within **80.52 seconds of the first attempt**, before
the original 180-second work deadline. Final receipt UTC is
`2026-11-04T17:05:20.779368+00:00`. Combined conservative parent/child peak RSS was
52,117,504 bytes; final free space was 31,552,978,944 bytes. SourceB was fully
released directly to Profile213 after process exit. No dependency link, temporary
cache, browser, server, port, application suite, build or database was created.
No duplicate tests or historical trace replay were performed.

## Delivery and limits

Exclusive existing checkout loan: `sl-world`; task branch
`codex/mcr-fe-FE16-original-parity`. Former World writer is stopped; its preserved
branch `codex/mcr-world-progress-publish` remains at
`3ee442c7661c17d93168a4657c64a6ed06ffad98`. Only this new document and
`evidence/fe16-original-parity/**` are changed. The granted new test prefix is
unused. No production lease was needed, and supplied originals remain untouched.
Exact-path and whitespace checks accompany the committed handoff; no source
origin, contract or planning exporter changed, so their unrelated suites were
not rerun.

Root153 receives the exact committed handoff and publishes through the existing
serialized route. One canonical publisher receipt establishes delivery; no
king/SL technical review, acknowledgment, duplicate run or proof-only follow-up
is required. The author stops all writes and returns a clean loaned checkout to
World after delivery, preserving both branches and commits. Current whole-app
build, real-server/device/load/release results are not claimed by this record.
