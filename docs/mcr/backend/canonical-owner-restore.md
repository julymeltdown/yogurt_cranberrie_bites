# BE15/QA09 canonical durable owner backup/restore segment

Status: IN_PROGRESS. Source committed and actual native restore qualification PASS (3/3); Root integration/publication pending.
This is a supporting segment, not acceptance of full BE15 or QA09.

## Owned scope and preserved protocol

New worktree/branch: `be-canonical-owner-restore` /
`codex/mcr-be-BE15-QA09-owner-restore`, based on qualified CF01
`3afb8111d8ef206f3e73c63186699c98d9fd4997`. Only the new
`tests/canonical_owner_restore.rs`, new `tests/canonical_owner_restore_support/`
and this document are writable. Production composition, main, registry,
protocols, migrations, manifests, previous fixtures and QA09 central recovery
scripts are unchanged. Root retains production composition and /me ownership.

The existing persistent full canonical-owner goal is aligned and reused. Its
historical metadata was BLOCKED on deferred intake; this assignment explicitly
resumes source execution, without falsely claiming ACTIVE or resetting it.
Completion still requires the larger owner/protocol/runtime/master evidence.

## Physical boundary and cases

The runner creates a new marker/inode-owned socket-only PostgreSQL 16 cluster
with five isolated owner databases and actual mTLS owner transports. A source
Root test process constructs the real CanonicalOwnerComposition, acquires the
actual Auth/Friend scope, and retains actual Local/Profile prepared children.
It persists the original registered native/child state and a private operator
handoff. The controller SIGKILLs/reaps that exact process, stops/reaps the exact
source postmaster, copies the entire stopped data directory into a distinct
backup, verifies every regular file hash, then restores the backup into another
fresh marker/inode-owned directory. Only one postmaster runs at a time.

The restored process is a distinct PID. Every database OID and system identifier,
actual pg_xact state, original prepared GIDs/XIDs, encrypted child references,
producer UUID and independently retained original recovery keys must match. New
transport endpoints do not grant an expired request authority. Rotated recovery
keys must retain the exact original key; missing-key and wrong-instance attempts
cannot ACK a child or release a fence. The immutable backup must remain byte
identical while recovery runs against its restored copy.

| Case | Actual source state | Required restored outcome |
| --- | --- | --- |
| FINISHED | Actual native registration rolled back; real outer body EOF finishes Root; negative Profile Complete retains Profile after the earlier Local child has actually settled | Rotated keyring retaining the original lets the real composite worker settle the remaining actual prepared Profile child; repeated recovery adds no ACK/effect; all native prepared work reaches zero |
| ACTIVE | Actual owner scope and children; inner serialization only, no outer terminal proof | Original expiry and restart do not finish Root; owner recovery and child worker retain all four native fences and both pending children |
| MISSING_WITNESS | Actual registered transaction commits a test-owned effect once; actual outer EOF finishes Root; native resolver cannot certify a commit witness | Committed SQL remains committed/one effect; authenticated owner-to-root resolver calls return UNKNOWN, owner registrations remain PENDING/fenced; eligible children may settle, and repeated recovery creates no duplicate effect |

All cases reject restored in-memory authority and DeliveryCompleted when the
original registry was lost. Child settlement is not reconstructed delivery or
fresh content permission. No test manually rolls back prepared transactions or
writes lifecycle phases/ACKs; production recovery performs supported rollbacks.
Negative cases retain quarantined prepared work until exact owned-cluster
teardown, which is cleanup rather than native settlement proof.

## Backup rationale and production prerequisites

PostgreSQL requires a shutdown for a plain file copy and a whole cluster because
transaction status in pg_xact belongs with the table/catalog files. Its prepared
transaction state spans WAL/shared state and pg_twophase at checkpoints. The
harness chooses the explicit offline full-copy method; no pg_dump or live-copy
consistency is inferred. See [PostgreSQL 16 file backup](https://www.postgresql.org/docs/16/backup-file.html)
and [two-phase state](https://www.postgresql.org/docs/16/two-phase.html).

A production committed-effect resolver needs the actual producer-specific
same-transaction receipt/witness; PgNativeAbortResolver intentionally cannot
supply it. The missing-witness case tests that refusal, rather than fabricating
a receipt or widening factory ownership. The test-owned SQL effect counter is a
native durability/no-reexecution observation, not a domain outbox consumer or
business pickup receipt. Separate owner clusters need a coordinated operational
backup/recovery plan; this harness covers one exact five-database cluster.

## Completed finite validation

The exact compile command was:

```sh
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 -p concert-service --test canonical_owner_restore --no-run --message-format=json
```

The environment selected the allocated BACKEND target, debug 0 for dev/tests,
and incremental off. Initial build at `d5209a06b0bb5be4104dfa1fdcd8fcc1a041e322`
passed (exit 0, 40.13 seconds). Peer review then identified the real Local-before-
Profile completion order, insufficient per-owner UNKNOWN observations, and
cleanup exception ordering. All three were fixed in
`1f1b60769e9c3387c70b80707ea33e0179902f8c`; the bounded rebuild passed
(exit 0, 5.38 seconds). The peer's second readback found no remaining issues.

The frozen executable was 50,427,392 bytes, SHA256
`1a209ec00333ec2d68d746d12e47a988479d69f0a306dd94181f36a2ebcb940c`.
All 3,534 source/runtime pins matched that commit's Git blobs, including the
selected source artwork and actual baked migration inputs. Target handles were
verified absent and the target released at 2026-11-04 08:36:21 UTC before any
copied test or PG launch. No target access occurred afterward.

The copied qualifier command used the exact owned checkpoint and explicit
PostgreSQL 16 binary directory:

```sh
python3 backend/services/concert-service/tests/canonical_owner_restore_support/qualify.py --checkpoint "$EVIDENCE/checkpoint.json" --pg-bin-dir "$PG16_BIN" --report "$EVIDENCE/native.json"
```

The finite private guardian added directory identity tracking, PG16.14 checks,
one-postmaster-at-a-time enforcement, a whole 2 GiB allocation bound (runtime
included), an 8 GiB free-space stop floor, a cumulative copied-run window and
cleanup reserve. Initial fresh free space was 33,340,325,888 bytes. No package
cleanup was performed because fresh cached-source mismatch was not confirmed.

Two unsuccessful launch attempts are preserved. Attempt 1 failed before any
cluster allocation because the private bootstrap lacked the new controller
import path. Attempt 2 reached owned PG16.14 but timed out waiting for the first
exact whole-line marker: verbose libtest prefixes the first output line with
the test name. Both clusters were cleaned up. Attempt 3 passed with the same
frozen binary and exact marker matcher after the private runner selected
`--format terse`. The committed controller successor records that identical
argument for portable reproduction. This is a Python-only command correction;
all Rust, manifests, migrations and compiled inputs remain byte identical to
`1f1b6076`. The accompanying provenance JSON update records qualification
metadata only. No additional compilation or unrecorded retry is claimed.

| Actual native case | Exit / elapsed | Prepared counts before teardown (Auth/Friend/Concert/Profile/Notification) | Observed durable result |
| --- | --- | --- | --- |
| FINISHED | 0 / 9.654 s | 0 / 0 / 0 / 0 / 0 | Remaining Profile intent recovered with retained original key; both original immutable intent rows preserved; repeated drains add no effect |
| ACTIVE | 0 / 9.372 s | 1 / 1 / 1 / 1 / 0 | All four actual fences and both pending children remain; expiry/restart never invents terminal proof |
| MISSING_WITNESS | 0 / 9.420 s | 1 / 1 / 0 / 0 / 0 | Actual native SQL remains committed with exactly one tracked effect; each owner's authenticated exact-transaction RPC successfully returns UNKNOWN with empty receipt; both registrations remain PENDING |

Every case copied and verified every whole-cluster regular file before startup,
preserved all five system-identifier/database-OID pairs and exact prepared
GIDs/XIDs, verified the backup stayed unchanged during recovery, then ran a
distinct restored Root PID. Migration vectors were Auth 18, Friend 18, Concert
37, Profile 9, Notification 6, with actual sqlx checksums checked in restored
SQL. Missing retained-key decryption was exercised on the two finished-root
cases. ACTIVE does not dispatch a child, so it cannot establish a decryption
failure even though no missing-key completion occurs.

The successful guard exited 0 in 31.06 seconds. Peak whole additional allocation
was 418,537,472 bytes, including peak runtime 313,905,152 bytes and frozen target
growth 51,200,000 bytes; runtime was not an extra allocation. Both unsuccessful
attempts remained within the same original offer and cumulative time/storage
bounds. Full release at 08:40:36 UTC verified all eight allocated cluster roots,
all six successful-case test hosts, the controller and its process group absent.
No negative-case teardown is counted as native settlement.

Private receipt directory basename: `mcr-owner-restore-p5s8vs96`. Exact hashes:

| Receipt | SHA256 |
| --- | --- |
| `native.json` (3 cases PASS) | `75d2a1be3b3f752ff4da6a1bfdfe731418fd8aa0f14bed8aca062e503f276046` |
| `runtime-guard.json` | `6b13d9e2c7f869a07a6768ab254f03e4b1bc88f3b30964fda334283a01256579` |
| `source-runtime-audit.json` | `6596ae387ebbf43e54180d0092e61979887bee867eb8a5fc9b88ba8b5418818b` |
| `target-release.json` | `797c8ade09a6fbea4c7336499ff707b9ab43d48b5380d30a3f5d0afa886ab5d4` |

Planning checker, scoped Python parse, rustfmt 1.88 and diff-whitespace checks
passed with exit 0. These static checks are separate from the actual native
qualification above. Adapted-code provenance records original paths/revisions,
SHA256 values, destination paths and dependencies in the adjacent provenance
JSON. Source ancestry is the CF01 base `3afb8111` followed by `f423ee8a`,
`d5209a06`, `1f1b6076`, and the helper/evidence successor. The delivery receipt
records all twelve changed-path postimages and exact commit IDs for serial
Root integration. No production composition, public inbox/provider behavior,
domain outbox/pickup, deletion or whole BE15/QA09 acceptance is claimed.
