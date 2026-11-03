# Sealed World projection context

Status: IN_PROGRESS. Base: `1b7caeccdd10856b3c5ebaa0e9b0286ef8a2ced0`.
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
or a fresh timeout. That read creates the current parent for initial visibility.
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

Focused lib + retained RelationshipAuthority test compilation passed (Rust 1.88.0, locked/offline, one job). Deterministic and native execution: NOT_RUN at initial source checkpoint.
Native qualification uses only private marker/inode-owned MCR PostgreSQL stores.
A context check validates captured scope/expiry; it does not freshly query current
ownership, account status, membership, blocks or privacy. Consumers must re-read
owning policy and local membership. This context is not an invalidation protocol,
mutation capability, event-admission decision or distributed publication fence.
Profile RPC and recipient producer qualification belong to their respective
source owners; full multiuser/runtime composition remains NOT_RUN here.
