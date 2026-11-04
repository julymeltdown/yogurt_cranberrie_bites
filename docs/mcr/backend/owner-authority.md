# Canonical owner authority

Implementation owner: `BE-CANONICAL-OWNER-AUTHORITY` in isolated
`be-intro-age-policy`, branch `codex/mcr-be-owner-authority`, base
`b8b5a939318bf39465fad44663eb272cb71868dc`. The former BE25 branch remains at
`7ca2f730`; its independently integrated policy work is not repeated here.
Status: **IN_PROGRESS**. Platform-only library validation is recorded below;
owner-store and combined producer native/runtime qualification remain **NOT_RUN**.

## Required behavior

Canonical Auth account/session authority and Friend pair restrictions must remain
excluded from revocation through the actual protected producer/read/commit and
final response assembly. A signed actor, workload credential, binding digest,
participant/profile existence or current owner observation cannot supply this
exclusion. Retain the existing owner journals and their mutation atomicity.

`authority/lease/v1` is not used: the additive file is
`backend/proto/authority/v1/lease.proto`, package `authority.lease.v1`.
`AuthOwnerAuthorityService` and `FriendOwnerAuthorityService` expose Acquire,
Check, Enroll, Track and Complete. Acquisition is bounded and actor-authenticated.
Opaque view and completion secrets have separate purposes. Child producers may
check/enroll/register; only the root receives completion capability.

Consumer module: Concert
`adapters::out::relationship_authority::guarded`. Binding construction requires
captured sealed Gateway parent authority and the original monotonic deadline;
the binding alone is never a grant. Acquisition creates a sealed `OwnerGuardScope`.
A `GuardedSubject` is enrolled under that existing scope, not acquired through a
new owner lock for every row. Up to 100 distinct subjects plus actor are supported. At most 202 held
local/Profile children cover all targets plus the actor without a page cut.
Root scope retains all owner guards through subreads, final assembly and the
Gateway HTTP body terminal callback. Concert gRPC serialization alone cannot
release exclusion. Local Concert event, participant and content revision locks remain
owned by the actual producer.

## Durable exclusion and uncertainty

Retained owner mutation triggers advance their respective relationship revision
singleton before committing source changes. A SHARE lock on that singleton fences
those changes, including changes to missing pair rows; source rows are read under
READ COMMITTED after acquisition. Auth is acquired before Friend. Subject
enrollment reuses the existing exclusion and does not reacquire it behind a
waiting writer. The original revision/journal lock order is unchanged.

A normal owner-process transaction or streaming heartbeat is insufficient: owner
process death would release its locks before a remote COMMIT necessarily stops.
The design uses prepared transactions to retain owner locks durably across owner
and PostgreSQL restart. PostgreSQL documents both persisted prepared state and
continued lock retention. This requires a recovery manager, bounded admission,
and prompt explicit cleanup; enabling prepared transactions without that recovery
path is not accepted. See [PostgreSQL PREPARE TRANSACTION](https://www.postgresql.org/docs/16/sql-prepare-transaction.html).

Environment prerequisite: `max_prepared_transactions >= 16`; this capacity is
cluster-wide across databases, not a separate per-database reservation. A complete
100-target projection fixture requires at least 256 cluster slots. No default or
alternate database is inferred. Missing support
fails unavailable. Admission and pool waits retain the original caller stop, with
a maximum five-second operation budget. Expiry prevents further protected work;
it does **not** permit blindly dropping uncertain exclusion. Quarantines are
capacity-accounted. Native qualification must establish the exact cap with the
allocated PostgreSQL instance before production composition is enabled.

Before any protected effect, its owning producer captures actual full xid,
backend PID and backend start on its transaction connection and registers them.
Completion first seals further registrations, then asks the configured actual
owning producer to resolve each transaction. No caller-supplied terminal boolean,
missing receipt or absent process alone proves rollback. PostgreSQL16
`pg_xact_status` plus the exact committed owning receipt (for writes) supplies the
intended outcome evidence. Unknown outcomes retain exclusion for recovery.

Cancellation drops the producer future and quarantines the scope. A cleanup
manager may release only after late work is barred and actual producer outcomes
are terminal. Owner restart, RPC loss, deadline expiry and unresolved commit are
explicit native tests, not assumed success cases.

## Current producer contracts

- Root composer owns Product RPC/config/root proto generation and actual response
  lifetime. It captures VerifiedProduct actor/event/family/request/operation,
  original `AuthorityRpcDeadline` and sealed Gateway parent.
- Feed owner owns Post read adapter/proto. Read RPCs have no content/outbox effects,
  but their actual transaction and late-response lifecycle still require terminal
  settlement before exclusion is released. No write recovery is substituted.
- Media owner captures actual producer xid/backend identity before BE15 effects;
  safe receipt is terminal only after actual commit. Overall HTTP errors can occur
  after commit and never imply rollback. Settlement queries original actor/event/
  operation/key receipt under retained authority.
- Public Profile owner needs an additional Profile-owned PUBLIC/visibility/deletion
  exclusion held through assembly. Auth/Friend exclusion does not protect a
  concurrent Profile PUBLIC-to-PRIVATE write. An owner snapshot alone is insufficient.

## Ownership and validation

Only assigned new Auth/Friend authority subtrees/handlers, their exclusively
allocated new `0017_concert_owner_guard_leases.sql` migrations, additive lease proto,
Concert guarded adapter/tests, dedicated smoke scripts, this document and narrow
exports/registration/mutator hooks are owned. Manifests, locks, source archives,
frontend, world, avatar and replay clients are untouched. Another invalidation
owner may add its disjoint parent export; preserve it.

The root composer released the exact Product Dispatch body-ceiling change as
`f2c750f9` (locally `5d901faa`); the canonical authority owner now owns the narrow
parent capture and method-policy additions. No broad request limit or authorization
rewrite is permitted. Backend owner-store developer works in isolated
`be-owner-authority-stores`, base `43f80a27`, with sole ownership of the new
Auth/Friend stores, handlers, 0017 migrations and narrow registration hooks.

## Platform checkpoint validation

Pinned Rust 1.88.0 command (MEDIA target exclusively leased for this check):

```sh
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target \
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j2 -p platform --lib
```

First and final passes: exit 0, 197 passed each; final pass has zero warnings.
Exact source and binary hashes are in `platform-checkpoint.json`. Logs are private local evidence under
`/Users/lhs/Desktop/mcr-build/owner-authority-evidence/`. This covers exact
Gateway parent/header/body binding, recovery-safe request fingerprints,
capability/producer identity separation, exact new service policy boundaries,
and actual body consumption versus cancellation/serializer failure. It does not
certify any owner database or live feature.

`platform::actor_delegation::response_scope::{ResponseScope,ResponseScopeToken,
RetainedScopeLayer}` is the response ABI: insert a token into the Tonic response
extensions, apply the layer to the server, and implement the callback for that transport boundary only. The Concert
wrapper calls `OwnerGuardScope::mark_serialized`, which records internal
serialization without release. The Gateway wrapper calls the exact separately
authenticated ProductDeliveryService after its external HTTP body ends. Only that
callback may call canonical `finish_response`. It never waits inside Dispatch
for the Gateway callback. Final trailers are withheld until the local callback
completes; errors/drop never call it. Body consumption does not attest client
receipt. Settlement cannot renew content authority.

No Cargo, PostgreSQL or target lease is held by this worker. Required validation:
actual owner-mutation races against real Concert/Post producer transactions and
response production; denied/rolled-back effects after revocation; mismatched
actor/session/event/action/target/body; bounded waits; restart, cancellation,
uncertain commit, resolution and leak-free cleanup. Mocks do not accept this scope.

## Concrete consumer source contract

Published platform-only commit `ba64c62e6b5ba7e4c3fa07fc3469c4d70e43c551`
(parent `08d1b1c0`) and IDL-only `79f7bc870a909977ce79519503b79de2d5730f4f`
were pushed without force and verified as actual remote HEAD at publication.
Neither includes the unqualified owner-store seed migrations. Local checkpoint
`1cc1cc0f` and its subsequent fixes contain the consumer source, not runtime
acceptance. Root supplies exact build source `570f7e2a` under its narrow temporary
copy grant; Participant supplies the new dedicated key wrapper source
`3bb9c295` (only `participant_decision.rs` copied; all other paths excluded).

`CanonicalOwnerFence::connect` requires explicit Auth/Friend endpoints, actual
TLS material, Concert credential, independent RelationshipReadSigner and
`Arc<ScopeRegistry>`. `ScopeRegistry::new` requires a nonzero producer instance
and a real durable `ScopeLifecycle`; there is no positive in-memory default.
Root owns that implementation in `product_rpc` and its migration 0028.

The lifecycle durably registers the scope before owner acquisition, records the
actual producer identity before tracking, records each owner registration UUID
immediately after its successful Track acknowledgement, then confirms both before
allowing effects. Track failure or lost acknowledgement cannot imply rollback.
The original deadline covers pool, lock and RPC waits. No retry creates a new
content grant. View getters expose only enrolled child capabilities, never root
completion secrets. Unknown authors are admitted through the authenticated
Post-to-Concert CheckSubject callback, an exact root view lookup and an actual
GuardedSubject before `PublicProfileReader::local_subject`. Scope check does not
call this local acquisition recursively.

The actual Gateway terminal handler seals the registry and durable lifecycle,
waits for existing RootReadPermit callbacks, and records `finish_root_work`
BEFORE owner Complete. That record proves no more root work can start; it does
not claim producer transactions are terminal. Each owner independently resolves
its registered producer transactions and only then releases exclusion. Held child
guards are completed afterward. Scope memory absence, instance mismatch,
expiry, missing receipt or body cancellation never proves terminal state.

`GuardedAuthority` implements the retained FencedRelationshipAuthority seam only
for an actual transaction already registered on the same system/database/full
xid/backend identity. The retained FreshAuthority scalar is the Auth source
revision, not a combined vector or the Media security version. The complete
Auth/Friend epoch/revision vector remains on the scope; per-user actor/subject
security generations are exposed separately from actual Auth SubjectState.
Projection owners must retain and validate that complete scope.

## Compiled SDK checkpoint

Rust 1.88.0, locked/offline, two jobs, debug/incremental disabled:

| Command suffix | Exit | Result |
| --- | --- | --- |
| `-p platform --lib` | 0 | 202 passed, zero warnings |
| `-p concert-service --lib` | 0 | 38 passed, zero failures, 13 inherited native tests ignored |
| `python3 scripts/mcr/check-plan.py` | 0 | Planning/provenance gate only |
| `protoc ... authority/v1/lease.proto` | 0 | Descriptor contract validation |

Concert emits nine inherited warnings outside this assigned adapter; the new
consumer has no warnings. The initial Concert build exited 101 because the shared
build helper artifact referenced another checkout. Refreshing only the existing
contracts-build source mtime recompiled its already-correct runtime root resolver;
no helper contents changed. The retry and final affected pass exited 0. Exact
source and copied binary hashes are in private `consumer-checkpoint.json` beside
the build logs; the MEDIA lease was explicitly released after freezing them.

The dedicated Participant key wrapper is copied only from `3bb9c295`; the dedicated
SELF key wrapper is copied only from `a91d7256`. No other producer files from those
commits are imported. Root explicitly delegated coherent publication of its exact
`570f7e2a` Concert build blob with the SDK, retaining subsequent build ownership.
No manifest, lockfile, owner seed migration or archived source is part of this
compiled standalone delivery.

Remaining acceptance gates are real Auth/Friend service and database races,
all actual producer Track/settlement adapters, Gateway HTTP terminal delivery,
restart/cancellation/unknown commit recovery and final leak cleanup. In particular,
opaque child handles need durable pre-dispatch/pre-PREPARE intent and idempotent
terminal recovery. Core remembers acknowledged child completions locally; that
alone cannot recover handles lost on process restart. Profile owner 205 and root
owner 1fc own the corresponding producer receipt and private durable intent
implementation. Their pending shared intent hook will be additive. No live
feature factory or production-readiness flag is enabled by this SDK delivery.

## Durable child SDK extension

The standalone SDK1 delivery is
`888cb6bd30846d891286b92a9a66e921e8d1ce78`, published on actual parent
`969b3b2fe45030cfad90e34f224797196ba3bdc8` with only 15 explicit SDK/platform/
build/document paths. Its remote HEAD was verified after the non-force push.
Original source-only ancestry is preserved on `codex/mcr-be-owner-authority-input`;
the active owner branch now starts from this clean published SDK baseline.

The additive closed `ChildRecoveryReference` has exactly PROFILE_PUBLIC and
LOCAL_PUBLIC variants. It binds all five query identities, the original expiry
and complete root fingerprint. Profile uses a preallocated child identifier and
private completion key; Concert local children carry actual cluster/database/
full-XID/backend/instance identity captured before PREPARE. Arbitrary URLs, SQL
identifiers or replacement GIDs are rejected. The Profile child-id encoding is
checked against the actual retained Profile ReadBinding validator.

`ScopeLifecycle::register_child_intent` is required before any Profile RPC or local
PREPARE can run. Root persists the private reference under an explicitly configured
at-rest key. `retain_child` accepts only that exact pre-registered reference;
there is no successful default hook. After actual idempotent child completion,
`record_child_completed` records the exact reference before advancing the local
completion index. `settle_child_intents` resolves all durable intents, including
lost replies with no live child handle, before root scope retirement. Profile
owner 205 implements its durable 0009 receipt/cancellation protocol; root owner
1fc owns its encrypted intent ledger and actual recovery dispatch.

Exact transaction registration is deduplicated only after both owner Track
acknowledgements and durable confirmation. The key includes cluster, database,
full xid, backend PID/start, producer instance, effect fingerprint and complete
producer reference. A partial/lost Track acknowledgement never enters this cache.

A private unexposed-acquisition guard handles cancellation before any capability
can escape. It synchronously seals callbacks, then a bounded settlement-only task
may persist no-root-work. It does not release owner transactions. Actual owner
recovery still independently verifies the durable root scope and every registered
producer transaction. Ordinary live scope Drop has no completion behavior.
Unknown cleanup keeps counted quarantine.

The settlement path uses a separate completion mutex and releases admission
serialization before waiting for existing callback permits. A callback already
holding a permit can then observe the sealed scope and exit instead of waiting
for a mutex held by its own drainer. Freshness checks cover the current subject
and the actor's local guard, avoiding a page-wide child RPC scan on every row;
both owner revision vectors are still checked on every refresh. A read-only peer
identified the lock-order issue; the final source includes this correction.
Actual joined callback-drain and 100-subject qualification remain NOT_RUN.

SDK2 validation on exclusive MEDIA leases: platform library 202 passed with
zero warnings; Concert library 40 passed, zero failed, 13 inherited native tests
ignored, nine inherited warnings outside the owned adapter. The two added tests
check actual Profile child-wire compatibility, closed reference serialization and
native identity shape. Initial build exit 101 came from a cached Friend build
script's other-worktree path; only helper/build-script mtimes were refreshed,
without changing their contents. Final retry of the corrected mutex/query source exited 0 using
`cargo +1.88.0 test --locked --offline -p concert-service --lib` with jobs 2,
debug 0 and incremental disabled. Frozen binary/source receipts are private
`sdk2-checkpoint.json` and `sdk2-concert-final-2.log`; the target was explicitly
released. Platform source pins match the earlier 202-test run.
These tests do not certify the real owner/root/Profile restart flow.

At this handoff the old BE25 persistent goal was reconciled against its original
pure-policy-only criteria: delivered commit 3810016a, existing combined 28/28
qualification and eight matching implementation/test pins satisfy it. That goal
was completed truthfully. The current unbudgeted canonical-authority goal remains
active for the outstanding implementation and real-flow qualification above.
