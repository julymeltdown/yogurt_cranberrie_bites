# BE33 Notification product facade

Status: **IN_PROGRESS**. The four-operation facade and its component qualification
are delivered: all 11 exact checks passed at clean source
`09395dd0afaa44ee28a13c657a8858f67818d672`. Real target-policy composition,
successful inbox flows and public HTTP acceptance remain **NOT_RUN**. The final
documentation commit changes this file only; it does not change the tested source.

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

At the initial checkpoint, static checks were Rust 1.88 rustfmt on explicitly assigned Rust files,
Python runner syntax, and `git diff --check`: exit 0. Cargo, frozen test binaries,
PostgreSQL and mTLS native execution are **NOT_RUN** at that initial checkpoint. The completed fresh allocation and
actual current results are recorded below. No shared target or live Root service was used.

The qualified exact artifacts are the Cargo-emitted `concert_service` library test
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

The three qualified native cases are:

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

A subsequent source review before the queued rerun found a fixture composition
omission: the Notification endpoint explicitly requires the production
`CallDeadlineLayer` task-local deadline, while the copied low-level server
fixture had only method/workload guards. The scope-specific fixture now installs
that same production layer on its owner/Notification/Root servers and Delivery
component. This prevents an unrelated missing-deadline 400 from masquerading
as the expected missing-target-provider 503. No runtime or shared target was
active while this correction was made; the pending pin is superseded.

## Completed fresh qualification and handoff

Tested clean source: `09395dd0afaa44ee28a13c657a8858f67818d672`.
Contract revision remains the base's frozen C04 bytes; full tracked backend and
contracts file hashes are in the source receipt. Both executables were emitted
by the same actual Cargo invocation after the authorized runtime-proto-path
prerequisite. No source changed during compile/freeze or copied qualification.

Named lease: `BE33-PRODUCT2-MEDIA-202611040651Z`. Fresh preflight verified clean
pin, no target handles and 37,702,811,648 bytes free. Command (backend directory):

```text
cargo +1.88.0 test --locked --offline -p concert-service --lib --test notification_product --no-run --message-format=json --config profile.dev.package.contracts-build.opt-level=2
```

Environment was two jobs, development/test debug information disabled and
incremental disabled. The package-specific contracts-build opt-level 2 was
explicitly declared under the offer. No main executable was built or run as
qualification, no cache was cleaned, and no new target directory was created.
Build/freeze exited 0 in 55.087 seconds. Cargo JSON, stderr, two exact emitted
executables, and the complete tracked backend/contracts source archive were
frozen privately before target release. The Cargo process group was absent;
fresh target `lsof -nP +D` returned exit 1 with empty output. Target release was
explicitly sent before any copied test or PostgreSQL process. There was no
subsequent shared-target access.

Copied qualification ran the committed
`tests/notification_product_support/qualify.py` with its exact frozen receipt,
owned PostgreSQL 16.14 tools and an external whole-growth/time monitor. It
returned exit 0 in 12.258 seconds. Each case was selected by `--exact` and
`--test-threads=1`; native cases additionally required `--ignored` explicitly.
The runner rejects a filtered empty pass. The actual executed inventory was:

| Cases | Result | Scope |
| --- | --- | --- |
| Four parser tests | 4 PASS, exit 0 each | Four routes, bounds, encoded/duplicate/unknown query, closed JSON object bodies, revisions, keys, actor/event/family/request/path/method |
| Four private DTO/error tests | 4 PASS, exit 0 each | Frozen schemas, closed tags/IDs/times/revisions, page/unread filter/operation identity, full count bounds, safe 409 reasons; 130 unrelated lib tests filtered per exact invocation |
| missing_target_policy_denies_all_four_under_real_owners_and_terminal_body | PASS, exit 0 | Real Auth/Friend/Notification mTLS and ledger/child/Delivery composition; all four 503 refusals, held child and owners until real component body EOF plus authenticated terminal completion |
| sealed_parent_mutations_and_missing_task_local_never_register_child | PASS, exit 0 | Original sealed parent enforced; no durable child or Notification local intent for altered requests |
| dropped_body_and_expiry_do_not_finish_registered_child | PASS, exit 0 | Actual Root ACTIVE, child pending, one Auth and one Friend prepared exclusion after lost body and expiry |

Native fixture execution used three fresh sequential socket-only clusters,
never more than one at a time, with isolated Auth/Friend/Concert/Notification
databases and fresh mTLS PKI. Every cluster's migration ledger matched current
source checksums: Auth 18, Friend 18, Concert 37 and Notification 6 migrations.
The frozen integration executable contains the actual current fixture manifest
path; observed baked service paths and exact migration hashes are recorded in
`migration-path-pins.json`. No old checkout was redirected or edited.
The first two cases ended with zero prepared transactions in all four databases.
The lost-body case deliberately ended with Auth=1, Friend=1, Concert=0,
Notification=0, proving quarantine; its private cluster stop/delete is cleanup,
not successful producer settlement. All three owned roots and corresponding
postmaster processes were verified absent afterward.

Peak observed whole growth was 444,641,280 bytes, including the retained first
run's 176,951,296-byte carry, all new target deltas, both frozen executables,
complete source copies/archive, PKI, four-database clusters, temporary files,
logs and reports. This is below the hard 1 GiB cap. The minimum observed free
space across fresh build and runtime was 37,688,340,480 bytes, above the 8 GiB
floor. Compile/freeze was below ten minutes; copied qualification was below
five minutes. All target, runtime and owned PostgreSQL resources are released.

Evidence is retained in `/private/tmp/mcr-be33-product-09395dd0`:

| Artifact | SHA256 |
| --- | --- |
| notification_product executable | fa2e13620ce890f58af81354e4e11502e3e04a8b980dc324febd48ad85968dcc |
| concert_service library test executable | 3bb598033e90e62d2c97e287f9da61d771706091349ebbec1c663e459a42cf88 |
| complete source archive | c7b27bca691719e8e33cefdd7ae12f06c80ffc42c37be727a9a27223c184eec4 |
| artifact-receipt.json | 083f1973de16690d939156fb9e24df07db71b746c616f31b9caa4e3a39d49006 |
| qualification.json | 9d101a68c2c2f5a5b1e1e52370d46a01d9830831e0bab08c788b580b69b70937 |
| migration-path-pins.json | c75741f4072c731c22197838a741ac0334046cfb95c084732254f2b10552d503 |
| runtime-bounds.json | 837e7dff5c1d595872619804b35bb38fbc68209d926615c16dcd538363d7db93 |
| target-release.json | 36e07e8617d09cf34cf0b23a236029f1e51f9a0cbabc79dc2ddbc6df3e82cb01 |
| full-release.json | c7b7af2cda597db6eb4716554b8436dfd7c16b47d402287e9111e7e218531f95 |

Feature commits for serial Root intake are `5f85e06f` (facade/harness),
`d48edb83` (object-shape correction), `58a30102` (authorized prerequisite
provenance documentation), `09395dd0` (actual fixture deadline layers), and the
final evidence-only documentation commit. Root's existing `591d908a` is the
prerequisite; branch import `ecb08815` must not be duplicated on a master that
already contains it. Feature changes stay within the original thirteen paths;
the exact five prerequisite paths are separately authorized above. Final path
and cross-directory import checks found no frontend or other application-owner
writes, no backend/frontend coupling and no changes to `docs/source/` Git object
identities. Only Root publishes to master; remote publication and actual Root
postimage verification are a subsequent handoff obligation.

This is a component handoff, not BE33 product acceptance. Remaining real target
provider/routing/public flow checks above remain **NOT_RUN**. The persistent
goal API still reports its aligned historical **BLOCKED** status; the resumed
work was delivered without falsely completing or recreating that goal.
