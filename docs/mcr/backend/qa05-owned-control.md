# QA05 owned fault control

Status: IN_PROGRESS. Source protocol checks PASS; actual managed attachment is
NOT_RUN. This is the remaining original QA05 combined App/Auth/
World frontier, not a new IN04 gate or QA08/performance qualification.

## Objective and completion criteria

QA05-OWNER-CONTROL-01: implement an owner-bound control attached to Root's existing
single real stack and exact two App principals. Deliver real requested faults,
bounded request/checkpoint/observation APIs, source failure tests, provenance and
committed publication handoff. The implementation developer resolves validation
failures. Actual producer attachment and frontend convergence require a fresh
allocated owner window; synthetic checks do not certify that journey.

Completion requires private root/token/dev-inode and current retained child
checks; exact candidate/environment/event/window/two-principal binding; fixed
expiry and request deadlines; no duplicate stop on retries/polls; natural 5.1s
owner lease expiry; observed replacement generation/rooms/memberships; meaningful
negative checks; honest actual attachment evidence; documentation; and committed
handoff through Root. The active persistent BE13/BE14/focused-QA05 goal is reused.
Its tool cannot append objective text while active; no false completion/reset or
second unfinished goal is used. This document records the additional objective.

Original BE13/BE14 task-local criteria are satisfied by developer disposition:
published `9e4ace4dc9e7d8c019c5f149bc048304029b48bd`, exact five validated
postimages from `9f5a8d352`, compiled source `b966f0ff`, 89 passing selected checks,
retained real-Auth runtime51 restart/replay/switch and real PostgreSQL native
revocation evidence. The four pre-existing bridge PostgreSQL cases remain
NOT_RUN on this latest candidate; unchanged paths retain their earlier actual
qualification. Combined QA05, IN04 and device outcomes remain separately open.
See [recovery evidence](world-recovery-completion.md) and
[remote invalidation evidence](world-remote-invalidation.md).

## Exact ownership and provenance

World20f owns only these new files:

- `backend/scripts/mcr/in04_wall_runtime/owner_control.py`
- `backend/scripts/mcr/in04_wall_runtime/control.mjs`
- `backend/scripts/mcr/in04_wall_runtime/test_owner_control.py`
- this document.

Branch `codex/mcr-be-QA05-owner-control`, source base `9e4ace4dc`. Root1fc
retains launcher/observer/runtime integration and real stack, PG and owner window.
Root153 owns transparent WebSocket faults and frontend composition. FE83 owns
the actual App driver. There is no main/king/SL technical approval stage.

Code is newly authored. Runtime dependencies are Python standard library, Node
builtins, and existing repository helpers: `in03_stack_runtime` source/binary
pins, `migration_smoke` owned cluster/ledger/command checks, and
`world_in01_live.RestartPorts`/`dev.runtime.Child`/health probing. These dependencies
are read at base `9e4ace4dc`; the existing restart helper is not overwritten.
Root's observer at `c5b489d99e3d1e480e9b0f97b3f040ae3c487f6a` supplied the compatible
private descriptor pattern, not a claimed running producer. No dependencies,
manifests, lockfiles, frozen source, contract or application-service code change.

## Owner attachment

Root passes actual `stack`, its direct unreaped `concert_child`, actual RAM
`concert_environment`, `listener_names`, `guard`, all seven `pins` and actual
`manifests`, `source_root`, assembled `candidate`, loopback `origin`, canonical
`event_id`/`environment_id`, finite `window_id`, exact two already authenticated
`actor_user_ids`, epoch `expires_at` and `validate_deployment` closure to
`OwnerControls`. The closure checks the actual source/component/environment
closure; a candidate is not a relabelled old executable. Each pin retains its
own binary source revision. Existing principal creation is not repeated.

Instantiate `ControlServer(controls)`. Publish `server.descriptor()` with an
exclusive no-follow private 0600 descriptor inside the already owned 0700 root.
Set `MCR_QA05_BACKEND_DESCRIPTOR` to its absolute path for the Node caller. The
existing owner loop calls `server.handle_request()` and then `controls.tick()`
under its incumbent allocation/guard. Do not add a server thread, backend stack,
browser, PG cluster or broad probe. Root retains cleanup of all registered
children. A replacement is registered before listener/readiness checks, so a
failed replacement is still included in the same stack cleanup. Close the socket
and retire its descriptor in Root's existing cleanup path.

The implementation accepts no discovered PID or descriptor as kill authority.
Only the retained Child/Popen handle can be terminated. Root token, directory
dev/inode, custody, exact seven live child objects, binary cohort and deployment
closure are checked before mutation. Current SQL ledgers, data directory, PG
system ID, room owners and exact-principal membership rows are observed. Tokens,
SQL errors, endpoint diagnostics and application frames are not returned/logged.
The owner does not invent an authenticated identity from a supplied user ID.

## Node interface

`control.mjs` exports `ownerControl`; all imports are Node builtins and work from
verified data-URL bytes. Config uses `releaseRevision`, `environmentId`, `eventId`,
`windowId`, `origin`, `contractHashes.{domain,realtime}` and `members` containing
exactly two `{userId}` values in the agreed order. There is no operator requirement
for QA05. The descriptor also binds those fields and has a fixed expiry, root
and socket identity and private owner token. It is not a live readiness proof.

- `inspect(config)` returns `{state:'OBSERVED', binding, generation, rooms,
  memberships}` from the actual owner. It refuses while a fault is active.
- `requestWorldRestart(config, {requestId, beforeGeneration})` queues exactly one
  world-only operation against the exact preceding owner inspection.
- `requestPostgresConcertRestart(config, {requestId, beforeGeneration})` is the
  separately named IN04 mode. It is never implied by world-only restart.
- `checkpoint(config, requestId)` returns the retained request state. It neither
  stops a child nor initiates another operation.

`requestId` is 1–128 ASCII letters/digits/colon/underscore/hyphen. FE composition
can use SHA-256(windowId + ':world-restart').slice(0,32). Same ID and identical
action/generation returns the existing checkpoint; conflicting reuse refuses.
At most 16 IDs and one active fault exist per controller. Requests require at
least 65s remaining for World or 165s for PG+Concert, without extending expiry.
The operation deadline is respectively 60s or 160s. A lost mutation reply is an
uncertain outcome; the driver does not blindly issue another request.

`generation` contains actual `systemIdentifier`, `postmasterStartedAt`,
`concertPid`, `concertStartedAt` (bounded OS read for the retained child's PID),
`executableSha256` and `ownerEpochs:[{roomId,ownerEpoch}]`. `rooms` additionally
contains actual `ownerProcessId`/`leaseUntil`; `memberships` includes only
`userId`, `roomId`, `ownerEpoch`, `disconnected`, never session/token contents.

Checkpoints contain `binding`, `requestId`, `action` (`WORLD_RESTART` or
`POSTGRES_CONCERT_RESTART`), `state`, `before` observation, `requestedAtEpoch`,
then applicable stop/replacement/completion timestamps, measured
`naturalOwnerLeaseWaitSeconds`, `after` observation or a fixed failure label.

States are `QUEUED` → `STOPPING` → `LEASE_WAIT` → `REPLACEMENT_STARTING` →
`COMPLETE`; any failure becomes `FAILED` and propagates a fixed owner error for
existing cleanup. Graceful direct termination is polled for at most 12s; this
fault path never forces a kill. The lease wait starts after observed successful
process exit and requires monotonic >=5.1s. No lease or membership is manually
deleted/shortened. World-only completion requires unchanged PG generation,
changed Concert PID, the same room set with new epochs and no old-epoch members.

PG+Concert additionally calls stop/start on the exact retained owned cluster,
checks its unchanged system ID/data directory and all source-bound ledgers before
launching the replacement, and requires a changed postmaster generation. It
never initializes or resets data. Its cluster stop/start is bounded but may
temporarily occupy the owner's loop; the Node checkpoint timeout accommodates
that mode and is always capped by the descriptor's expiry.

FE83's agreed producer-ready phase polls until actual COMPLETE, then performs one
normal UI reconnect. It verifies fresh App streams/epochs/renderer convergence
and obtains current membership observations afterward. No reconnect during
QUEUED/STOPPING/LEASE_WAIT/REPLACEMENT_STARTING. The backend does not manufacture
client snapshots, frames, renderer state, foreground events or convergence.

## Validation plan and current limits

Executed source qualification command (exit 0):

```sh
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/in04_wall_runtime/test_owner_control.py -v
```

Tests cover a synthetic owned stack, foreign/stale/replaced child and principal
bindings, binary/ledger/custody failures, idempotency, detached checkpoints,
natural lease timing, operation/owner expiry, graceful-stop failure/timeout,
replacement cleanup ownership and separately named PG identity changes. The
Node protocol fixture imports actual module bytes via data URL and exercises
private descriptor custody, expiry, inode checks, foreign responses, exact
actions, duplicate JSON keys and bounded socket frames. It creates no product
principals, Rust services or PostgreSQL.

The frozen implementation is `915d45c4790e105574a559ac7e8f0b32fbdfc46c`
(initial implementation `20965b19b`, then exclusive-event guard). All three code
files remain byte-identical to that executed source; later edits only record
documentation/evidence. There are **21 passing synthetic tests, zero failures**,
including the actual Node data-URL/private Unix-socket protocol fixture. This is
one set of 21 cases executed twice, not 42 independent cases or actual service
qualification. All 129 source/helper/controller input hashes were checked before
and after both runs. `git diff --check` passed for the exact new four-path patch.

The first SOURCE round accepted at 2026-11-04T20:24:15.909761Z and fully released
at 20:24:17.341364Z. Tests passed; its independent live resource watcher passed.
The primary enforced admission, deadlines, pins and cleanup but did not separately
sample live resources. The original overly broad "both guards passed" statement
was explicitly corrected before sealing this handoff. Its logs and FULL receipt
are retained at `/private/tmp/mcr-qa05-owner-control-source-20261105`; no spent
offer or clock was reopened.

A distinct corrected SOURCE offer accepted at **20:28:42.965162Z** and fully
released at **20:28:44.350168Z**. Both primary and independent controllers sampled
live new/whole bytes, all descendant processes including each other and Node,
RSS, free space and deadlines. Both report failure `null`. The command exited 0;
test execution reported 21 passes in 0.157s, 1.348s including guarded supervision.
The offer allowed 90s work/150s FULL, 128 MiB total new growth, 12 GiB whole,
8 all-processes and 512 MiB RSS, with fresh 11 GiB start, after-all-reservations
9 GiB and continuous 8 GiB floors. Fresh free space was 17,365,274,624 bytes;
after the permitted conservative OTHER 4 GiB plus this 128 MiB reservation,
12,936,089,600 bytes remained. Actual other-position census and ordinary prep
were recorded once; future A workflows were not summed as simultaneous jobs.

Observed maxima across both controllers: 65,536 new bytes, 9,944,113,152 whole
bytes, 4 processes and 88,145,920 RSS bytes; minimum free space 16,744,398,848 bytes.
All owned groups and the watcher were reaped, no PG existed and no target was
accessed. Literal FULL went directly to next BE38 SQL owner, allocator21d and385
before this documentation. There is no current resource hold.

Corrected evidence root:
`/private/tmp/mcr-qa05-owner-control-source-correction-20261105`.

| Artifact | SHA-256 |
| --- | --- |
| `full-release.json` | `690381c19f1bbb6cb7c7e8cf08114c866757fa44b5a6801feb38fd63267cac15` |
| `source-pins.json` | `697a40f21c9aca6faec06d24d9620b50b9ec0bd8e442c36983ee49cde571dc55` |
| `plan.json` | `302fc3020223f16cfd1c043f7186ea2cbd43cb2f5a5dcb89ca0b223b220b313a` |
| `supervisor.py` | `7b8ac8829fb24e695c90a496d7d8f8edfedeeab9016fb5323feab9a1edaa093f` |
| `guard.py` | `2ab11c1522cf049c6c97b66e514723b3bddb51d9b47828ceeccd07a232207a96` |

Retain both attempts and their original fidelity. Future carry is at least
9,944,113,152 bytes or a higher fresh/prior maximum. Actual managed attachment,
real process/PG restart and App/Auth convergence remain NOT_RUN for this control.
Root owns launcher/descriptor/window composition; implementation owners resolve
any actual faults directly and World20f fixes its assigned control if needed.
No standalone duplicate fault driver is added. Publication of this source
component does not close the original combined QA05 or IN04 flow. The persistent
goal remains active while those required assigned outcomes are unfinished.

## Managed attachment continuation

QA05-MANAGED-ATTACHMENT-01 is the next concrete objective in the same active goal:
wire the published controls and read-only observer to the actual incumbent
seven-service launcher, preserve default IN03 behavior, use Root's real managed
Enrollment/role/checkpoint profile, and deliver validated lifecycle/cleanup,
committed source and actual allocated producer attachment evidence. No new stack,
Auth pair, database or browser is created by this attachment. Actual App recovery
and optional PG+Concert results remain open until the real owner window executes.

Root1fc explicitly stopped and transferred only `in03_stack_runtime.py::run`
managed lifecycle/loop/cleanup hunks, its optional managed flag/call signature,
matching new cases in `test_in03_stack_runtime.py`, new
`in04_wall_runtime/{attachment.py,test_attachment.py}` and this document appendix.
The launcher postimage before editing is exactly Root
`9022e739a77842c0a1ee0ba66753087aef73d13e`. Root retains new
`in04_wall_runtime/launch.py`, actual profile adapters, signed role/checkpoint
production, deployment closure and real owner allocation. Root's frozen build
checkout/readset is untouched. Latest human no-review/no-acknowledgment policy
overrides obsolete instructions in historical checkouts; no technical approval
stage is introduced. Implementation developers own fixes and relevant checks.

The four exact Root9022 dependencies `__init__.py`, `observer.py`, `runtime.mjs`
and `test-runtime.mjs` were copied read-only into the isolated World worktree for
compatibility. They are not authored changes or part of this five-path handoff.
Observer SHA-256 is `ed219d1997d92bc5c27eb0d0370e47fd102861bd7b1fc015641e55f103bc1052`;
runtime SHA-256 remains `7d29fee99b5eab4b3e3320ba5a4513f803d6d2b450da33246fa467767deba6ad`.
The published control files remain unchanged; their 21 source checks are reused.

### Frozen profile ABI

Root and World agreed `main(argv=None, managed=None)` and `run(args, managed=None)`.
The optional CLI flag is `--in04-managed-inputs` with an absolute private Path.
An actual injected profile and that flag must both exist, or both be absent.
There is no profile discovery or fake fallback. Root's launch entrypoint injects
its actual profile. Missing/incomplete profiles refuse before native work.

All hooks are synchronous keyword-only calls:

- `before_start(args,stack,guard,pins,manifests,context,world,prep,dev,patches)`
  installs actual MailSink/Enrollment adapters in the existing startup ExitStack
  before Auth starts. That patch context ends after stack startup; it does not
  become a second producer or independent resource lifetime.
- `configure_environment(envs,concert,context,cluster,ports,args)` runs after the
  actual event/rooms/release/digest/context are set and before child listeners.
  The normal mode and mTLS checks run again after configuration.
- `principal(stack,browser,ordinal,deadline,context,args)` returns exactly
  `(user_id:str, credentials_path:Path, access_token_in_RAM:str)` from the actual
  supplier. The normal three-principal loop is reused; no extra cohort is added.
- `auth_provenance(stack,browser,report,principals,event,revision,auth_source,
  expires_at,world,args,context)` calls Root's actual Enrollment producer and
  preserves genuine HTTP dispositions; it does not apply the legacy all-200
  signup assumption.
- `after_principals(stack,browser,guard,pins,manifests,context,principals,
  source_root,candidate,origin,expires_at,args)` produces actual roles/checkpoint
  and returns exactly `environment_id`, `window_id`, `restore_snapshot_id`,
  `expires_at`, `validate_deployment`, `actor_sessions`. The three session tuples
  must equal the actual supplied principals; first two are App members, third is
  the actual signed operator. The callback returns no claimed readiness proof.

The attachment performs actual public-role/snapshot/current-source/child/ledger
observations before publishing two exclusive 0600 descriptors in the existing
owned 0700 root. Private handoff adds `managedRuntime` with version,
environmentId, windowId, restoreSnapshotId, expiresAtEpoch, backendDescriptor and
qa05ControlDescriptor. Root maps the last two paths to
`MCR_IN04_BACKEND_DESCRIPTOR` and `MCR_QA05_BACKEND_DESCRIPTOR`. Canonical access
tokens never enter those files. The launcher refuses token material in private
handoff data and writes only a fixed failure report if a profile accidentally
places a bearer in report metadata.

The existing guarded loop services the control socket, advances the actual
owner tick and services the observer. Only a currently active owner fault skips
full readiness; direct children, peer identity, binary/source and resource checks
remain active. Read-only observation refuses during the fault rather than
claiming health. Full seven-service health resumes after completion. A registered
replacement's actual RAM environment is followed before the profile's deployment
closure checks it. No observer exports or control implementation are changed.

Cleanup closes both server handles, retires only exact owned descriptor/socket
inodes and still runs the original stack cleanup after an attachment failure.
Replaced foreign files are never unlinked. Missing/replaced custody is a cleanup
failure, not an assumed success.

### Attachment validation status

Source validation PASS: 14 dedicated attachment cases and all 25 changed-launcher
cases (19 retained, 6 new managed entry cases). They cover actual
object routing, canonical-session and expiry binding, real-observation refusal,
token-free private files, fault-aware health, current-env tracking and safe partial
cleanup, plus explicit managed activation and unchanged default entry behavior.
They use synthetic stack/profile fixtures and private Unix sockets; no product
Auth, Rust process or PG is started. Both commands exited 0 under Python 3.14,
with bytecode disabled and ResourceWarning treated as an error:

```text
/Library/Frameworks/Python.framework/Versions/3.14/bin/python3 -B -W error::ResourceWarning backend/scripts/mcr/in04_wall_runtime/test_attachment.py -v
/Library/Frameworks/Python.framework/Versions/3.14/bin/python3 -B -W error::ResourceWarning backend/scripts/mcr/test_in03_stack_runtime.py -v
```

The source under test is commit
`75809d0c64b0a602f93d7c837495891a1873f1cb`, parent
`fe7310cdb926a36abf93cf5dd1fe7e04960cfdf2`. All 135 source/helper/controller
pins matched before and after execution. No old control test matrix was rerun.

The finite SOURCE offer `QA05-MANAGED-ATTACHMENT-SOURCE-20261104T2110Z`
was accepted at epoch `1793825715.157002` (2026-11-04 20:55:15.157002 UTC)
and reached literal FULL at `1793825716.56196` (20:55:16.561960 UTC).
Evidence is retained at `/private/tmp/mcr-world-managed-attach-20261105`:
`check-0.log`, `check-1.log`, `source-pins.json`, `guard-receipt.json` and
`full-release.json`. The FULL receipt SHA-256 is
`324306793455b86bf9fb8b1ba33823fccbb753292f9d1b9ad3a7087172c02f11`.
The primary 50ms controller and independent controller both reported no failure;
all owned groups and the independent controller were reaped, no owned PG
remained, and target access was NONE. Combined command duration was
1.366187708 seconds. Observed peaks were NEW 217088 bytes, WHOLE 9946427392
bytes, five processes and 87326720 RSS bytes; minimum free was 16725938176
bytes. No workflow, browser, service, database or target allocation remains.
The offer is spent. Future accounting preserves at least the retained WHOLE
peak plus subsequent growth; any further execution requires a fresh offer.

An interrupted controlled restart retains normal conservative cleanup semantics:
every registered child is stopped, but a fault interrupted before replacement
does not become a successful runtime result. No cleanup behavior was weakened.

Publication is a five-path handoff to Root1fc, without king/SL review or
acknowledgment. Root's coherent helper-source commit must include the four
read-only Root dependencies, this attachment and Root's actual launch/adapters:
the launcher pins the complete Python helper inventory, so this isolated source
commit alone is not a complete managed helper source. Root9022 remains the
retained per-binary source identity; a helper-only change does not claim a new
Rust executable or require an otherwise unnecessary Rust rebuild.

Actual Enrollment/role/checkpoint provider attachment, managed same-stack faults,
PG restart and App convergence remain NOT_RUN. Root's real profile must be
present and valid; source construction does not certify any of those outcomes.

## Optional owner-profile lifecycle continuation

After the qualified 39-case handoff was frozen, Root1fc explicitly assigned a
separate three-path continuation: two optional hook call sites in
`in03_stack_runtime.py::run`, focused lifecycle cases in
`test_in03_stack_runtime.py`, and this documentation. The existing active goal
continues; completion requires implemented hooks, relevant developer validation
and fixes, documented provenance, committed handoff and Root publication. The
previous qualified source commit and receipts remain unchanged.

An actual managed profile may expose callable keyword-only hooks:

- `attachment_ready(attachment, context, args)` runs immediately after successful
  attachment construction, before the private ready handoff is published.
- `poll(attachment, context, args)` runs after the attachment's guarded poll and
  before the attachment's health check on each existing owner-loop iteration.

Both receive the actual existing attachment, shared RAM context and argument
objects. Absent or noncallable optional attributes leave the previous behavior
intact. Exceptions remain within the launcher's existing failure/cleanup owner:
ready-hook failure cannot publish readiness; poll-hook failure cannot claim
post-hook health, and both attachment and stack cleanup are attempted.

Root retains `observer.py`, `runtime.mjs`, `launch.py` and the actual profile.
Its proposed read-only `restart-baseline` notification records a validated
baseline only. Root's separately prearmed profile owns any actual restart queue
through the guarded owner loop. These two hooks introduce no new public port,
CLI flag, private profile schema, observer restart authority, timer, stack or
cohort. This continuation does not implement or certify Root's notification or
restart decision.

Five focused cases exercise the actual `run` control flow using synthetic
service boundaries: exact hook object identity and ordering, missing optional
hooks, noncallable attributes, ready failure before publication, and poll failure
before health. They start no native helpers, product services, Auth, PG or
browser. These cases were NOT_RUN at preparation; their subsequent fresh
qualification is recorded below. The prior 39-case matrix is reused rather than
repeated without cause.

### Optional lifecycle qualification and handoff

The five new `ManagedProfileLifecycle` cases passed on source commit
`fa95f0dbad1524f6408d511c4616628f26272378`, parent
`d4300acb5af2104b171dcdf78f2736e02af660e0`. The single command exited 0:

```text
/Library/Frameworks/Python.framework/Versions/3.14/bin/python3 -B -W error::ResourceWarning backend/scripts/mcr/test_in03_stack_runtime.py ManagedProfileLifecycle -v
```

The test log reports five cases in 0.048 seconds. The unchanged attachment/control
source matrices were not rerun. All 135 source/helper/controller pins matched
before and after execution; Root's four read-only dependencies remain exact
Root9022 copies. No product service, real Auth, PG, browser or Rust process ran.

Fresh offer `QA05-MANAGED-HOOKS-SOURCE-20261104T2140Z` followed FE37's actual
FULL at 2026-11-04 21:04:25.723637 UTC. Its release and outer-release receipts
from source `77ca88b042a0dba936c7d41f0693fcf480228b7d` showed all owned resources
released; the copied outer receipt SHA-256 is
`bd60dcb3ffaa3fcbe17c5662dfc21093fe6a03b256ead8859ceccedd4cd87b7a`.
This World round was accepted at epoch `1793826571.2151418` (2026-11-05
06:09:31.215142 KST) and reached literal FULL at `1793826580.285345`
(06:09:40.285345 KST). Evidence is retained at
`/private/tmp/mcr-world-managed-hooks-20261105`, including `accepted.json`,
`check-0.log`, `source-pins.json`, `guard-receipt.json` and `full-release.json`.
The FULL receipt SHA-256 is
`a65e391b0e7e2d1baec5afb29fe5af277fe8233c240282b3f6f31a5da56427ef`.

The primary controller and independent controller both reported no failure.
Combined supervisor duration was 0.782804792 seconds; observed maxima were NEW
65536 bytes, WHOLE 9950687232 bytes, four processes and 84574208 RSS bytes.
Minimum free during execution was 14010351616 bytes; admission free was
13318537216 bytes, with 11036835840 bytes remaining after all reserved NEW.
All owned groups and the independent controller were reaped, no owned PG
remained and target access was NONE. Literal FULL was sent to allocator21d and
the named FE83 successor before documentation. No reservation or live workflow
remains; this offer is spent. Future accounting retains at least this WHOLE peak
plus subsequent growth.

The original attachment is already published: publisher412e verified the exact
five-path mapping `d4300acb5af2104b171dcdf78f2736e02af660e0` to
`bf839af5dd4c6bf7927374c40fa87a9afde1df17`, on master
`5d962c03a03ec559d304eb5ea87ee301a18d5ce9`, with fresh fetch/remote agreement,
ancestry and path postimages. Its receipt is
`/private/tmp/mcr-publisher-412e-attachment-register-receipt.json`. Root's
unqualified observer/profile/launch dependencies were excluded. This optional
hook follow-up is a separate three-path handoff to the same named publishing
developer; no king/SL review or acknowledgment gate applies.

Actual managed profile startup, baseline notification, owner-queued restart,
canonical-session recovery and App convergence remain NOT_RUN. Root owns those
real profile/window outcomes, with the relevant implementation counterparts.
Passing synthetic lifecycle cases does not close those requirements.
