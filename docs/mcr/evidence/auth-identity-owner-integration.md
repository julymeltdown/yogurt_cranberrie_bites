# Auth owner admission read integration

Date: 2026-11-03. Worker `6630fc4`, base `5863f91`, integrated as `c300a23`.
The parent-owned binding epoch migration was already present as `607a903`;
the worker's prerequisite cherry-pick was not duplicated. All nine assigned
paths match the reviewed bytes: [inventory](auth-identity-owner/hashes.json).

The new application port and real PostgreSQL adapter read active account,
exact user/session-family liveness and an optional exact provider row/type/owner
epoch in one MVCC statement. The database observation clock is sampled during
execution, including after relation-lock waits. Ordinary admission omits the
provider relation entirely. Optional OAuth bindings reject legacy missing,
blank, control-bearing and oversized subjects inside the database without
projecting their values. EMAIL verification is separate from OAuth binding.

Parent review accepted the exact two one-line module exports. The generic lane
checker cannot express explicitly assigned backend smoke-script exceptions;
the parent instead compared all nine paths and inspected imports and scope.
This does not grant general script, composition or opposite-lane ownership.

On clean candidate `c300a231506cfcd08b25697234e2e427c426fca9`, the parent ran
locked/offline Rust 1.88.0 with two jobs and the retained target:

- Auth service binary and native repository-test compilation passed.
- **8** deterministic admission-value tests and **23** retained application
  tests passed, alongside **89** Python smoke-helper tests.
- **22 real PostgreSQL 16.14 tests** passed in a new marker/inode-owned cluster.
  All **13 SQLx migrations**, exact ledger checksums, reapply, restart persistence
  and cleanup passed. Missing fixture ownership is a hard failure, not a skip.
- Parent independently confirmed the owned directory was absent after cleanup
  and every recorded source hash matched the integrated candidate.

The [native receipt](auth-identity-owner/native.json) records the clean SHA,
source/helper/migration and artifact hashes, command exits and diagnostics.
Deterministic test logs accompany it. Cases include retained production relink
and refresh-family repositories, live rotated successors, foreign identities,
epoch invalidation/reuse/overwrite/exhaustion, coherent committed snapshots,
expiry after pool and statement waits, original deadlines, cancellation and
single-connection cleanup with statement-timeout restoration.

The two-second ceiling bounds internal resource use; it is not a revocation SLO
or admission lifetime. Local cancellation does not promise instantaneous server
cancellation. Facts describe one database snapshot, not a lease or signed proof.
Later committed revocation requires another read. Sequence restore/privileges,
authenticated RPC/Gateway/Concert composition, event restrictions, production
invalidation and real provider login remain **NOT_RUN**. No HTTP/RPC/server
registration or production accepting policy is added.

The bounded owner-read goal is approved for completion; **BE04 remains
IN_PROGRESS**. See the [worker record](../backend/auth-identity-owner.md) and
[admission decision](../identity-admission-decision.md) for the remaining boundaries.
