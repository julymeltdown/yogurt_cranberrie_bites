# BE33 Notification product facade

Status: **IN_PROGRESS**. Source checkpoint for the four frozen C04 operations;
no application, owner-policy or HTTP acceptance is claimed by this document.

## Ownership and ancestry

Delivery repository is `julymeltdown/mcr`, `master`. This patch starts at
`a7f3989d9c40c53a9c2982bced938f8b4853f627` in the isolated
`codex/mcr-be-BE33-product` branch. Root integrates and publishes. The former
BE33 binding writer is stopped; NB01/NB02 reader/client/ledger/settler behavior
is unchanged. New files are `notification_live/{protocol,product}.rs`,
`tests/notification_product.rs`, `tests/notification_product_support/**`, and
this document. `notification_live/mod.rs` adds module declarations and exports
only. No contracts, generated code, migrations, manifests, lockfiles, Gateway,
global operation registry or main composition are modified.

The contracts are the base revision's `contracts/domain/v1/operations.json`,
`feed.schema.json`, `common.schema.json`, and the referenced core schema.
The runtime uses existing `concert-domain` closed Notification DTOs and the
actual base `NotificationClient` transport. Rendering is not reused.
`docs/source/` is untouched.

Test provenance: `notification_product_support/environment.rs`, `owners.rs`
and `capture.rs` were copied from `tests/owner_authority_support/` at the exact
base above. Changes add the fourth isolated Notification database, actual
Notification settlement endpoints, and capture an arbitrary exact signed
ProductRequest through the existing production Gateway workload/signature
guard. The fixture and native tests are newly authored. The qualification
runner adapts the existing owned `migration_smoke.Cluster`, frozen executable
checks, owner migration receipts and mTLS PKI helper, adding the Notification
workload certificate. Those source dependencies are included in the complete
source hash inventory. Existing copied sources remain unchanged.

## Concrete interface and behavior

`NotificationRuntime::new(Arc<NotificationClient>)` accepts the actual configured
transport. `dispatch_for_scope(Arc<OwnerGuardScope>, ProductRequest)` returns
`Result<ProductRepresentation, tonic::Status>`. It also implements the existing
`ScopedProductProducer`; global operation registration belongs to Root.

The runtime requires the original task-local `VerifiedParentAuthority`, decodes
its exact original ProductRequest, compares all fields, verifies parent/frame
fingerprints and the supplied scope's actor/event/family/request, operation,
method, complete target, body digest and original deadline. It validates current
Auth/Profile observation bounds using the existing two-second Product budget.
It does not acquire another scope, mint a token, release a child, finish delivery,
renew authority, or construct a successful default.

The closed parser accepts only:

| Frozen operation | Request |
| --- | --- |
| listConcertNotifications | GET own notifications; limit 1..100, default 20; opaque C04 cursor; unread_only true/false |
| getConcertNotificationUnreadCount | GET own notifications/unread-count; no query or payload |
| markConcertNotification | PATCH own notifications/{canonical nonnil UUID}/read; closed expected_revision 1..9007199254740991 object |
| markAllConcertNotifications | PATCH own notifications/read-all; closed empty object |

Both mutations require exact `application/json` and a canonical nonnil UUID
idempotency key. Reads reject body/content type/key. Duplicate decoded query
keys, unknown keys, malformed escapes/UTF-8, controls, foreign event paths,
unknown JSON fields, duplicate JSON fields and invalid revisions fail locally
before remote work. Query values are decoded once. The owner currently uses
UUID cursor values internally; a structurally valid opaque C04 cursor remains
subject to actual owner validation and is not treated as authority.

The same existing client preallocates and durably registers the private child
before Execute, then retains it through the actual Root lifetime. Explicit DTO
projection strips the internal child ID, digests, views and capabilities.
Projection additionally validates page limit/unread filter, unique IDs, current
actor/event, exact mutation ID, closed kind/target tags, read/revision coherence,
and nonfuture valid timestamps bounded by the retained disclosure ceiling.
Unread totals remain the owner's full count, independent of page size.

Owner version conflict and idempotency conflict remain HTTP 409. The existing
mTLS owner RPC has no structured error-reason field, so idempotency expiration
uses its exact closed constant `Notification idempotency expired`; unknown
FailedPrecondition errors remain safe dependency failures. No raw diagnostic or
metadata is copied to the fixed public error envelope. This does not extend the
shared proto or infer an expired result from a generic precondition failure.

`ProductRepresentation` deliberately has no header or completion field. Actual
`ScopedProductOwner`/Product/Gateway composition owns no-store and response
capabilities. This patch cannot certify its actual HTTP no-store boundary until
Root routes these operations through that composition.

## Qualification and limits

Initial static checks: Rust 1.88 rustfmt on explicitly assigned Rust files,
Python runner syntax, and `git diff --check`: exit 0. Cargo, frozen test binaries,
PostgreSQL and mTLS native execution are **NOT_RUN** pending a fresh named
backend resource allocation. No shared target or live Root service was used.

Planned exact artifacts are the Cargo-emitted `concert_service` library test
binary (four private projection tests only) and `notification_product`
integration test binary (four parser tests and three explicitly ignored native
cases). Private tests validate DTOs against the actual frozen JSON schemas,
including referenced schemas. Synthetic response values do not attest visibility.

The native runner refuses dirty/mismatched source or unknown binaries and owns
fresh socket-only PG16 clusters, private PKI and four databases per native case.
It starts production Auth/Friend/Notification owner stores and guards, uses the
real signed Gateway parent, production ledger and Notification child transport.
The fixture's late-bound recovery adapter delegates to the actual durable ledger
lookup and real Notification recovery transport; it grants no default outcome
and breaks its test composition links on Drop.

Expected native cases are:

- All four operations refuse with safe 503 when the real target provider is
  missing, including empty inbox/count. Root has one pending durable child and
  both real owner exclusions until actual component body EOF (serialization
  only) and authenticated real DeliveryService completion. Duplicate completion
  is idempotent; real Notification close-before-dispatch creates a no-effect
  tombstone and confirms the child.
- Missing original task-local parent and altered sealed request fields never
  register a child or invoke Notification local intent work.
- Dropped/unconsumed component body plus original expiry leaves Root ACTIVE,
  the child pending and actual owner exclusions quarantined. Teardown of that
  owned PostgreSQL fixture is explicitly not settlement evidence.

These are component mechanics, not the complete ProductRpcService operation
registry, actual ScopedProductOwner no-store wrapper, production Gateway outer
HTTP, or a successful authorized inbox flow. All four real successful flows,
real target stale/foreign/deleted/revoked/block visibility, target-writer
exclusions, successful mutation revision/idempotency round trips, native target
transactions and restart/recovery acceptance are **NOT_RUN** until the real
shared target provider and routing are composed. NB01/NB02 historical results
are not relabeled as fresh evidence here.

## Root-owned target provider contract

At this source base, `RootTargetSnapshotOwner` is a required port with no
production implementation. Notification refuses missing configuration before
local work even for empty/list/count. Existing Post-only CheckSubject is not
an eight-kind Notification target policy.

Existing input is the exact private `ParentScope` (binding and Auth/Friend/Root
views), immutable `Context` (actor/event/family/request/digest/observed/expiry),
ordered `Candidate` values (notification ID, optional source actor, optional
closed typed target), and current time. Existing output is exact bound
`Authorization` with the same ordered decisions Visible/Stale/Hidden,
checked_at/valid_until no later than the original expiry, and a retained
`AuthorityFence`. A remote callback must additionally preserve its exclusions
at Root after replying, rather than returning timestamp-only permission.

Root must resolve the exact `ScopeRegistry::lookup_root_read` private capability,
compare complete original bindings and exact candidates, validate real event
membership/moderation and current target/source policy, register actual retained
exclusions/native effects under the same Root before replying, and retain them
through Notification completion and genuine Root terminal EOF or mechanical
lost-body settlement. The RootReadPermit is callback admission and must drop
before EOF callback drain; retaining that permit itself until EOF would deadlock.
Missing/unknown policy denies even an empty candidate list. Stale neutral items
require explicit owner permission; deletion is not automatically permission.

Actual source owners required by the closed target types are Concert party
(PARTY/APPLICATION), Concert gift (GIVEAWAY/RESERVATION), Concert social
(SOCIAL_REQUEST), Post (POST), and Concert notice/moderation (NOTICE/REPORT),
with canonical Auth/Friend source exclusions. The callback protocol, provider,
root policy composition and exact shared followup ownership are Root's scope.
This facade does not acquire those paths or wait for their review to finish
its own source work.

## First qualification checkpoint (superseded by object-shape fix)

Source `5f85e06fee7c6a2d5b5109f90f05b75831db4e45` compiled successfully
with Rust 1.88, locked/offline, two jobs, debug/incremental disabled. The exact
command was `cargo +1.88.0 test --locked --offline -p concert-service --lib
--test notification_product --no-run --message-format=json` (exit 0).
Compile/freeze took 58.90 seconds, peak observed total growth 176,316,416 bytes.
Both Cargo-emitted binaries and the complete tracked backend/contracts sources
were frozen outside the shared target; zero target handles were verified and
the target explicitly released before copied runtime execution.

The first run found that Serde's empty struct decoding accepts `[]`; C04 requires
an object. The failing parser case returned exit 101. Seven other exact tests
passed (three parser cases and all four private DTO/error tests; 130 unrelated
library tests filtered). The fix requires a JSON object before typed decoding
for both mutations and adds the analogous `[1]` revision regression. No native
case or PostgreSQL process was started. This failure remains retained, not
rewritten as a pass. All lease resources were released.

Evidence directory: `/private/tmp/mcr-be33-product-5f85e06f`, containing complete
Cargo JSON/stderr, `artifact-receipt.json`, `target-release.json`,
`qualification.json`, `runtime-bounds.json`, and `independent-tests.json`.
Integration executable SHA256: `4dec5740550376bde3482518eb2f303c99f5a5e8ec89b22c49388c71aa7ff686`;
library executable: `03a1533403add1e675d7932f7f7921fff4e9e18f2bd595d09c6847b0323ab2ff`;
source archive: `2d8c7976acd26aa24dd18b861b6ee756be8ef37e763995c6481bdc5b2d4fae38`.
Copied checks took less than two seconds total, with no roots remaining,
peak observed whole growth 176,951,296 bytes and minimum free 39,745,662,976 bytes.
The fixed source still requires a fresh clean-pin allocation and rerun.

## Explicit prerequisite intake

After full release of the first lease and the object-shape fix, Root explicitly
transferred the exact five-path prerequisite
`591d908a85a01fe0a494e04c0109bb0bec64a3e1` for serial `cherry-pick -x`.
It was imported as `ecb088156b6cc5ff0cfbafdc5682381193b177b7`.
This is an authorized prerequisite intake, not implicit application ownership:
Auth/Friend/Notification/Concert `build.rs` now resolve their existing proto
roots using runtime `contracts_build::proto_root()` instead of the previous
compile-time manifest directory expressions; Root's accompanying
`build-script-worktree-paths.md` preserves its observed failure and rationale.
No cleanup, extra artifact, migration, schema or operation scope is acquired.
The earlier source pin is superseded for the fresh allocation. Runtime fixture
migration paths still need actual native verification; the proto-path fix
alone is not evidence that those baked paths point to the current checkout.
