# World runtime and socket delivery

Status: IN_PROGRESS. Native compilation and socket tests are NOT_RUN at this source checkpoint. Production live-owner transport, real two-account Auth flow, reviewed product art, load and IN01 acceptance remain unproven.

## Ownership and ancestry

The resumed sole writer uses `codex/mcr-be-world-runtime-production` in `/Users/lhs/Desktop/mcr-worktrees/be-world-runtime-production`, exact base `e869480395b70b4aae6cfcf50e904b079a59a0e7`. Main preserved six untracked source files from the stopped `fed927b00e05a0b25747d628b6c5a662d0cbc4b5` runtime checkout under `/private/tmp/mcr-world-runtime-resume-20261104`. That checkout remains stopped. No old prerequisite history was replayed into this branch.

The owned additions are `world/{runtime,membership,ingress,admission_bridge}.rs`, `adapters/in/{mod,world_ws}.rs`, `config/world_runtime.rs`, `tests/world_runtime.rs`, `tests/world_ws.rs`, `tests/world_runtime_support/**` and this document. Shared-file edits are additive module registrations and world-only startup/supervision in `main.rs`. Main owns Cargo manifests/lockfiles, prerequisite simulation/replication/admission modules, RPC/proto and Gateway integration.

Rendering and artwork are untouched. These modules are newly authored and consume the committed typed Concert ports; no legacy rendering is copied. Frozen C05 revision `69a5624cd153a216f2398cebabf062400dce23eb`, schema SHA256 `1732e627037fe91533ac6f7fe74e36683ce9b846bfbfc020b3cce298ae84df36`, policy SHA256 `3e77e21b036386803c7d5bff0145c434f111ba51b861219a211e853a43d0628b`. C06 revision `12acc2d7a93701d89aad63ea7c3376f91b7fcb13`. Plan SHA256 `e19732ac9c5c5d5397652bf047645da0640ec72c97f59869974680c4e7f69a36`.

## Runtime boundaries

One task mutates every incumbent room on a 50ms schedule. The owner queue is 64 commands, at most 64 commands per tick, bounded catch-up follows BE12, room count is at most 16 and capacity at most 50 each. The process claim prevents a second local controller; actual PostgreSQL owner leases fence other processes. Membership reads compare the complete server-owned binding, captured immutable access expiry and live authority deadline. No client coordinate or heartbeat renews authority.

Ingress rejects unknown/duplicate fields, malformed versions, binary data, noncanonical IDs, oversized/deep documents and stale streams. Authentication is the first application message and has a fixed connection-relative three-second deadline. Whole-message and input rate limits are separate. Exact Origin validation rejects missing/null/multiple origins, wildcard configuration, alternate schemes and paths. Tickets are excluded from URL query strings and subprotocols. Heartbeat IDs match application pongs and never extend access expiry.

The socket adapter uses actual Axum WebSockets. It hands each freshly projected delivery directly to a ready native sink under the shared publication fence; it has no second application projection queue. Native write buffering is capped at 262144 bytes, each server application frame at 65536 bytes, assembled client messages/frames at 4096 bytes, and concurrent sockets at 864. Slow sockets are coalesced by replication before sequence allocation. A failed write closes rather than replays. This source requires native backpressure and malformed-frame qualification before claiming these boundaries verified.

The bridge consumes a one-use digest through `PgWorldRepository`, performs fresh external owner and Concert event/participant/restriction checks, invalidates the displaced local view before a durable takeover, and admits under the canonical owner. It uses committed `WorldService` switch/renew/leave and typed repository admission ports. Stream/room epochs are runtime-owned and never invented as database proof. Background maintenance renews room ownership and current membership independently of client traffic; expiry remains capped by the original access credential. Exact cleanup remains bounded, idempotent and owner fenced; nonempty shutdown cleanup is reported as failure.

`LiveWorldOwners` is a strict integration dependency: current canonical session family, active Profile/public identity and reviewed avatar selection, current restrictions, and bidirectional recipient policy. Its stable publication fence and monotonic invalidation generation must also govern known Auth/Profile/restriction/block changes. Async responses are compared to the generation before publication; stale responses cannot republish known revoked authority.

## Production startup and missing producer

`CONCERT_WORLD_LISTEN_ADDR` explicitly opts in. It requires `CONCERT_WORLD_ALLOWED_ORIGINS` (JSON string array), canonical UUID `CONCERT_WORLD_OWNER_ID`, `CONCERT_WORLD_EVENT_ID`, `CONCERT_WORLD_ROOMS_JSON` (`[{"room_id":"...","capacity":50}]`), the existing six product-release file settings and a qualified live-owner adapter. Partial/malformed configuration fails startup. Product validation uses only `ProductReleaseBundle::from_product_files`; synthetic fixtures cannot select this route. Existing health and guarded bootstrap/world RPC serving are preserved.

`WorldRuntimeSettings::serve(pool, owners)` composes the real database bridge, room owner, authority maintenance and socket listener when qualified ports are supplied. Current root `production_owners()` explicitly reports unavailable before any world listener opens. At base e869480, Gateway is the allowed caller of Auth `ReadConcertAdmission`; no Concert transport supplies fresh ticket-only canonical Auth/Profile checks or the BE37 recipient invalidation fence. Main has been sent the exact missing producer/authorization dependency and is allocating it separately. This fail-closed state is an implementation dependency, not completed production delivery.

## Validation and resource record

- Scoped `rustfmt +1.88.0 --edition 2021`: exit 0. Default stable rustfmt is unavailable due to a missing dylib; explicit Rust 1.88 succeeds.
- `git diff --check`: exit 0 before the source checkpoint.
- `python3 scripts/mcr/check-plan.py`: exit 0; all 13 archive hashes and planning provenance pass. This checks planning only.
- Main granted exclusive mutable `/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target` after BE26 released its handles and froze evidence. No third target may be created. Rust 1.88, locked/offline, at most two jobs, development/test debug zero and 8GiB free floor are required. No Cargo has run at this checkpoint; central ws dependencies and BE13/14 prerequisite imports are pending.
- Authored unit and actual loopback socket regressions are NOT_RUN. Socket fixtures explicitly synthesize authority and geometry. They do not prove live canonical Auth, product art, deployed Gateway routing, real multiuser behavior, load, IN01 or release acceptance.

A later evidence checkpoint must record exact commands/exits, current source hashes, Cargo JSON-selected executable hashes/copies and unresolved failures. No test count or historical evidence is promoted here.
