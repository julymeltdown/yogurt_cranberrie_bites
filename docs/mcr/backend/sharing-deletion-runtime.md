# Sharing deletion production composition

Status: **Real factory/Auth worker cohort PASS; production main mount open** (2026-11-05 KST).

The already qualified Sharing producer (`4592a705`) remains unchanged. This task
connects its real publisher through the existing production deletion factory and
checks the Auth worker/source/Apply/store boundary. It does not repeat the old
producer-only cohort or replace the publisher with a successful fixture.

Root transferred the exact main input hunk, optional Sharing forwarding in
`config/account_deletion_runtime.rs`, new `tests/sharing_deletion_runtime*.rs`,
`backend/scripts/mcr/sharing_deletion_runtime/**` and this document. The existing
factory already accepts `LiveDeletionOwners.sharing_publisher`, so its source
needs no modification. Main retains `policy.publisher.clone()` from the incumbent
`SocialWorldPolicy`, before moving that factory's transport and sink into the
actual Social/World composition. There is one pool, publisher and registry. Off
mode leaves the typed input absent. Commit `91d16699` provides that exact input;
207 owns the typed deletion composer, actual World ready handle, consumption of
this input and mounting one guarded Apply service on the existing listener. This
input commit alone does not prove that main has mounted deletion.

The new `sharing_deletion_runtime` test imports the existing authenticated RPC
fixture helper and canonical marker/inode/PG-identity environment guard read only.
The production `AccountDeletionRuntime` receives the exact incumbent publisher
Arc. The actual Auth service performs signup, email verification, login, deletion
and persistent worker dispatch. SOURCE and Apply run their real guarded mTLS
handlers. The fixture parks the other seven required saga steps by scheduling
only; it never removes a target, inserts a success receipt or claims aggregate
BE42 completion.

The focused SOURCE round covers an actual foreign serving-membership row missing
from the incumbent registry. Publication refuses, the Sharing worker step retries,
and its committed deletion progress remains incomplete. A documented storage
transition disconnects that fixture membership; a new worker dispatch completes
Sharing, revokes both incoming and outgoing requests/grants, erases the subject's
encrypted handle, preserves the peer's handle and consent history, and replays the
same committed receipt. Ordinary command receipts and outbox counts stay fixed.
The membership transition is not proof of actual WebSocket disconnect or frontend
cache removal. Social rows use actual opaque AES-GCM fixture material and a short
storage guard; they establish no Intro witness, admission, age or consent grant.

A separate real worker dispatch is cancelled only after SQL activity shows its
factory call waiting on the common deletion advisory lock. Its original short
clock expires while the lock remains held. The transaction leaves no fence or
Sharing receipt and the saga never records success. A fresh bounded worker round
recovers that durable lease and commits through the real factory. This exercises
the composition boundary without substituting a positive responder.

The native runner creates a new private PG16 cluster with actual full Auth and
Concert SQLx ledgers and private PKI, then physically stops/restarts PostgreSQL.
The RESTARTED round uses a fresh test PID, freshly composed Auth/source/Apply and
Social factory, verifies exact stored receipt replay and retains seven pending
saga targets. It requires a clean frozen source, copied test executable hash,
source and native-tool readsets, frozen migration hashes, explicit absolute paths
and an external finite deadline. It neither invokes Cargo nor accesses a target.
The independent lease watchdog receives marker/inode/token custody before PG
children start. Zero prepared transactions are checked before restart and teardown.

Provenance for the small adapted worker/identity/scheduling/cancellation helpers:
`backend/services/concert-service/tests/account_deletion_runtime.rs` at
`cf1ca629a33ece977f8b5b1b6faf6152b35e5dbd`, SHA256
`5da1dfd25d286b480a94177f41211ea0cd1a1116c5f687a91e133721bc5374d6`.
The opaque AES-GCM seed and guarded Social row setup derive from
`backend/services/concert-service/tests/social_account_deletion.rs` at the same
revision, SHA256
`e59fee93de180b4e44e439a4cea4382dcf2d3d361da13da2c4767b7a138b35df`.
Existing read-only support paths are named in the new test. The Python runner
reuses `migration_smoke.Cluster`, `invalidation_clients_smoke.pki` and the Social
native source manifest, all in the frozen readset. No dependencies, migrations,
contracts, manifests or lockfiles change.

Validation commands (the build below ran as part of the five-test cohort):

```sh
cargo test --manifest-path backend/Cargo.toml --locked --offline -j 2 \
  --config profile.dev.package.contracts-build.opt-level=2 \
  -p concert-service --test sharing_deletion_runtime --no-run --message-format=json
python3 -B backend/scripts/mcr/sharing_deletion_runtime/qualify.py \
  --artifacts /ABS/copied-artifacts.json --pg-bin /ABS/pg16/bin \
  --report /ABS/new-sharing-report.json --deadline-epoch FINITE_EPOCH
```

Rust formatting and Python AST parsing passed. The fresh native disposition below
records the separate compilation, RPC, SQLx, worker and restart evidence. Root is the named publishing owner;
207 is the implementation counterpart for main mounting. Neither handoff waits
for king/SL review, acknowledgment or a ledger update.


## Actual validation disposition

The complete source `46defd2346e2e73641c552b408623ce357500c15` compiled with
Rust 1.88, locked/offline, jobs=2, dev/test debug=0, incremental=0, no Rust flags or
wrappers, contracts-build opt-level=2: exit 0. The Sharing executable was copied
and frozen before target release. This used the fresh finite
`SOCIAL-SHARING-MEDIA-202611041753Z` allocation, independent guardian and original
clock; the target was not accessed during native checks.

Both SOURCE and RESTARTED invocations passed, exit 0. SOURCE took 8.628 seconds;
RESTARTED took 8.001 seconds. Both used the production Auth/source/Apply handlers,
actual canonical repositories and the real incumbent Social factory. The positive
Sharing step, refusal/cancellation non-ACK, exact cleanup and immutable replay
assertions all passed. Actual full SQLx ledgers matched frozen inputs: Auth 18,
Concert 47. Actual postmaster PIDs changed 78937 to 80666. Fresh test PIDs and
postmaster start timestamps differed. Prepared transaction counts were zero before
physical stop, after replay and before final teardown. Seven other saga targets
remained required and pending; aggregate BE42 is not accepted here.

Report `/private/tmp/mcr-social-sharing-f_ut5nyi/sharing-report.json`, SHA256
`653d3019eb4883568422ff40bb73d0e8b3e03887e62f7c3babdfc02e3540c188`.
The encompassing corrected Social+C03 component+Sharing copied cohort passed in
33.475 seconds; see `social-policy-runtime.md` for source/artifact and release
hashes. All owned clusters and RPC processes stopped; FULL release was
02:58:36.864049 KST, with all future reservations zero and the guardian exited 0.
The source remains a component/real-service boundary qualification. Main mounting,
public Gateway and actual socket/cache removal are still separate explicit gates.
