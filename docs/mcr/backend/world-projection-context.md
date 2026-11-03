# Sealed World projection context

Status: CONTRACT_READY (committed source handoff; runtime integration NOT_RUN). Base: `1b7caeccdd10856b3c5ebaa0e9b0286ef8a2ced0`.
This is a bounded read-context prerequisite, not BE13/BE14 or IN01 acceptance.

## Source and ownership

Newly authored Concert adapter code builds on the Auth-only World read and
retained `platform::actor_delegation::authority_deadline::AuthorityRpcDeadline`.
No upstream rendering, dependency, proto, manifest or lockfile changes.
Writable paths are `relationship_authority/world_auth.rs`, its `world_auth/tests/`
subtree, new `world_projection.rs` and its subtree, this document, and only the
new module export/conversion in `relationship_authority/mod.rs`.
The runtime, Profile owner, recipient owner and wire contracts have separate writers.

## API and lifecycle

All new types/methods are crate-private in
`adapters::out::relationship_authority::world_projection`.
`WorldOwnerRequest::projection_parent(&WorldAuthObservation)` is fallible and
checks the exact Auth result before capturing immutable consumed-ticket or current
membership provenance. The parent can be cloned; cancellation and attempt slots
remain shared with the originating request. It retains the original absolute and
monotonic operation stop, canonical actor/event/family/request, original credential
and source expiry. Effective deadlines may only shorten, including Auth expiry.

Parent `subject()` returns `&WorldSubject`; `actor()`, `event_id()`,
`session_family_id()`, and `request_id()` return `Uuid`. `deadline()` and
`original_deadline()` return `AuthorityRpcDeadline`; expiry getters return
`DateTime<Utc>`. `pin()`, `room_id()`, `connection_id()`,
`current_membership() -> Option<&CurrentOwnedMembership>` and
`consumed_ticket() -> Option<&TicketClaim>` expose captured authoritative scope.
`check()` returns `Result<(), WorldError>`; `cancel()` is terminal and shared.

`parent.profile_projection()` returns `Result<WorldProfileProjection, WorldError>`.
It has only self subject (`subject() == actor()`, both UUIDs), purpose
`WORLD_SELF_PROFILE`, and the inherited scope/expiry/deadline/check/cancel getters.
Its `begin_read()` returns `Result<WorldProjectionAttempt, WorldError>`.

`parent.presence_projection(candidate, expected_owner, expected_subject,
expected_connection)` returns `Result<WorldPresenceProjection, WorldError>`.
The parent must be backed by current membership. The candidate must be an actual
server-read current membership with exact owner/session/connection and matching
parent event/room/owner/full resource pin. Candidate source and credential expiry
clip the inherited deadline. `parent()` and `candidate()` return references to
captured records; `query()` returns a `PolicyQuery` with `Field::Presence`.
Actor and subject getters return UUIDs. Candidate expiry getters are
`candidate_credential_expires_at()` and `candidate_source_expires_at()`; the
unprefixed expiry getters retain the actor's original values.
`begin_auth_read()` and `begin_friend_read()` are fallible reservations.

A ticket parent cannot be promoted by attaching arbitrary membership. After real
DB admission, the runtime must perform a separate exact current-owner Auth read
bounded by the SAME original operation deadline, without another ticket consume
or a fresh timeout. Pass the earlier effective `ticket_parent.deadline()` (or a tighter stop) to this new read; `original_deadline()` is provenance and does not undo a shorter effective/Auth expiry. That read creates the current parent for initial visibility.
A current operation retains its request and checked Auth result rather than
reducing them to raw IDs or an AuthWindow. Runtime composition is a later patch.

## Attempt and I/O rules

Profile has one self slot; Auth's original World read keeps its separate slot.
Presence has independent Auth/Friend slots for each canonical target subject
under the immutable actor/event/family/request/purpose scope. Repeated derivation,
cloning, lease/owner/session/connection changes never reset the same wire target's
slots. Distinct targets can fan out, bounded to 800 targets per original request.
A reservation burns the slot before checking cancellation/expiry; failures and
dropped tokens or futures do not restore it.

The attempt token is not Clone. It exposes scope/check/deadline getters and
`run<T,F>(self, F) -> Result<T, WorldError>` for a future producing that Result.
It captures an earlier ambient stop at reservation and execution, checks before
and after await, times out at the inherited monotonic stop, polls shared
cancellation and rejects a noncooperative late success. Wrap the entire owner
operation, including channel readiness, signing and response validation.

`RequestBinding::from_world(&WorldPresenceProjection)` is a pure, fallible scope
conversion for the existing signed RelationshipRead. The legacy binding does not
itself carry cancellation or attempt state. Reserve BOTH Auth and Friend slots
before invoking the existing dual-owner adapter, then nest their `run` guards
around the full call. Reserving both before I/O ensures either owner error cannot
leave the other budget reusable. Independent Auth and Friend revisions stay
separate. No new wire fields or scalar revision packing are introduced.

## Evidence and limitations

Qualified implementation ancestry: `353e7f8ebe96bac3c90134e6ca60e627589decb5`
→ `9aabea8e937ad935f62ea228d6f4f3b9e742f20d`
→ `65419f6557cd56d7f0b3d840bacd20fd9bb65570`, all atop the exact base above.
The final documentation-only handoff follows these commits; no master push here.

Focused compile and execution passed with Rust 1.88.0, locked/offline, one job,
dev/test debug 0, incremental disabled, and the exclusively allocated existing
`/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target`. No new target or install.

| Check | Result / exit |
|---|---|
| `cargo +1.88.0 test --locked --offline -j1 -p concert-service --lib --test relationship_authority --no-run --message-format=json` | PASS / 0 |
| `cargo +1.88.0 build --locked --offline -j1 -p auth-service -p concert-service --bins --message-format=json` | PASS / 0 |
| Pinned lib, filter `adapters::out::relationship_authority::world_auth::`, `--test-threads=1` | 14 PASS (6 context + 8 retained Auth); 3 native cases deliberately ignored here, executed below / 0 |
| Pinned retained RelationshipAuthority integration binary | 3 PASS; separate two-owner native case outside this context scope / 0 |
| New `world_projection/native_smoke.py` with exact successful combined Cargo JSON and private copied binaries | Context native 1 + retained Auth native 2 PASS / 0 |
| Isolated PostgreSQL 16.14, Auth 14 / Concert 13 migration checksums, empty-store refusal, migrate, verify, reapply | PASS / expected positive and negative exits checked |
| Two controlled restarts and exact persisted owner/ticket/membership state | PASS |
| Missing marker/owner fixture refusal and final fixture cleanup | PASS |
| `python3 scripts/mcr/check-plan.py` | PASS / 0 |
| `git diff --check`, assigned changed-path check, no frontend import, old Auth checkout clean | PASS / 0 |

New deterministic cases cover self-only Profile scope, checked Auth mismatch,
shared replay gates across clones and repeated derivation, distinct-target fanout,
800-target bound, owner/session/connection/event/room/map/avatar/expiry drift,
original monotonic stop with a still-live wall deadline, earlier ambient stops at
reservation and execution, shared cancellation, dropped tokens/running futures,
failed reservations, and noncooperative late success. Existing Auth attempt
semantics remain independent. The native case reads actual consumed/current
Concert PG records and a guarded signed owning Auth response, then exercises
context scope, attempt replay, guarded SQL and cancellation of a real PG await.
Its SQL is a context transport probe; it is not a real Profile or Friend RPC.

The first build caught an incorrect test-only AvatarPin field name. The first
native attempt correctly refused a reused fixed test ticket digest. Both failed
receipts remain preserved; the corrected native fixture uses a unique ticket and
the production invariants were unchanged. Final source hashes match the passing
native report, which records a clean source checkout.

Evidence root: `/private/tmp/mcr-world-projection-context-1b7caec-20261104/`.

| Receipt | SHA-256 |
|---|---|
| `build-combined-3.jsonl` | `df28ddf3c1148280f78262edf8cc5e88cd51e1bc57c231cdfabd240d00b2325e` |
| `native-report-2.json` | `576ea48659d0dc60635079ee39d2c96fdebff7be72b7e9846be834ae3e0d512c` |
| `deterministic-final.json` | `dd912ddd2b29e09224b997bd2a3e15be6dbf1e12db3e241ee21b4fa8eb92eda8` |
| `target-release.json` | `1255f54bf59722925c175fea768e0f2c3b0437f1434fb430c5608cb789579caf` |

The successful native report pins lib test
`2584b6fd2c60f3ca649e0a6a0e4daf50a4af14c93c24ea170b348fd68af30d1e`,
Auth binary `781a14a1bedba587e69ea1837acabdfae4eb8991dd0edba74b7db2b66d95540f`,
and Concert binary `c01d2574f6d6e4f48d0140be515f7983eb5092bcc15c709e9bb1f85ae358410f`.
The harness delegates the retained marker/inode-owned migration and artifact
selection logic in `tests/world_auth_read/native_smoke.py` without editing it;
its source and the new wrapper are pinned in the successful report.

Target release is terminal: all own Cargo/test/native/PG handles exited, zero
owned processes, both owned fixture roots absent, all 64 previously frozen Auth
evidence files unchanged, and 12,021,399,552 free bytes at release (>8 GiB).
The writer stops after committed handoff; Main reallocates the target serially.

A targeted read-only review found no additional scope/gate/deadline or Auth
regression issue. Its one API finding was the already documented bare-binding
escape hatch: converting alone cannot enforce cancellation or attempts. The
assigned conversion leaves legacy/Gateway binding behavior unchanged; guarded
consumer use is mandatory. This limit was sent explicitly to Main, World scrum,
Profile and recipient owners. Cross-owner runtime composition must verify the
nested guard contract; this local handoff does not certify that later flow.
Native qualification uses only private marker/inode-owned MCR PostgreSQL stores.
A context check validates captured scope/expiry; it does not freshly query current
ownership, account status, membership, blocks or privacy. Consumers must re-read
owning policy and local membership. This context is not an invalidation protocol,
mutation capability, event-admission decision or distributed publication fence.
Profile RPC and recipient producer qualification belong to their respective
source owners; full multiuser/runtime composition remains NOT_RUN here.
