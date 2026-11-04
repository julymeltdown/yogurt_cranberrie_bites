# QA05 owned fault control

Status: IN_PROGRESS. Source authored; source protocol tests and actual managed
attachment are NOT_RUN. This is the remaining original QA05 combined App/Auth/
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

Source qualification command, once an actual bounded SOURCE offer is accepted:

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

SOURCE and actual attachment results remain NOT_RUN until explicitly recorded.
Current resource state: no target, compile, native, browser, PG or live control
window. A future source offer is requested; retained evidence/copies count toward
whole-workflow highwater, at least 9,876,881,408 bytes or a higher fresh measured
maximum. Actual attachment requires Root's real single-stack allocation and
developer-owned validation/fixes. No standalone duplicate fault driver is added.
