# BE37 owner relationship observations

Status: `IN_PROGRESS`. Source and test harness are authored. Rust compilation
passes, but transport runtime qualification found the shared method-policy guard
rejects Concert-issued actor-bound reads at service construction. A narrowly
scoped platform fix is requested from delivery. Native PostgreSQL remains
`NOT_RUN`; the build lease was released for this source fix.
This is an identity/privacy prerequisite, not acceptance of BE37 or IN03.

## Scope and provenance

- Assigned checkout: `/Users/lhs/Desktop/mcr-worktrees/be-relationship-authority`,
  branch `codex/mcr-be-relationship-authority`, base
  `9f72a71a99091fe89411b180af3404be85d963f1`.
- Existing BE37 domain/application policy prerequisite:
  `bcd2cd2`, applied locally as `cbdfcfd`. Delivery-owned test dependencies:
  `20503f4`, applied locally as `dd15c78`.
- `Field::SafetyInteraction` and its exact comment are copied from
  `b71ae36a06d8c87cfe48c140ac52cd58e7741d67`, source and destination
  `backend/crates/concert-domain/src/relationship/mod.rs`, local prerequisite
  `ed2ee0f`. The associated social implementation and service policy are excluded.
  Delivery composes the original peer source; do not replay this duplicate delta.
- All new owner application/persistence/RPC/client logic is authored for MCR.
  The portable `backend/proto/relationship/v1/relationship.proto` is a separately
  granted internal contract; existing source protos are unchanged.
- Private fixture ownership checks follow the existing in-repository
  `migration_smoke.Cluster` and bootstrap/Auth-identity test conventions. The new
  harness imports the existing migration and receipt helpers. No Bigyeting UI,
  identity store, or historical rendering code is copied. `docs/source/` is unchanged.
- Runtime dependencies remain the retained workspace resolutions: chrono, UUID,
  async-trait, SQLx/PostgreSQL, Tokio, tonic/prost and shared platform guards.
  New dev edges to Auth/Friend service libraries are delivery-owned; no normal or
  build dependency from either owner reaches Concert service.

## Boundaries and protocol

Auth owns actor/subject account status and deletion state, and the actor's exact
refresh-token family. Friend owns both block directions and local deletion
proof. Neither service reads the other's database. Concert invokes the two
isolated generated services through `GrpcRelationshipAuthority` with the
Concert workload credential and body-bound actor delegation. Method policies
admit only Concert to `ReadCurrent`; existing service policies remain unchanged.

Every signed query binds actor, event, subject, session family, request ID,
explicit purpose and absolute expiry. Purposes are `PUBLIC_PROFILE`, `PRESENCE`,
`INTERACTION`, `SAFETY_SELF`, `SAFETY_INTERACTION`, and `PRIVATE_HANDLE`.
`SAFETY_SELF` additionally requires actor equal subject. Purposes describe a read;
they confer no producer ownership, consent or authorization to mutate data.
The complete query is echoed and compared exactly. There are no retries, caches,
provider identifiers, private profiles, access tokens or error-detail forwarding.
Successful observations require `cache-control: no-store`.

The owner handlers require sealed `VerifiedCaller`, `VerifiedActor` and
`VerifiedReadAuthority` from the shared method guard. The original signed wall
expiry and monotonic call deadline survive every await. An outer request stop
and a two-second maximum bound the entire operation, not two seconds per owner.
SQL reads use a read-only transaction, statement timeout and cancellation polling;
results are checked again after commit and before response. Time expiry alone
also invalidates session liveness; Auth clips validity to the latest matching
nonrevoked refresh-token expiry.

Production `RequestBinding` can currently be created only from a sealed verified
Gateway bootstrap context, with matching actor and event. This prerequisite does
not invent another public route or raw actor/family constructor. Future projection
producers need their own reviewed sealed context integration. Public/presence
eligibility still requires producer-side event membership, pair restrictions and
field privacy. All other purposes are refused by `permits_public_projection`.

Auth and Friend have separate UUID epochs and positive safe-integer revisions.
The result preserves the vector `(Auth epoch/revision, Friend epoch/revision)`;
it never packs, hashes, truncates, takes the maximum, or converts it into BE37's
scalar `FreshAuthority.revision`. `ensure_current` compares an independently
fetched current vector plus the exact request scope and expiry. Comparing an old
observation to its own vector does not fetch current owner state.

## Database semantics and limits

Auth migration 0014 installs a permanent revision singleton and statement-level
transactional bumps on every INSERT/UPDATE/DELETE/TRUNCATE of `users` and
`refresh_tokens`. Friend migration 0015 does the same for `user_blocks` and
`friend_user_deletion_tombstones`. Revision identity cannot be deleted, truncated,
reinserted, have its epoch replaced, or be updated without advancing exactly one.
Overflow fails closed. Owner snapshots read revision, epoch, clock and all relevant
facts in one PostgreSQL MVCC statement. Rolled-back changes do not advance the
revision; committed changes do.

These are coarse service-wide revisions. Unrelated writes invalidate observations,
and owner writes contend on a singleton; production throughput/deadlock recovery
is not qualified here. UUID epochs distinguish fresh database recreation, not
rollback to an old backup that preserves the same epoch. Operational restore and
privilege-change invalidation require separate qualification.

Most importantly, an owner may revoke immediately after a snapshot. Two remote
reads cannot fence that revocation through a Concert transaction. The two-owner
snapshots need not represent one globally atomic instant. The result is therefore
a bounded observation for projections only. This adapter does **not** implement
`FencedRelationshipAuthority`, produce `FreshAuthority`, or enable private reveal,
consent mutation, block commands, inventory effects or other protected writes.
Production mutation/reveal authority stays unavailable until a distributed fence
protocol is implemented and independently qualified.

## Composition requirements

Auth/Friend startup registers their new isolated service on the existing guarded,
TLS-capable server. Concert is an optional accepted workload in owner configuration;
no credential means Concert is not accepted in production. Deployers must provision
its dedicated service credential and trusted Concert actor-delegation public key
in both owners. Constructor TLS parameters use the existing owner-audience checks.
No production credentials, TLS deployment, receiver key rotation or new Concert
product router composition are qualified by loopback fixtures.

## Validation and handoff

Source-only checks completed with exit 0:

- `python3 -m unittest discover -s backend/scripts/mcr -p test_owner_relationship_smoke.py -v`
  — nine refusal, interruption, cleanup, private report, skipped-test, artifact rejection and binary-pinning checks.
- `python3 scripts/mcr/check-plan.py` — planning/provenance baseline passes;
  it does not certify this implementation.
- Rust 1.88.0 `rustfmt` on assigned new modules/tests/build files; `git diff --check`.
  The machine's unpinned stable rustfmt has a missing dylib; pinned 1.88.0 succeeds.

Application suites cover binding shape, exact purposes, cancellation, deadlines,
malformed snapshots, revision limits, redacted diagnostics, and all eligibility
flag combinations. Owner transport suites exercise generated services and actual
workload/signature guards. Concert transport tests use a genuine guarded Gateway
bootstrap to construct the binding, then production signed clients against guarded
loopback owner services. Controlled replies cover scope mismatch, malformed epoch,
revision and timestamps, missing no-store, outages, delay and every restriction.
These are authored expectations until executed with Cargo.

The native harness selects exact executables from successful Cargo JSON receipts;
it never globs old binaries. In a fresh marker/inode-owned PostgreSQL 16.14 root it
creates only `mcr_auth_relationship` and `mcr_friend_relationship`, runs each owner's
real migration CLI and exact ledger/reapply checks, and requires one successful
non-skipped native test per owner plus the real two-owner Concert client test.
The tests cover rollback isolation, committed restrictions, session revocation,
block/unblock generations, tombstones, revision permanence and bounded lock waits.
Missing fixture ownership must fail before database access. PG restart must
preserve epochs, revisions and denial facts; cleanup must remove the owned root.

Pending lease commands use Rust 1.88.0, target
`/Users/lhs/Desktop/mcr-build/backend`, `--locked --offline -j2`, and both
`CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0`:

1. Build/test Auth/Friend application `concert_relationship` suites and three
   service integration suites (`concert_relationship` for Auth/Friend,
   `relationship_authority` for Concert).
2. Build Auth/Friend bins with Cargo JSON output, preserving the exact successful
   build/test artifact metadata in a private file.
3. Run `backend/scripts/mcr/owner_relationship_smoke.py --cargo-json ABSOLUTE_JSON
   --pg-bin /opt/homebrew/opt/postgresql@16/bin --report NEW_PRIVATE_REPORT`.
4. Record exact exit codes, source hashes, test counts, report hash, restart and
   cleanup before claiming runtime qualification or delivering reviewed commits.

The persistent goal was inspected and remains `blocked` from the earlier broader
identity assignment. Delivery explicitly retained that aligned goal while this
prerequisite is source-active; the goal is not falsely completed or cosmetically
recreated. Runtime status, committed handoff and review fixes remain required.

## First runtime checkpoint

Private diagnostics: `/tmp/mcr-owner-relationship-validation.7uZzuj`.
All builds used the pinned toolchain/profile/target above, with exit 0:

- `cargo test --locked --offline -j2 -p auth-application -p friend-application -p auth-service -p friend-service --test concert_relationship --no-run --message-format=json` — 24.98 seconds.
- `cargo test --locked --offline -j2 -p concert-service --test relationship_authority --no-run --message-format=json` — 16.12 seconds.
- `cargo build --locked --offline -j2 -p auth-service -p friend-service --bins --message-format=json` — 9.80 seconds.

Exact application test executables selected from these receipts passed five tests
each (10 total). Auth/Friend's two transport tests each and Concert's three
transport tests failed before dispatch with the same registration error:
`acts for a user and must accept only the Gateway`. This comes from existing
`platform::grpc_method_policy::MethodPolicy::validate`, not owner persistence.
Native tests remain ignored until the explicit owned harness runs; compilation
is not a runtime success claim. No database process was started by this chat.

Delivery was asked to supply an exact exception for the two new internal owner
service names/ReadCurrent, matching owner workload, Concert-only caller, and actor
field 1, while retaining all other Gateway-only constraints. No platform file was
modified without a grant. The lease was released immediately after collecting
the failure; the next worker can proceed while this prerequisite is corrected.
The harness now copies all selected binaries, verifies SHA-256, and uses only its
private copies during native execution so it can release the shared target first.

## Dedicated read-proof correction (source checkpoint)

Delivery granted the exact additional paths
`backend/crates/platform/src/grpc_method_policy.rs`, its new dedicated
`grpc_method_policy/relationship_read_tests.rs` and exact test module line,
`actor_delegation/relationship_read.rs`, and one export in `actor_delegation.rs`.
The reserved registration exception requires the exact Auth/Friend read service,
matching owning workload, only `ReadCurrent`, only Concert, and only actor field 1.
All other actor-bound registrations remain Gateway-only; reserved names reject
extra methods, wrong workloads/callers/fields and missing actor declarations.

`RelationshipReadSigner` loads `CONCERT_RELATIONSHIP_SIGNING_SEED` with no
fallback and refuses Gateway signing-seed reuse or any key trusted by the supplied
Gateway verifier. `RelationshipReadVerifier` loads the separate
`CONCERT_RELATIONSHIP_VERIFYING_KEYS` ring and rejects overlap with the existing
Gateway ring, including rotated keys. Both refuse the shared parity key. Environment
keys use standard Base64, matching the existing platform's key encoding.
The owner startup composition uses the dedicated verifier **only** for the new
read service. If the dedicated verification variable is absent, the new route is
not registered; malformed/empty/overlapping configured keys fail startup. Existing
Gateway service registration and verification remain unchanged.

The Concert client constructor now requires the typed dedicated signer; explicit
fixtures use separate read seed 37 and Gateway seed 41. New tests attempt to forge
the other issuer string using each key, so a string-only issuer check cannot pass
cross-domain tests. Other tests cover missing/wrong/tampered/actor/audience/method,
replayed and expired proofs, strict registration, and dedicated environment loads.
These are source-authored expectations pending the next bounded Cargo lease.
This remains trusted Concert-signed observation, not forwarded Gateway child
proof and not a remote database fence. The sealed bootstrap binding and current
owner session check remain required. No platform runtime claim is made yet.
