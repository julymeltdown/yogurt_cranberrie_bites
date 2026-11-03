# BE12 authoritative world simulation

Status: **IN_PROGRESS**. Source and deterministic regressions are implemented;
Rust formatting and planning checks have executed. Cargo execution is queued
with the delivery owner of the shared target; no compiled/test PASS is claimed
at this checkpoint. IN01, real multiuser movement, database admission composition,
WebSocket delivery, device and load qualification are **NOT_RUN** by this patch.

## Ownership and ancestry

Worker branch: `codex/mcr-be-BE12`; isolated checkout:
`/Users/lhs/Desktop/mcr-worktrees/be-world-simulation`.
Assignment base: `8bb878c979138d0f6b3a4b880edb394f41cf99be`.
The BE10/BE11 owner explicitly excluded this carve-out from its writes.

New implementation/test/document paths:

- `backend/crates/concert-domain/src/world/mod.rs`
- `backend/services/concert-service/src/world/simulation.rs`
- `backend/services/concert-service/tests/world_simulation.rs`
- `docs/mcr/backend/world-simulation.md`

Delivery explicitly delegated a separate registration-only commit for
`backend/crates/concert-domain/src/lib.rs` (`pub mod world;`),
`backend/services/concert-service/src/world/mod.rs` (`pub mod simulation;`),
and `backend/services/concert-service/src/lib.rs` (`pub mod world;` when absent).
Delivery composes admission exports when integrating these exact registrations.
No manifests, lockfiles, router, migrations, contracts, frontend, selected art or
`docs/source/` bytes are changed. Only delivery pushes master. Commit IDs and
final command results are sent directly to delivery with the exact path list;
remote delivery remains unverified until delivery reports the published ancestry.

## Behavior

`Input` decodes the frozen C05 shape with closed fields, exact protocol/type,
canonical UUID strings, safe positive sequence, finite axes in [-1,1], vector
length at most 1 + 1e-9, and zero/one `interact` button. Rounding-only excess length
is clamped to one. The type exposes no client coordinate, speed, delta-time or
identity field. Unknown fields and repeated typed fields are rejected by Serde.
The listener must still enforce the assembled 4096-byte envelope, nesting,
UTF-8/binary rules and all-message rate limit before decoding; this module is
not the listener.

The room owner calls `advance` on a monotonic server clock. Logical ticks are
50 ms (20 Hz); input receipt never advances coordinates. One latest pending
intent per player bounds memory and coalesces packets. Strictly increasing input
sequence allows gaps, rejects duplicates/decreases and resets only on a fresh
stream. `ack_input_seq` advances only when a pending input is consumed by a tick;
held motion does not invent acknowledgements. Inputs received after a tick's
time cannot be backdated into it. At most four steps run per call; excess wall
time is discarded and reported as `dropped_steps`, with server ticks advancing
over those omitted steps. The process cannot accumulate an unbounded catch-up
loop or grant extra speed for a packet flood.

`Limits::new` requires explicit server values: positive speed up to 1000 virtual
pixels/second, interaction range 0..256 virtual pixels, input hold 50..1000 ms.
These hard ceilings bound collision work; they are implementation limits, not
measured tuning/capacity claims or changes to C05/C06. Tests select speed 120,
range 48, hold 250 ms. Analog magnitudes below one retain proportional speed;
diagonal movement cannot exceed the configured scalar speed. Zero axes stop;
expired held input stops. Expired unsimulated input is discarded unacknowledged.

A per-player token bucket enforces C05 input 30/s and burst 60 with integer
milli-token accounting. Replays consume rate credit too. An in-place stream
replacement preserves the bucket rather than granting another burst. The listener
still owns the separate 40/s burst 80 all-message bucket and reconnect/global
admission rate limits. A new connection does not authorize any input before
admission.

`WorldMap` copies immutable C06 row-major `0=walkable/1=blocked` geometry after
rechecking dimensions/cell ceiling (65536, matching BE08), virtual wire bounds,
finite runtime values, unique asset IDs, named/default spawns, walkable interaction
cells and connected spawn/interaction components. Runtime interaction count is
capped at 256. A single flood fill runs only when loading geometry. Production
composition must first validate the exact immutable BE08 release, content/geometry
hashes and resource pins. This constructor does not establish art rights, physical
venue truth or production resource readiness.

Server-selected named spawn uses the C06 cell-centered foot coordinate. Collision
uses a swept point-foot segment, not the visible sprite rectangle: all crossed
cells are checked with supercover grid traversal, including both neighbors at a
diagonal corner and the adjacent cell when traveling exactly along a grid edge.
Outside cells are blocked. A blocked segment rejects the whole step (no wall
slide); a clear endpoint beyond a wall cannot tunnel through it. Traversal fails
closed at 512 visited cells, covering both a 50-pixel maximum movement step
and a 256-pixel interaction segment even with 1-pixel tiles.

`interaction_in_range` requires a known interaction, server foot coordinate,
configured Euclidean virtual range and clear segment. A consumed one-shot intent
selects the first qualifying interaction by stable ID order. It emits at most once,
not on subsequent held-input ticks. Latest-intent coalescing can replace an
unsimulated intent before consumption; this is not a durable command channel.
These candidates only open virtual interactions. They never change physical
arrival, stock, party membership or pickup state, and they do not authorize a
player-to-player operation. Durable operations and social-policy checks remain
with their owning HTTP/application boundaries.

## Membership integration boundary

`MembershipPort` is a typed synchronous view of current authoritative membership
owned by the single room process. The production adapter must serialize calls
with admission, takeover, switch, disconnect and owner fencing; it must not issue
blocking network/database queries inside each simulation tick. It returns a
complete binding and independently validated deadlines, or fails closed on
revoked/unavailable/unknown state. It is not a test-default allow policy.

Binding includes canonical `(event,user)`, session, non-nil connection and owner,
room, room epoch, stream ID, and exact map/avatar IDs and versions. A lease may be
at most 5000 ms ahead and both lease/access deadlines must be strictly later than
now. The access deadline is captured once at admission and cannot change through
input, heartbeat, lease renewal or same-connection resync. Renewing policy authority
requires the live Auth/Concert check owned by BE10/BE11, not client activity.
The adapter must map authoritative UTC deadlines conservatively onto the same
monotonic millisecond origin used here, without extending expiry on conversion.

Every accepted input and every `advance` validates authority at current server
time; even an advance with no tick removes expired/revoked players. During one
serialized advance, the same current authority fence covers its bounded tick
batch. Before publishing any output, BE13 must apply current recipient policy
and revoke subscriptions; removed simulation state alone is not a socket close.
The synchronous port must not change authority mid-batch outside the owner lock.

A stream replacement requires a freshly authorized binding and distinct stream
ID, preserves the server coordinate and clears pending/held input and ack. A
new authorized connection takeover can capture its new access expiry; old
connection input and disconnect cannot mutate/remove the replacement. Failed
replacement leaves old simulation state for the authoritative owner to revoke
on its next check. Room/owner/epoch/resource changes reject stale bindings; create
a new owner simulation for an epoch reset and atomically replace streams, rather
than translating old coordinates into changed geometry. Room switching needs the
BE11 owner transaction; callers must not leave two simulation owners active.

The configured room capacity cannot exceed 50, one player per canonical user in
this event/room. The pure owner API additionally rejects duplicate admissions;
only committed takeover/resync may replace a stream. `state` is an owner-local
read for BE13 projection, not an authorized public roster or presence endpoint.

## Provenance and dependencies

All three new Rust files and this document are newly authored for MCR BE12.
No Bigyeting/port-rust movement, rendering, UI, art or game library is copied.
The existing BE08 `Geometry`, source/canonical-release parser and canonical
Concert identity/asset/revision types are reused from the assigned base.
No dependency or lock resolution changes are needed: existing `serde`,
`serde_json` (tests), `uuid`, `thiserror` and the Rust standard library suffice.
No migration or database write is introduced.

Specification source: `MCR_IMPLEMENTATION_PLAN_V2_EN.md` §§5.4, 7.5 and BE12,
revision `f5b97aca2b002484fc4cbdd7581c723392df10c6`, SHA-256
`e19732ac9c5c5d5397652bf047645da0640ec72c97f59869974680c4e7f69a36`.

Frozen C05 v1.0.0 at `69a5624cd153a216f2398cebabf062400dce23eb`:

- `contracts/realtime/v1/world.schema.json`: SHA-256 `1732e627037fe91533ac6f7fe74e36683ce9b846bfbfc020b3cce298ae84df36`
- `contracts/realtime/v1/policy.json`: SHA-256 `3e77e21b036386803c7d5bff0145c434f111ba51b861219a211e853a43d0628b`

C06 metadata v1 at `12acc2d7a93701d89aad63ea7c3376f91b7fcb13`:

- `contracts/assets/v1/map-manifest.schema.json`: SHA-256 `ed01d26b2f561c9885ad3fc5fe66912ee8175334393e49a85652756e301d6800`
- `contracts/fixtures/assets/map-manifest.json`: SHA-256 `0cccde2c009a44cc2355dfa15ef2d030bc7b85be6f58bec7470108dba46562f7`

The fixture regression canonicalizes those existing synthetic metadata bytes
before using BE08's strict stored-release parser. It makes no production-art,
physical-place or actual Tiled resource claim.

## Validation record

Executed from the assigned checkout on 2026-11-04 (Asia/Seoul):

| Command | Exit/result |
| --- | --- |
| `python3 scripts/mcr/check-plan.py` | 0; preserved archive hashes, plan graph/export regressions and delivery destinations pass; application tests NOT_RUN by that checker |
| `rustfmt +1.88.0 --edition 2021 --check backend/crates/concert-domain/src/world/mod.rs backend/services/concert-service/src/world/simulation.rs backend/services/concert-service/tests/world_simulation.rs` | 0 |
| `git diff --check` | 0 |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -p concert-service --test world_simulation` | NOT_RUN; queued explicit shared compile lease with delivery |

Twenty deterministic test cases cover strict intent decoding/teleport attempts,
invalid axes and safe counters, scalar/diagonal speed, 50ms timing/ack, packet
flood/rate refill, input gaps/replays, stream/epoch/session/connection/owner/pin
fencing, takeover reset, access-expiry immutability and bucket preservation,
revocation/expiry/unavailability without a due tick, held stop/timeout, catch-up
bounds/non-backdating, high-speed wall tunneling, corner/edge/bounds collision,
spawn/malformed geometry, interaction distance/occlusion/unknown IDs, one-shot
behavior, fifty-player bounded workload and the frozen synthetic fixture.
These are authored tests, not executed results until the queued Cargo command
completes. No native admission, two-account world, actual WebSocket, mobile,
200-connection load, QA05/QA08 or IN01 acceptance is asserted.
