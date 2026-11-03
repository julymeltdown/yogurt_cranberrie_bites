# BE13/BE14 recipient replication and recovery

Status: **IN_PROGRESS**. The bounded owner components and deterministic tests
are authored; native execution is **NOT_RUN** pending the explicitly queued
shared Cargo target lease. No public HTTP/WebSocket listener or qualified
production policy adapter is supplied by this patch. IN01 is **NOT_RUN**.

## Scope and ancestry

Exclusive checkout: `/Users/lhs/.codex/worktrees/1152/mcr`.
Branch: `codex/mcr-be-BE13-14`.
Assignment base: `5c95ed4ddb243853b27fb625f726341b9771dadd`.
The exclusive feature paths are the four service `world/` files `interest.rs`,
`replication.rs`, `session.rs`, `recovery.rs`, integration tests
`tests/world_replication.rs`, `tests/world_recovery.rs`, and this document.
An exact delivery grant permits a separate registration-only commit adding
those four module exports to the existing service `world/mod.rs`. Other world
modules, particularly admission/simulation/mural, retain their named owners.

Authorized BE12 prerequisites were cherry-picked serially, without source edits:

| Source commit | Isolated local mapping | Purpose |
| --- | --- | --- |
| `1ea1813` | `c2522f9` | BE12 domain/service/tests/evidence |
| `8f649a6` | `3f392da` | Domain/service module registration |
| `398bbfd` | `aefd9c7` | Interaction traversal bound |
| `4187219` | `7583564` | Stream reuse fence/history |
| `988ddb3` | `ab4b78e` | Bounded movement history/one-shot/collision corrections |
| `40b3192` | `b9df641` | Full-binding simulation provenance/actual tick |

Delivery must skip duplicated prerequisite commits and integrate only the
feature and registration commits after its equivalent BE12 ancestry. The BE12
owner separately reports 26 native tests passing at source `988ddb3`; that is
source-owner evidence, not execution of this BE13/14 candidate. No BE12 source
is copied from uncommitted checkouts. `docs/source/` is unchanged.

All new code is authored for MCR, without copied Bigyeting UI, rendering,
legacy art or third-party game logic. Existing dependencies suffice: canonical
Concert types, typed BE12 owner API, `chrono`, `serde_json`, `uuid`, and std.
No migrations, package dependencies, lockfiles or contract bytes change.

Binding specification: `MCR_IMPLEMENTATION_PLAN_V2_EN.md` sections 5.4, 7.5,
BE13, BE14, IN01; SHA256
`e19732ac9c5c5d5397652bf047645da0640ec72c97f59869974680c4e7f69a36`.
Frozen C05 v1.0.0 (`69a5624cd153a216f2398cebabf062400dce23eb`):

- `contracts/realtime/v1/world.schema.json`: `1732e627037fe91533ac6f7fe74e36683ce9b846bfbfc020b3cce298ae84df36`
- `contracts/realtime/v1/policy.json`: `3e77e21b036386803c7d5bff0145c434f111ba51b861219a211e853a43d0628b`

C06 schema/identity and actual shared resource pins are unchanged. Test asset IDs
are metadata fixtures only; they do not introduce synthetic production art or
make any source-pack/art/physical-venue claim.

BE37 committed interfaces inspected with `git show`: API `bcd2cd2`, repository
implementation `b71ae36`, evidence pin `a5e84da`. No unrelated BE37 implementation,
exports, migration or lock changes are imported. Their production fenced
adapter is unqualified, so this patch uses an explicitly unavailable local
`RecipientPolicy` port. The eventual adapter requests `Field::Presence`,
`actor=receiving canonical viewer`, `subject=projected canonical player`,
with exact event/query, freshly resolved current revision stamps and deadlines.
`PolicyDecision::ensure_current` cannot be fed its old stamp as fresh authority.
Fresh remote reads alone do not constitute a cross-service transaction fence.
An unfenced owner observation vector cannot be cast/hashed/packed into a scalar
BE37 authority stamp.

## Projection and bounded publication

`RoomFrame::capture` captures a complete, immutable BE12 owner-local state frame
with the supplied same-lock roster through `Simulation::replication_state`. It obtains the actual simulation tick and validates each full stored binding rather than accepting a caller tick or user-only state lookup. It checks room/owner/epoch/resource scope,
capacity, complete/unique players, safe tick/ack/coordinates, bounded public
display names and avatar manifest versions. The only identity output is canonical
user ID, display name and approved public avatar selection/revision, with virtual
position and facing/moving presentation. No profile secrets, claims, handles,
credential material or physical arrival fields are projected. Production must
validate selected avatar catalog membership through BE07 before capture.

`RecipientPolicy` returns a fresh exact-binding view, including self, with a
strictly current monotonic check and deadline no further than 5 seconds away.
Absent/unknown/stale self or policy outage fails closed. The adapter must reject
both-direction local/inherited blocks, missing membership, suspension, revocation
and event restrictions. A local `invalidation_id` represents the room owner's
serialized privacy-generation notification. It is an opaque local UUID, **not** a
cross-service authorization stamp. Every relationship/event/privacy invalidation
must rotate it before further output, including deny-to-allow changes. Even an
unchanged generation requires fresh policy resolution on every send. No cached
allow or generation equality is authority. An implementation that omits those
notifications is not a qualified adapter.

Each visible subject's complete BE12 `MembershipLease` is checked before
projection; mismatched owner/session/connection/stream/event/room/epoch/resources,
expired access/lease, and unavailable authority terminate the recipient stream.
A conservative whole-recipient close on an indeterminate subject avoids emitting
an older roster while subject authority is unavailable.

Initial snapshot is stream sequence zero with self present and input ack zero.
The baseline is installed only on handing bytes to the transport. Deltas compare
against the last delivered baseline, with a fresh per-recipient sequence assigned
after filtering/coalescing. Hidden-only motion causes no sequence increment.
Avatar revisions/config changes become upserts. Removed visible users become
removes. Ticks/acks never regress within a stream. New snapshot/stream is required
for a changed privacy generation; queued private state/baseline is immediately
cleared and the connection closes after best-effort `resyncRequired` (4409).

The outgoing queue intentionally contains at most **one latest-state frame**,
coalesced against the delivered baseline; it is stricter than the C05 ceiling of
64 frames / 262144 bytes. Each serialized frame is <=65536 bytes. Server-only
`QueueLimits` can tighten, never loosen, those caps. Replacement preserves the
original stall timestamp. At 5 seconds (or a tighter configured bound) without
handoff, or a frame/byte overflow, pending state is cleared and a bounded
`resyncRequired: BACKPRESSURE` terminal control closes with 4409. No invisible
sequence skipping or historical replay is implied. Deltas hand off at most 10 Hz;
snapshots and terminal revocations are immediate. Policy is checked even while
rate-throttled. The terminal slot contains only a small control, never private
roster data, and is consumed once.

`next` refreshes again at actual handoff; a queue prepared before block/revoke
cannot bypass current policy. Delivery must call it under the owner/policy
invalidation fence, directly into a transport with bounded writable capacity.
An independent unbounded socket queue would defeat this guarantee. On partial or
failed transport writes, close and obtain a fresh ticket/snapshot; the committed
baseline cannot be safely reused after uncertain delivery. Deadlines and blocked
socket writes need transport-level timers, not waiting for a simulation tick.

## Session and recovery integration

`Session` captures the verified access deadline once (both conservatively mapped
monotonic deadline and UTC wire timestamp). Membership lease renewal, input,
heartbeat, resync or room switching cannot extend it. `check` rejects backwards
clock, expiry, unavailable/revoked authority and a replaced canonical binding.
Explicit reasons cover SESSION_EXPIRED/SESSION_REVOKED/CONNECTION_REPLACED
(4401), SUSPENDED/EVENT_CLOSED (4403), POLICY_UNAVAILABLE (4500).
No credential is accepted or logged by these APIs. Access UTC/monotonic mapping
must originate from the same credential at trusted admission; this component
does not verify a JWT or issue/consume tickets.

`Recovery` holds at most one subscription per canonical event/user (stricter than
one per active shell). Calls run under BE11's serialized committed admission /
switch / takeover fence. `activate` validates fresh authority before touching
simulation, requires a new server-generated stream and appropriate snapshot
cause, and invokes BE12 `replace_stream` for same-owner-room takeover/resync.
This clears held/pending motion and input ack while preserving authoritative
virtual position. Epoch/server owner resets construct a new simulation with a
validated server-selected spawn; no old client motion is replayed.

Same-connection recovery keeps session and exact access deadline immutable,
requires correlated non-nil request IDs, forbids request/stream reuse, bounds
history at 1024 streams, and rate-limits client resync to once per 1000ms.
History resets only after a freshly authorized different connection. New
connections require INITIAL; same-connection controls require RESYNC,
ROOM_SWITCH or a valid unsolicited EPOCH_CHANGED. Room/epoch replacements cannot
silently change the frozen event/resource identity. Resource changes require
close and fresh admission. Closed sessions cannot resync; a fresh connection
still must pass current authority and therefore cannot restore a suspended,
revoked or expired membership.

`activate` returns the replaced binding, allowing the listener to immediately
close the displaced socket. `switch_room` explicitly removes the old simulation
binding after validated destination activation. Other owner/epoch replacement
adapters must remove/fence their displaced simulation and close its transport
using that returned binding; they cannot keep the old owner publishing. Delayed
old disconnect/input compares the complete binding and cannot remove or mutate
the replacement. `Recovery::submit` gates input until a snapshot handoff and
rechecks authority; BE12 then enforces stream/epoch/sequence/movement invariants.
No durable HTTP domain commands are replayed by recovery.

`revoke` clears the subscription and simulation; `sweep` can revoke against live
authority without any room frame/tick and returns exact bindings for their owning
simulation cleanup and socket close. `take_terminal` drains controls without a
frame. `expire` revokes expired slots and routes cleanup through a caller-supplied owner callback. Successful cleanup removes the slot; failed cleanup retains a closed, discoverable slot for deterministic retry, so another room cannot be orphaned. The
integration adapter schedules these promptly and handles async policy
invalidation before publication. No remote query executes in the BE12 tick.

Remaining composition is explicitly owned by delivery/admission/policy:

1. Actual one-use ticket/JWT/session verification and authenticated WS ingress,
   origin/size/depth/rate and heartbeat timers from BE10/BE11.
2. Trusted deadline conversion, same-lock roster/avatar and actual server tick,
   owner/membership serialization, persistent room/epoch fencing and cleanup.
3. Qualified BE37/live authority adapter plus prompt privacy invalidation; the
   shipped default is unavailable, not a permissive synthetic production policy.
4. Transport backpressure/partial-write/deadline/close handling with no second
   unbounded queue; close old socket and remove displaced owner on replacement.
5. Real two-account IN01, approved art, mobile/device and load qualification.

## Validation and limits

Executed in the assigned isolated checkout (2026-11-04 Asia/Seoul):

| Command | Exit/result |
| --- | --- |
| `python3 scripts/mcr/check-plan.py` | 0, planning/archive checks only |
| `python3 scripts/mcr/validate-realtime.py` | 0, 24 contract fixtures, websocket/capacity NOT_RUN |
| `rustfmt +1.88.0 --edition 2021 --check` on exact four owner modules and two tests | 0, exact four modules/two test files |
| `git diff --check` | 0 at source checkpoint e582c6b |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -p concert-service --test world_replication --test world_recovery` | NOT_RUN, shared-target slot queued after BE24 |

Tests use fixed clocks, canonical bindings and deterministic fixtures, with actual
output fed to the frozen repository C05 Python schema/semantic oracle (requires
its existing `jsonschema` environment). Authored cases cover different viewer
block sets/no false gaps, stable snapshot/delta coalescing, avatar revision,
policy/subject expiry/outage, privacy-generation queue withdrawal, frame/byte/
frame-count caps, slow consumer, 10 Hz/ack/removal behavior, stale clock/tick/
epoch/resources, one-shot terminal cleanup, unavailable default, immutable
expiry, fresh-ticket takeover without motion replay, pre-snapshot input denial,
resync/request/stream reuse, revoked/suspended restore denial, expiry without
simulation ticks, restart, room switch, delayed disconnect, capacity and failed
motion replacement. Native results are recorded only after actual execution.

Source/test implementation is not an implemented public listener and does not
accept BE13, BE14, QA05, QA08 or IN01 as a whole. Actual services/database,
two real independent accounts, approved product art, device, load and release
qualification remain **NOT_RUN**. Final commit/remote ancestry is reported to
the delivery integrator; only that chat publishes to master.

Targeted read-only peer review found three P2 defects during source development.
The initial committed feature checkpoint `e582c6b` includes fixes and regressions
for fresh rejoin after simulation removal and multiroom expiry cleanup. The third
is fixed by committed prerequisite `40b3192` and the feature follow-up consuming
its full-binding read/actual tick; regressions reject older simulation frames
relabeled as current owner/room/epoch/connection/stream.
The independent registration-only commit is `66c7fff`. These are worker commits,
not proof of published master ancestry.

The first native `--no-run` invocation exited 101 because the retained shared
Cargo target reused an older `concert-domain` artifact without `world` exports.
Under the exclusive lease, a scoped `cargo +1.88.0 clean --manifest-path
backend/Cargo.toml --locked --offline -p concert-domain -p concert-application`
removed 73.8 MiB of those package artifacts (exit 0). The identical focused
`--no-run` command then compiled the actual domain/application/service and both
test executables at `e582c6b` + `66c7fff` (exit 0, 4.58 seconds). Those are build
results for the earlier source only; tests were not executed. Delivery requested
lease release at that point; it was explicitly released to BE34/35 with no
running Cargo. The provenance source correction is now ready for a later
explicit slot; no compiled result is reused for its changed bytes.

A follow-up peer reread identified a retained-cleanup edge case: replacing a
closed slot after failed expiry cleanup could discard the old owner binding.
The correction retains displaced bindings in an independent cleanup map,
separately bounded by the configured subscription capacity. Activation checks
that budget before destination admission; `retry_cleanup` routes each binding
to its owner and keeps failures discoverable. `expire` also retries this map.
Successful `switch_room` removal acknowledges its entry. Adapters must keep
retrying pending cleanup even after the active slot disconnects. Added regressions
cover failed expiry cleanup -> cross-owner fresh reconnect -> successful retry,
and capacity refusal before mutation when pending owner cleanup is full.

Final narrow peer rereview at committed `e787379` reported no actionable findings
in the displaced-owner cleanup correction. It verified the independent bounded
map, pre-mutation capacity refusal, persistent failure retries and successful
switch acknowledgement by static inspection only; no builds/tests were run by
the reviewer. The four initial/follow-up concrete P2 findings are addressed in
source and authored regressions. Native execution remains a separate obligation.

`python3 scripts/mcr/check-boundaries.py --base ab4b78e --lane backend
--allow-doc docs/mcr/backend/world-replication-recovery.md
--allow-doc docs/mcr/backend/world-simulation.md` exited 0 after final fixes.
Its 11-path comparison includes the separately authorized BE12 provenance
prerequisite; feature commit changed paths were also inspected individually
against the seven-file assignment plus separate four-export registration.
The frozen C05 schema/policy SHA256 values were rechecked and match the pins.

The final cleanup audit also routes a failed `disconnect` removal into the
bounded cleanup map; a wrong-owner simulation cannot discard the last binding.
If that map is full, the closed active slot remains discoverable and disconnect
returns false. Owner callbacks can acknowledge idempotent absence only after
checking the actual owner. Activation additionally rejects reuse of one transport
connection across different canonical event/user subscriptions. Two regressions
cover wrong-owner disconnect and cross-principal/event connection reuse. Native
validation is still queued; these changes are not claimed executed.
