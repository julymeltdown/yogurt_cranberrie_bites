# World admission producer

State: **IN_PROGRESS** pending delivery integration. This is the additive BE10
rooms/ticket producer at integration base
`cc3bb84046cc96388c4e34186b4eae399ec02642`. The focused source and native gates
pass. No runtime, public two-account, load or release acceptance is claimed.

The producer exposes an internal `concert.world.v1.WorldService`: `ListRooms` and
`IssueTicket` return the frozen C05 `WorldRooms` and `WorldTicketResponse` JSON;
`CurrentMembership` returns the exact persisted user/event/session/connection/
owner/room/map/avatar lease. It deliberately contains no runtime stream or epoch.

The Gateway route must use strict Concert access and fresh current Auth/Profile
reads before signed dispatch. Concert must combine the verified Gateway request
with a current event, participant, restriction and validated release check. A
missing owner, release pin or CSPRNG is a failure; there is no positive fallback.
The internal ticket remains hash-only at rest and is returned only in the one C05
response. Public HTTP/WebSocket ingress, runtime handoff, simulation, privacy and
two-account execution remain outside this bounded producer until separately
qualified.

Registration changes are intentionally isolated for integration review:

- `backend/proto/concert/v1/world.proto`
- `backend/crates/contracts-build/src/lib.rs`
- Concert and Gateway `build.rs`
- Concert `lib.rs`, `main.rs` and `Cargo.toml`
- Gateway HTTP router/response-middleware and `AppState` construction

## Evidence

Environment: Rust `1.88.0`, locked/offline, two jobs, development and test
debuginfo disabled. The shared target is `/Users/lhs/Desktop/mcr-build/backend`.
The final source before documentation is
`f7451692f4270984ab75fb37a9a589fa3fda6bc7` plus the fix and test paths listed in
the delivery handoff; the receipt's `source_revision` is the last committed
producer revision and its source digests bind the actual native inputs.

- Focused Gateway transport and HTTP composition: `cargo test -p api-gateway
  --test concert_identity_composition -- --test-threads=1`, exit 0, **21/21
  passed**. This covers signed rooms/ticket dispatch, exact Access expiry,
  cross-event rejection, no replay/cache, browser-origin/intent rejection before
  owner reads, no-store errors, and the actual public rooms GET route through
  strict Access, fresh Auth/Profile, signed Concert dispatch and response checks.
- Concert world authority: `cargo test -p concert-service --test
  world_rpc_authority -- --test-threads=1`, exit 0, **4/4 passed**. This invokes
  the actual `WorldRpcService` handler; `ListRooms` runs canonical room authority
  before catalog access and denial has no catalog fallback.
- Native gate: exact built `world_admission` test binary
  `/Users/lhs/Desktop/mcr-build/backend/debug/deps/world_admission-9fddb6775dd23919`,
  SHA-256 `3ba99b68a35a8d714cbad596a69276ca866f5962855fd19310650cfdc5b66b27`.
  `backend/scripts/mcr/world_admission_smoke.py` exit 0 against fresh owned
  PostgreSQL **16.14 (Homebrew)**. Result **PASS**, native tests exit 0 (4/4),
  direct migrations and schema constraints PASS, restart persistence PASS,
  direct migration reapply `EXPECTED_REFUSED`, cleanup PASS, and captured
  password/URL/token leakage checks PASS. Receipt:
  `/private/tmp/mcr-world-producer-native-1793732868610.json`, SHA-256
  `f25ca6a36a5837ce955de85a2d0341b6f5aaa7fe3128e0dbb7c60f0034ddea71`.
  Native test output SHA-256:
  `238b87d2932524cd96911a16b5fa3350962f7a2ebd134801cafee83c81ed2736`.
- `rustfmt --check` on every changed/new producer Rust path: exit 0.
  `git diff --check`: exit 0. `python3 scripts/mcr/check-plan.py`: PASS, exit 0.
  Harness guards: `python3 -B -m unittest test_world_admission_smoke.py -v` from
  `backend/scripts/mcr`, 5/5 PASS, exit 0.

The full Gateway library suite reached 123 passed and two failed. Both failures
need Docker's `/var/run/docker.sock`, which is unavailable; they test external
schema damage and quotas and are unrelated to C05. Docker was not started.

## Limits

The public rooms and ticket HTTP routes are now exercised through the composed
Gateway transport test with loopback signed RPCs. This is not a live deployed
Gateway/Concert/native-PostgreSQL journey. No public WebSocket handshake,
two-account runtime, `CONNECTION_REPLACED` delivery, live capacity, load or
release acceptance was run. BE12 movement/simulation and BE13 privacy projection
remain outside this producer.
