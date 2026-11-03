# World admission producer

State: **IN_PROGRESS**. This is the additive BE10 rooms/ticket producer at
integration base `cc3bb84046cc96388c4e34186b4eae399ec02642`. Source is currently
uncompiled. No runtime, public two-account, load or release acceptance is claimed.

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

No evidence is claimed before the focused compile and native tests pass.
