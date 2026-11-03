# World owner composition — 2026-11-04

The concrete Auth, Profile, Friend and Concert composition passes its isolated
native backend proof. Production World serving remains **BLOCKED** because the
required reliable known-invalidation path is absent. The startup error names
that dependency; configuring the clients does not enable publication.

Worker implementation and native validation are delivered in `2e33bba3`, based
on `aa33fa7eea26c2918e956e85983890560db98daf`. Integration into the delivery
branch is **NOT_RUN** by this worker. IN01, product-art/browser/device/load and
production deployment acceptance remain **NOT_RUN**. This increment does not
change the required task or IA scope. The broader persistent runtime goal still
reports `blocked`; its manually authorized continuation does not reset or
falsely complete that goal.

## Source, ownership and dependencies

Only these ten code/test paths changed, followed by this evidence document:

- `backend/services/concert-service/src/config/world_runtime.rs`
- `backend/services/concert-service/src/world/admission_bridge.rs`
- `backend/services/concert-service/src/world/membership.rs`
- `backend/services/concert-service/src/world/mod.rs` — new module declaration only
- `backend/services/concert-service/src/world/production_owners.rs` — new
- `backend/services/concert-service/src/world/runtime.rs`
- `backend/services/concert-service/tests/world_runtime.rs`
- `backend/services/concert-service/tests/world_runtime_support/auth_bridge.rs`
- `backend/services/concert-service/tests/world_owner_composition/mod.rs` — new
- `backend/services/concert-service/tests/world_owner_composition/native_smoke.py` — new

No producer implementation, HTTP/proto contract, migration, dependency manifest,
lockfile, frontend, infrastructure or supplied artwork changed. `main.rs` keeps
its existing pre-database/pre-listener call to `production_owners()`; its public
signature is preserved. `docs/source/` remains byte-identical, confirmed by the
planning provenance check and a clean diff.

The production adapter uses the qualified sealed Auth/projection, typed Profile
and recipient APIs already present in the base. Their producer/integration
provenance is retained in [World Profile integration](world-profile-integration.md)
and [recipient integration](world-recipient-and-intro-integration.md): Profile
`f9dcab02`/`1712307` integrated as `f757e1d0`/`87390bc1`; recipient author
`6a832c691fdcf64051e6c981ff18ada5d8511e7e`, integrated as `55fefdb8`,
`aae51b01`, `882fcdd3`, `386de58f`, `ca65d8e6`. No duplicate producer/context
commits were copied.

New native support adapts the owned-root, guarded Auth server, admission and
cleanup fixture in `tests/world_runtime_support/auth_bridge.rs` at the base;
that fixture records its upstream Auth source `30ca2a499edabb75720273a312d2a4196f18201d`.
Friend and Profile server construction follows the qualified fixtures in
`tests/world_recipient_owners/mod.rs` and
`src/adapters/out/world_profile/tests.rs` at the base. The controller reuses
`backend/scripts/mcr/migration_smoke.py` for marker/inode ownership and cleanup.
It applies the existing migrations directly in separate private databases;
this does not prove the production migration-ledger or deployment path.

## Implemented behavior

`WorldProductionPorts` now contains actual typed Auth, Profile and relationship
clients rather than the generic fixture owner interface. Configuration requires
explicit `CONCERT_AUTH_GRPC_ENDPOINT`, `CONCERT_FRIEND_GRPC_ENDPOINT`,
`CONCERT_PROFILE_GRPC_ENDPOINT`, `CONCERT_SERVICE_CREDENTIAL`, dedicated
relationship signing/trust and the existing Concert TLS resolver. Origins reject
userinfo, path/query routing, invalid schemes and unsafe production endpoints.
Production requires HTTPS/TLS; there is no fallback service credential.
Both port construction and `serve()` enforce the unavailable publication
qualification. No boolean bypass or fixture path enables the production root.

`AuthWindow` retains `WorldProjectionParent` through current identity and
recipient observation. After durable admission, the bridge reads the actual
owned membership before installing any runtime view, constructs the current
request using the consumed parent request ID and **effective `deadline()`**, and
obtains current Auth again. `original_deadline()` is never used to extend the
post-admission operation. Maintenance and resync compare the captured credential
expiry with the exact current SQL membership. Session, connection, owner, room
and map/avatar pins remain bound by the sealed producer APIs.

Profile self projection and every recipient candidate derive from that same
current parent. Current display content comes from the guarded owning Profile
RPC; avatar selection/revision comes from Concert SQL and the validated release
catalog. The generic `LiveWorldOwners` compatibility path is used only by
explicit fixtures. The actual recipient reader retains its reservation of both
Auth/Friend attempts and nested `run()` around the full read, including local
checks. Neither a raw subject nor a bare legacy request binding replaces it.

A separate Concert-local publication lease rechecks local event, pair,
participant/restriction, exact membership/owner/room and avatar state under SQL
locks. Absent relationship rows are provisionally inserted inside that lease's
transaction so concurrent insertion is fenced too; they are rolled back.
Independent remote revision vectors remain observations, not publication grants.

The lease uses the same synchronous mutex as runtime native handoff. Refresh,
renewal, admission, switch and cleanup disable publication under that mutex
before releasing SQL locks. Runtime suppresses sends and roster candidates when
the local permit is unavailable, while still enforcing real authority expiry.
A supervised transaction task disables the permit before rollback on stop,
original monotonic expiry or parent cancellation; `pause()` waits for rollback
and propagates rollback failure. The native proof establishes this local locking
behavior on a healthy owned PostgreSQL server. It does not establish behavior
under arbitrary backend termination or qualify production availability at load.

## Final validation and retained evidence

Rust **1.88.0**, locked/offline, two jobs, dev/test debug info disabled and
incremental disabled. The authorized MEDIA target was the only build target
used. Cargo JSON selected the exact executables, which were copied and hashed
outside the target before any private PostgreSQL controller started. Each build
iteration released all target process handles before native testing. Final
handle scan was empty; free disk remained above the 8 GiB floor.

Final evidence directory: `/private/tmp/mcr-world-owner-composition-20261104/`.
Final source/executable freeze: `build8/freeze.json`, SHA-256
`8d3323bc22201cc712deef102cf059118b09e8a3cd87a81f543861e91629a9d3`.
All **1,577** backend/contract hashes matched after validation. Exact copied
executable hashes:

| Executable | SHA-256 |
| --- | --- |
| `build8/concert_service` | `91d20a4a6bd95afd74438cb957481c936f2541d4acb52e8e9392a8b7ce145af3` |
| `build8/world_runtime` | `d221a2d11a4e32b678f73e499d3926a9f24e4291aea7deb1f93f342d4dccabe7` |
| `build8/world_ws` | `daccb16c95932dee9ffae57956f967d4040b279bd399805c4d37eff3c2c36942` |

Build command, exit **0**:

```sh
CARGO_TARGET_DIR="$MEDIA_TARGET" CARGO_PROFILE_DEV_DEBUG=0 \
CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo +1.88.0 test \
  --manifest-path backend/Cargo.toml --locked --offline -j 2 \
  -p concert-service --lib --test world_runtime --test world_ws \
  --no-run --message-format=json
```

`MEDIA_TARGET` denotes the integrator-reserved existing MEDIA target, not a new
cache. Tests below ran only copied executables, with two test threads for the
non-native suites and one for each selected native scenario.

| Check | Result | Exit |
| --- | --- | --- |
| Copied `world_runtime --test-threads=2` | 28 passed | 0 |
| Copied `world_ws --test-threads=2` | 16 passed, 4 explicit native tests ignored here | 0 |
| Copied `concert_service --test-threads=2` | 30 passed, 7 native tests ignored here | 0 |
| New `tests/world_owner_composition/native_smoke.py` | 1 actual four-owner native scenario passed | 0 |
| Existing `tests/world_runtime_support/bridge_native.py` | 4 selected native bridge regressions passed | 0 |
| Existing `tests/world_runtime_support/auth_bridge_native.py` | 1 sealed-Auth bridge scenario passed | 0 |
| `python3 scripts/mcr/check-plan.py` | planning/provenance PASS | 0 |
| `git diff --check` and ownership/import inspection | PASS | 0 |

The controllers take explicit `--binary` and `--report` absolute paths. Reports
`build8-composition.json`, `build8-bridge.json`, `build8-auth_bridge.json` record
native exit **0**, cleanup **PASS**, and removed private roots. Their SHA-256
values are respectively:

- `7e8d28abe1e68e80909ad2b92de8129e7737d117f04c1959b1f23408375f4a59`
- `a872d7802f199cbb4c472e69c4027ea8144e2d8e90754aed91193ec43f13a690`
- `f8b073b42c0470b1227ce0b2e01ef2db043773637de9529f928fc99f33ce3521`

`build8-checks.json` records individual process exits and log hashes; SHA-256
`7f4799a5efeaf15b51ee26086436c73a61b6b80fa9c60e3a3a36b383efeda40b`.
The native composition proves first admission without a prior view, one-use
consumption, current Profile refresh/deletion handling, directional Friend block
filtering for two real users, observed local SQL lock timeout, pause-before-
rollback, switch, repeated renewal, immutable access expiry, Auth revocation/
suspension/outage, credential drift rejection and owned cleanup. The runtime
unit regression verifies pause suppresses native send without extending expiry.
Synthetic avatar IDs/catalog data are metadata fixtures only, never approved
product artwork or a release.

Earlier attempts are retained: build 1 failed on a fixture server type name and
catalog error conversion; native attempts 1/2 failed before admission on missing
required fixture columns, and attempt 3 exposed a default-stack overflow. The
fixture schema was corrected and large RPC/scenario futures boxed, without a
stack-limit override. Native attempt 4 passed. Build 8 then tightened rollback
error propagation and reran the final 80 tests. All failed native attempts also
cleaned up their owned clusters. Unchanged producer native suites were not
silently counted as freshly rerun. No live production WebSocket/browser or
production TLS/deployment proof is inferred from these checks.

## Minimal C05 follow-up

C05 [authorization](../realtime-contract.md) requires revalidation within five
seconds and sooner on **known** logout/block/suspension/closure. Its replication
rules require removing known blocked users before further presence/interaction
handoff. The effective Auth publication deadline here is at most two seconds;
maintenance is scheduled every second, persistent membership/owner leases remain
bounded by five seconds, and the captured access deadline never extends.
Those existing bounds do not supply a reliable known-invalidation consumer.

The concrete remaining contract is:

1. Owning mutators durably record a scoped invalidation with the owner-local
   mutation: Auth family logout/user suspension; Friend directional block or
   privacy changes; Profile deletion/content invalidation. Concert closure,
   admission restriction, resource and ownership changes need the corresponding
   local routing. Use authenticated canonical IDs, owner epoch/revision, unique
   mutation ID and replay cursor; do not publish private Profile content.
2. Deliver each applicable signal reliably to every active World owner holding
   the affected user/family/pair/event. Preserve independent owner sequences,
   deduplicate retries and support reconnect/replay. A stale or gapped cursor
   cannot certify a live consumer; fail closed until catch-up/current reads.
3. The World consumer immediately takes the native-handoff fence, disables the
   affected permits, cancels retained parent operations and invalidates the
   relevant view generations. Revoke affected sessions or remove blocked users
   before another handoff; re-observe through the typed ports before resuming.
   Signals cannot manufacture new membership, extend access expiry or refresh
   authority by themselves.
4. Acknowledge consumed signals only after that local fencing/invalidation is
   effective. Exercise receipt-versus-send races, duplicates, replay, subscriber
   loss and mutator/owner restart. Keep already numbered transport semantics and
   exact connection/scope fences intact.

This is a scoped delivery/consumer acknowledgment contract. It does **not**
require a global atomic transaction across Auth, Friend, Profile and Concert,
nor retroactive withdrawal of bytes already handed to the native transport.
Producer mutation/outbox or equivalent reliable signal support and the World
consumer need a separately allocated contract/ownership patch. They are not
implemented by this increment. The production qualification error remains in
place until that C05 path and its real handoff races are qualified.
