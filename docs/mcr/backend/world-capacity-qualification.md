# QA05 / QA08 public World qualification drivers

## Delivery and evidence boundary

This is a source contribution to original QA05 and QA08. Source and offline fixtures are `MOCK_READY`; actual public HTTP/WS execution, the original 200-socket/four-room run, serving-process restart, server queue measurements, database cleanup, physical mobile performance and chosen production caps are `NOT_RUN`. Neither task is `ACCEPTED`. A successful driver result means its selected observations completed, with acceptance always `NOT_CLAIMED`. Missing hooks remain explicitly `NOT_RUN`; they are never inferred to be zero.

Authorized base: `0bd35b223d2590fb2b343171d6d8ac30d37ff857`. Writer: `codex/mcr-be-QA08-world-capacity`, isolated worktree. Root integrates and publishes. No serving process, credentials, database, migration, contracts or application code was changed. Heavy execution requires its own named BackendSL allocation; this source allocation does not grant one.

The nine delivered paths are:

- `backend/scripts/mcr/world_capacity_probe.py`
- `backend/scripts/mcr/world_capacity_support/__init__.py`
- `backend/scripts/mcr/world_capacity_support/transport.py`
- `backend/scripts/mcr/world_capacity_support/scenarios.py`
- `backend/scripts/mcr/world_capacity_support/report.py`
- `backend/scripts/mcr/test_world_capacity_probe.py`
- `backend/scripts/mcr/world_live_failure_probe.py`
- `backend/scripts/mcr/test_world_live_failure_probe.py`
- `docs/mcr/backend/world-capacity-qualification.md`

## Provenance and dependencies

All nine files are newly authored. No Bigyeting rendering, legacy art, external source or implementation physics was copied. The existing two-principal helper was read for public ingress/header compatibility, not modified or imported. C03 bootstrap validation uses the existing `scripts/mcr/validate-core.py` and committed generated core OpenAPI; C05 uses the existing `scripts/mcr/validate-realtime.py` offline registry. Full supplied map/avatar/theme bundles are validated, compared against each principal's actual bootstrap and hashed in the report. Synthetic contract fixture art remains test metadata, never product art.

Runtime dependencies are Python with `asyncio.timeout` support (validated on Python 3.14) and the existing pinned `scripts/mcr/requirements-contracts.txt`. Network framing uses the standard library. No package was installed. System Python lacked the required `date-time` and `uri` format checkers; validation failed closed. The already prepared contracts environment supplied them. Source reports SHA-256-pin both entrypoints, all support modules, existing validators/generator, dependency declaration, core API and frozen realtime/core/asset schemas and policy files. Preserve those pins alongside the candidate commit and the private producer allocation record.

## Private input

Both commands accept JSON through stdin or `--input` pointing to an absolute, owned, regular, single-link mode-0600 file. Symlinks and broad file permissions are rejected. Input is bounded to 8 MiB; duplicate JSON keys and nonfinite values are rejected. Never put bearers in command arguments, shell history, public artifacts or checked-in fixtures. Both commands require `--execute`; without it they return `NOT_RUN`, exit 2 and do not read credentials or open connections.

The private object has exactly these required fields:

| Field | Producer supplies |
| --- | --- |
| `allocation_id` | Nonempty identifier of the separately granted run allocation; not printed. |
| `gateway_url` | Numeric-IP HTTP(S) origin, no credentials/path/query/fragment. |
| `ws_url` | Numeric-IP WS(S) endpoint with exactly `/world-ws`, no query credentials. |
| `browser_origin` | Allowed HTTP(S) browser origin, including port where needed. |
| `event_id` | Actual canonical event UUID. |
| `bootstrap` | Full expected public C03 bootstrap with the selected approved resource manifests; World must be available. |
| `principals` | Rows `{ "bearer": "<private existing token>", "room_index": 0 }`; each token and actual bootstrap canonical user must be distinct. No signup or manufactured runtime identities. |
| `rooms` | Rows `{ "room_id": "<registered UUID>", "capacity": 50 }`; actual public catalog must match. |
| `schedule` | Required numeric fields below; optional reconnect/slow-reader schedule. |

Schedule example (operating parameters, not acceptance thresholds):

```json
{
  "arrival_concurrency": 20,
  "operation_timeout_seconds": 20,
  "measurement_seconds": 60,
  "total_timeout_seconds": 180,
  "input_hz": 10,
  "reconnect": [{ "after_seconds": 20, "participants": [0, 1] }],
  "slow_reader": { "participant_index": 2, "after_seconds": 30, "pause_seconds": 5 }
}
```

Arrival concurrency is 1..cohort size; operation timeout .05..60 seconds; measurement .05..3600 seconds; whole-run timeout .1..7200 seconds and longer than measurement; zero-axis input rate 0..30 Hz. At most 16 explicit reconnect cohorts are allowed. Slow-reader pause and scheduled actions must fit the measurement window. Missed actions fail with a fixed label. These are resource bounds and explicit choices; the specification supplies no invented minimum soak duration or latency pass threshold. Producer must choose a practical duration, storm cohort and timeouts for the granted resource window.

QA08 requires exactly 200 distinct actual principals and exactly four registered rooms of capacity 50 with 50 assigned to each. It opens actual public HTTP/WS connections, verifies complete per-room cohort convergence, tries one normal public ticketed switch into a full room, verifies `ROOM_FULL` retains the old stream/roster and resumes input ACK progress, then measures the requested schedule. A 200-row config validation test is not a 200-socket result. Use isolated empty rooms with a cohort whose privacy projection legitimately includes every same-room participant; extra or invisible principals fail the cohort oracle.

QA05 permits 2..50 principals and 1..16 registered rooms. Its only additional input is optional `faults`, selected without duplicates from:

| Fault | Observation and limit |
| --- | --- |
| `ticket_replay` | Reuse consumed real ticket on an extra actual socket; reject any admission and require close 4401. |
| `application_gap` | Discard one received application delta; next gap withdraws roster, requests correlated resync, and checks a fresh complete baseline. Requires positive input rate. This is not network packet loss. |
| `application_delay`, `slow_receiver` | Pause the configured participant's client socket reads, then explicitly reconnect and converge. Requires `schedule.slow_reader`. This does not emulate mobile suspension or network impairment. |
| `room_switch` | Ticketed public switch to another configured room; successful fresh baseline or full-room rejection plus old-stream ACK progress as appropriate. |
| `world_restart` | External producer checkpoint, actual peer disconnection, fresh admission, changed public room epochs and convergence. Missing producer hook reports `NOT_RUN_NO_PRODUCER_HOOK`. |

## Execution and producer-owned hooks

From the checkout root, using a contracts-capable Python chosen by the producer:

```sh
python3 -B backend/scripts/mcr/world_capacity_probe.py --execute --input /absolute/private-capacity-input.json
python3 -B backend/scripts/mcr/world_live_failure_probe.py --execute --input /absolute/private-failure-input.json
```

For restart, use `world_live_failure_probe.py --execute --input /absolute/private-failure-input.json --restart-checkpoint` with controlled stdin. The driver emits one fixed JSON line `{"event":"world-restart-requested","owner":"external-producer"}`. The authorized producer restarts the intended serving process through its own lifecycle controls, then sends exactly `{"checkpoint":"world-restart-complete"}` and a newline on stdin before the configured operation deadline. The driver never invokes a shell, sends a signal, runs SQL or starts/stops a server. It independently checks public disconnection and changed epochs; process/PID evidence remains producer-owned. Private input must be a separate file because stdin is reserved for this bounded checkpoint.

Programmatic `world_capacity_support.scenarios.run(...)` accepts an async `restart_hook` and an optional async `observation_hook`. Root owns their implementation and evidence. The latter runs after client cleanup, has a five-second deadline and permits only numeric `server_cpu_seconds`, `server_rss_bytes`, integer `memberships` and `prepared_transactions`. Absent hook is `NOT_RUN`; bad/missing producer data is `UNAVAILABLE`. Server queue observation has no implementation in this contribution and remains `NOT_RUN`. Counts supplied by a hook are producer observations, not an independent SQL audit. CLI does not accept arbitrary executable hook commands.

Exit 0 means selected driver steps completed; exit 1 means fixed-label failure; exit 2 means no execution requested. Inspect per-step statuses, missing oracles, source pins and limitations even on exit 0. No result promotes a task or changes production caps.

## Measurements and safety bounds

Reports contain safe numeric measurements and fixed labels, not bearers, tickets, raw actor/event/room IDs, endpoints, messages or exception text. They include admitted and peak socket counts (including replay probes), verified canonical-principal/room counts, configured ordinal schedule, public resource hash, HTTP/admission timings, client event-loop lateness, client CPU seconds, latest-connection input ACK latency and transport byte/frame counters. Counters from earlier connections are retained, whereas ACK samples explicitly describe only the latest connection per principal. Input is zero-axis; no private 8px/tick or negotiated motion profile is invented.

Measurement active-socket minimum is sampled between actions and does not measure the low-water mark inside an awaited reconnect cohort. Tick comparison is explicitly latest-stream versus initial tick, so resets can affect it; it is not an FPS or simulation-rate acceptance check. Configured read pause can follow an already pending receive. `observed_client_read_pause_seconds` reports actual time spent in read-loop suspension; a configured pause alone is not proof of network backpressure. Expected slow-reader backpressure control suspends input and is followed by explicit fresh admission.

Samples retain the first 4096 values, with exact full-run count/mean/max and clearly labeled retained-window percentiles. Each client keeps at most 64 unacknowledged inputs, one roster of at most 50 players, bounded stream-reset history, and one persistent socket per admitted connection. HTTP bodies are bounded to 2 MiB, headers to 8 KiB, client messages to 4 KiB and server messages to 64 KiB. WebSocket parsing checks masks, fragmentation, UTF-8, control-frame budgets, close codes and strict JSON. Schema/resource/stream/epoch/sequence/ACK/correlation violations fail closed.

`max_buffered_bytes` measures the Python client's receive buffer and `max_write_buffer_bytes` its transport write buffer. Neither is server queue depth or process memory. Client handle closure and close ACKs are reported separately from database membership cleanup. Deadline failures cancel pending admission tasks and close admitted handles. This driver does not prove server membership deletion, absence of prepared transactions, physical mobile rendering, browser background/foreground, bundle size, heap stability or chosen production limits.

## Validation and remaining gates

Exact new-suite command used in the prepared environment:

```sh
PYTHONPATH=backend/scripts/mcr /private/tmp/mcr-contracts-admission/bin/python -B -m unittest test_world_capacity_probe test_world_live_failure_probe
```

35 tests passed, exit 0. Tests use in-memory stream bytes and a two-principal fixture producer; no listener, Cargo build, PG or browser was started. They cover frozen C03/C05 validation, original scale configuration, malformed credentials/config/JSON, file permissions, HTTP framing/limits/policy, masked WebSocket control handling, fragmentation/UTF-8/close codes, cancellation, input bounds, ACK/epoch/gap/reset checks, distinct actual-identity oracle behavior, complete fixture admission, replay, room switch, reconnect/read pause, producer restart signal and cleanup/oracle redaction. The fixture producer is explicitly `IN_MEMORY_FIXTURE`, not public-stack evidence.

Targeted read-only peer review identified three transport defects before delivery: graceful-close timeout did not force transport abortion, explicit reserved wire close code 1005 was accepted, and concurrent close/data writes were not fenced. All three were fixed with reproducer tests: cleanup aborts the underlying transport after stalled graceful shutdown, empty close payload is distinguished from forbidden explicit 1005, and close state changes under the frame-write lock before draining. No source or test writes were delegated.

`PYTHONDONTWRITEBYTECODE=1 python3 -B scripts/mcr/check-plan.py` passed, exit 0: all 13 original archive hashes match, pinned narrative matches, both nine-probe exporters reproduce, delivery metadata points to `julymeltdown/mcr/master`, and all 112 required release tasks/61 required IA entries remain accounted for. This checks planning provenance only. `git diff --check` is the scoped whitespace check; application build, public database/browser/load/release checks remain separate and `NOT_RUN` here.

Earlier system-Python execution failed because required format checkers were absent. An overly broad discovery pattern also selected existing two-principal tests whose release fixture is outside this sparse checkout; the final command explicitly selects only the two granted suites. Those attempts are not passing evidence. No existing tests or validators were modified to accommodate the sparse checkout.

Root's next runtime allocation must supply actual approved manifests, distinct existing credentials, four registered empty rooms with authoritative capacity 50, the public serving endpoints, process evidence for any restart and a separately validated cleanup/metrics oracle. Preserve the exact candidate ancestry and actual command exits/output. Existing historical native/regression evidence and frontend scripted/desktop measurements retain their earlier boundaries; this source delivery does not upgrade them or replace the original QA05/QA08 requirements.
