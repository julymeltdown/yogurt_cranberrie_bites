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

## Bounded producer profiles — original QA05 / QA08 continuation

The resumed single-writer grant retains the original nine paths and adds exactly `backend/scripts/mcr/world_in01_prepare.py`, `backend/scripts/mcr/world_in01_live.py`, and `backend/scripts/mcr/test_world_in01_profiles.py`. This continuation modifies the existing producer and probes; it creates no additional helper, task, checkout, integration gate or review stage. The default `in01` producer remains two principals/one capacity-two room with its existing held-runtime behavior. No king or SL technical review, acceptance or ACK is required. Publication follows the named publishing developer; validation belongs to this implementation owner. The historical unfinished goal remains `BLOCKED` in persistent tool metadata; it was not reset or falsely completed.

`--qualification-profile qa05` creates two actual independent Auth accounts and two capacity-two rooms. `qa08` targets 200 actual independent Auth accounts and four capacity-50 rooms. Accounts use real public signup, private mail delivery, verification and login. Canonical identities are checked for uniqueness. Normal admission refusal is a failure, never an automatic retry, forged session, SQL-created Auth identity or disabled guard. Operator SQL changes only owned event/room configuration; participant/admission/avatar creation still occurs through authenticated bootstrap. No token is persisted: credentials files keep the existing `MEMORY_ONLY` sentinel, while fresh bearer objects pass directly from the producer to `world_capacity_support.scenarios.run` in the same process.

Provisioning has an explicit deadline (default 120 seconds), checked between accounts and before successive authentication operations. The existing public HTTP operation timeout remains ten seconds, so a currently executing request may finish after the provisioning deadline before cleanup begins. Qualification profiles have no held external descriptor and require World enabled, zero hold, an explicit allocation identifier, whole byte cap and measured retained carry. These parameters identify an independently accepted resource round; supplying arguments alone does not allocate resources.

The producer owns its direct Concert child. Optional `--restart-world` is restricted to QA05: stop that exact unreaped child, require unforced exit zero, reserve its released loopback ports with ordinary TCP `SO_REUSEADDR` (never `SO_REUSEPORT`), start the same verified binary with fresh owner/producer identifiers, and check listener ownership/readiness. The existing failure probe additionally checks peer disconnection, fresh public admission, changed epochs and convergence. After probe handle cleanup, actual owned PostgreSQL queries poll memberships and prepared transactions; any nonzero or unavailable oracle fails the producer. Server queue metrics and physical mobile measurements remain `NOT_RUN`.

An actual source compatibility fix stages QA08 admission: the qualified server refuses public ticket issuance once a target room is full. The driver therefore admits all except the target's final participant, issues a normal switch ticket while that slot is free, fills the final slot normally, and only then attempts the switch. The result must be `ROOM_FULL` while the old stream/roster and ACK progress survive. No capacity guard is bypassed and no private ticket provider is introduced. A two-client offline control-order fixture covers this staging; it is not a 200-socket test.

QA08 uses globally paced public HTTP starts at 0.12 seconds and a bounded arrival cohort. Its explicit reconnect schedule includes all 200 socket ordinals; a configured slow reader is separately reconnected. All input remains zero-axis. The measurement duration/rates are chosen operating parameters, not invented latency, FPS, soak or capacity acceptance thresholds.

### Reusable native inputs and normal Gateway configuration proposal

The qualified immutable peer cohort is `/private/tmp/mcr-root-eighteen-34fa7a6d-pj9ld_8h/bin`, manifest `cohort.json` SHA-256 `dd43f3f629a958f3b126bacc039f6d3413f012b4e42c7f59528b0f3e617b8da5`. Gateway/Auth/Friend/Notification compiled source is `34fa7a6d60f6e771f63db54f4a91e8fd1ebf2f8c`; Post/Profile compiled source is `d0f063d18a0549935f80a536077c30962d89f9e9`. All six native file hashes and logical sizes were freshly checked against that manifest without executing them. Every peer migration tree matches this checkout's source postimages (Gateway 17, Auth 18, Friend 18, Notification 6, Post 31, Profile 9).

Concert is `/private/tmp/mcr-c03-read-green-7cd1e900-xvpia6kp/concert-service`, SHA-256 `b1ea0a07eab66aa2c1c313476b063b61cf54ee678ad0a0e44633944815f65433`, compiled source `7cd1e900d6bd1b4994482819486c828e43736822`. All 37 Concert migration files match its retained source-pin manifest. Qualification preflight verifies these immutable native objects and refuses a different peer manifest or Concert executable. This is reuse of earlier actual qualification, not a fresh build or runtime result.

The admitted resource tuple remains `mcr_reference_scene_r3` revision 4 and `mcr_selected_static_20261103` version 1. Preflight checks the complete original quality-v6 bundle hashes and rejects Map6/V10. Existing sparse-checkout bundle/migration inputs were materialized read-only; supplied art, contracts, manifests and migrations were not edited. Initial static release preflight identified two missing reference-scene quality-v6 input files; materializing those pinned inputs resolved that preparation failure. No application source rebase or native build was necessary.

Actual compiled Gateway policy classifies signup **and** email verification as `Signup`. At source `34fa7a6d`, `backend/crates/gateway-domain/src/admission.rs` defaults are signup 10/client-network/minute, login 30, write 300 and read 600, with enforcement enabled. Thus 200 genuine accounts require 400 signup-class requests and 200 logins; the default signup budget requires roughly 40 minutes. A short QA08 run must not use defaults and hope to pass or disable admission.

The explicit normal-configuration proposal is `GATEWAY_ADMISSION_BUDGETS=signup=400,login=200`, retaining `GATEWAY_ADMISSION_MODE=enforce`, normal identity checks, default concurrency ceilings, write/read budgets and durable admission behavior. The producer accepts only this fixed nonzero proposal through `--qa08-provisioning-budget signup=400,login=200`; it does not inherit arbitrary settings. The option is mandatory for QA08 and forbidden for other profiles. It is **not yet authorization to execute**: a fresh resource offer must explicitly cover the human-authorized normal configuration. Failure, 429, expired offer or insufficient provisioning time ends the stage and triggers cleanup/requeue; no silent budget increase, deadline extension or quota retry is allowed. QA05 fits default signup/login budgets and proceeds independently.

### Exact prospective QA05 command and resource boundary

From the existing `be-world-capacity` checkout, after a named fresh offer has been accepted and a new private mode-0700 output directory has been allocated:

```sh
/private/tmp/mcr-contracts-admission/bin/python -B backend/scripts/mcr/world_in01_live.py \
  --world --qualification-profile qa05 --allocation-id <ACCEPTED_OFFER_ID> \
  --browser-origin http://localhost:5173 --hold-seconds 0 \
  --bin-dir /private/tmp/mcr-root-eighteen-34fa7a6d-pj9ld_8h/bin \
  --concert-bin /private/tmp/mcr-c03-read-green-7cd1e900-xvpia6kp/concert-service \
  --report <FRESH_PRIVATE_OUTPUT>/report.json \
  --provision-timeout-seconds 120 --probe-timeout-seconds 120 \
  --measurement-seconds 20 --input-hz 15 --arrival-concurrency 2 \
  --whole-timeout-seconds 600 --whole-byte-limit <ACCEPTED_WHOLE_BYTES> \
  --retained-carry-bytes <FRESH_MEASURED_COHORT_SOURCE_PROOF_CARRY> \
  --restart-world --retain-public-projection
```

The two explicit placeholders for accepted limits/carry are mandatory and cannot be truthfully replaced before allocator disposition. Optional projection does not require a frontend process; an exact provided frontend consumer commit can be recorded with `--frontend-source-revision`, separately from `frontend_runtime: NOT_RUN`. The source-qualified FE reference supplied during preparation is `7be5e20bb4f5fa667878dd4aee98331144634b6a`; receiving FE HEAD was `b4325b82d34686feacf52147142721a954841934`. Neither pin claims actual native admission or a final future consumer candidate.

The proposed round requires one BACKEND workflow and one new isolated PostgreSQL cluster, the six qualified peer services, Concert, Redis, private mail/control helpers and Python controller/probe. No Cargo, native artifact copy/build, frontend browser, new checkout, cache or installation is required. Root measured retained native allocated bytes as 259,817,472 for the peer directory and 49,524,736 for Concert: **309,342,208 bytes** before source/proof and new owned outputs. Logical file totals differ (259,806,464 plus 49,521,632) and are not substituted for allocated carry. Prospective whole estimates must add source/proof carry, owned PG/TMP/logs/reports and optional projection; no implicit one-GiB waiver or extension exists.

The source guard samples retained carry plus this run's private output/owned PG allocations every 0.5 seconds, refuses symlinks/replaced output custody, requires four GiB free, and interrupts only its own producer on cap/deadline failure. A 180-second cleanup reserve is removed from the work deadline. Sampling is not an atomic disk quota; observed high-water and final teardown evidence must accompany the accepted limit. The existing child handles and owned cluster perform cleanup. No old ready file, expired descriptor or historical controller is executable authority for this round.

Preparation also measured 866,938,880 allocated bytes of still-retained historical evidence/native artifacts in `/private/tmp/mcr-world-invalidation-20261104`; nothing was deleted or silently exempted. With the measured native cohort and 12 assigned source paths, carry was 1,176,498,176 bytes before new PG/TMP/logs/reports. Refresh this measurement at acceptance because documentation/output changes affect source carry. The concrete pair proposal is therefore 600 seconds, one BACKEND workflow/one PG, expected whole footprint below 1.5 GiB and a requested hard whole limit of **2,147,483,648 bytes** including retained carry. This is a request, not a granted cap. No old allocation, one-GiB assumption or native artifact cleanup authorization is reused.

### Consumer-safe projection and privacy

The earlier raw-body retention request was withdrawn after inspecting C03 `PrivateParticipant`. Full authenticated C03 includes age/preferences/profile/identity context and is **not public data**. It is validated only in memory; no raw body or full-body digest is retained, copied or published. Tokens, cookies, headers, identities, preferences and age are absent from retained outputs.

Optional `--retain-public-projection` writes only `public-resource-capability-projection.json` in the same private output directory, outside the ephemeral PG root. It is mode 0600, explicitly `NOT_BYTE_EXACT_FULL_C03`, and hashes **only projection bytes**. It retains nonprivate resource descriptors, capability outcomes, source/runtime/contract pins and actual HTTP timestamp/status. Map4 public descriptors contain place UUIDs; to honor the no-UUID boundary, the projection omits map geometry/places and theme place-ID lists. It explicitly states `resources_are_complete_manifests: false`; it cannot be passed off as a complete C03 response or unchanged contract manifest. All actual source manifests and production guards remain untouched. A recursive scan refuses credential/private-field names or UUID strings in the retained projection.

Producer custody persists until direct consumer readback or transfer, within the same output/carry budget. Exact path, projection SHA-256/bytes, contract/source tuple and custody go directly to Root153/26e and WorldSL only after actual production. No artifact exists yet. Any later requirement for the full private response needs an in-memory transfer in an already named round; it does not justify persistent private retention or another Map run. Bounded frontend contract fixtures remain separately labelled and do not satisfy live admission.

### Source validation disposition

The three scoped suites (`test_world_capacity_probe`, `test_world_live_failure_probe`, `test_world_in01_profiles`) cover producer account/deadline/refusal behavior, operator-only room seed, default IN01 compatibility, fixed original cohorts, in-memory token handoff, real-oracle gating, owned restart controls, carry/deadline guard, Map4 fence, public projection privacy and staged ticket admission. All are offline fixtures/static checks, not a new actual Auth/PG/restart/capacity result. Source remains `MOCK_READY` until actual original runtime observations are recorded. Actual pair/200/four-room/storm/restart/slow-receiver/ghost behavior and full QA05/QA08 acceptance are not inferred from these suites.

Continuation verification: `PYTHONPATH=backend/scripts/mcr /private/tmp/mcr-contracts-admission/bin/python -B -m unittest test_world_capacity_probe test_world_live_failure_probe test_world_in01_profiles` passed 51 tests, exit 0. Static complete Map4/static1 bundle pin validation passed with no native/network activity. Scoped AST parsing and `git diff --check` passed. An initial seed-fixture test lacked a mocked binary digest and was corrected; no native test executable was invoked. The earlier 35-test evidence remains historical source evidence, not additional runtime coverage.
