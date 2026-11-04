# QA09-BE recovery orchestration and owner observations

Status: IN_PROGRESS. Twenty source regressions and the fresh physical recovery,
owned-restart and read-only SQL supporting qualification passed. Normal Root
publication/readback remain pending. Original QA09 remains open.

## Assignment and original criteria

Root transferred these three exclusive new paths after the previous provisional
writer stopped without authored files. Existing worktree `qa-contract-coverage`,
branch `codex/mcr-be-QA09-backend`, fresh fetched/remote-verified base
`2bd046796acdf32a05e6ed04e4d95e12c9909481`. Older task branches are preserved.

- `scripts/mcr/recovery/qa09_backend.py`
- `scripts/mcr/recovery/test_qa09_backend.py`
- `docs/mcr/backend/qa09-backend-recovery.md`

The specification's original QA09 requires database restart/backup restoration,
outbox replay, pending pickup, deletion and partial event closure: restored state
must remain consistent, committed pickup must not be lost, and shutdown/cleanup
must be resumable. Its prerequisites are BE44, BE42 and FE39. This supporting
backend delivery does not amend those criteria or accept whole QA09.

The frontend counterpart uses the same six original case IDs. IN03 and World
harnesses retain their existing owners; this patch does not replace them. Root
retains all main/configuration/SQL/manifests and production wiring. No old shared
helper, application code, migration, frontend or source archive is modified.
Developers own validation and fixes; the named Root developer publishes the
validated handoff, with no king, SL or separate QA approval stage.

## Existing owner interfaces and provenance

All adaptation here is newly authored standard-library Python. No existing
feature harness or authority implementation is copied or replaced. The physical
restore action invokes the unchanged existing qualified runner through its
`run(args, report)` interface, preserving its original real owners and guards.

| Existing source at the base | SHA-256 | Use |
|---|---|---|
| `backend/scripts/mcr/migration_smoke.py` | `a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8` | Original live `Cluster`, marker/inode/postmaster ownership checks, exact SQLx ledger verification |
| `backend/services/concert-service/tests/canonical_owner_restore_support/qualify.py` | `287df4a176cb803eb036d732534a8a6c924662f89b50d9d1ec0bfd73cf253a63` | Existing PG16 physical restore orchestration; same-checkpoint verification before/after |
| `backend/services/concert-service/tests/canonical_owner_restore_support/controller.py` | `440635a8abdc2c918aa244e93b5596456da1cce79151a306c3f067530c1910ad` | Actual source SIGKILL/reap, stopped full-cluster backup, every-file verification, distinct restored process |

Those sources already record original CF01/restore extraction provenance in
`canonical_owner_restore_support/provenance.json`. Dependencies remain the locked
Rust restore target, explicit PostgreSQL 16 tools, OpenSSL for existing test mTLS,
and Python's standard library. No packages, manifests or retained resolutions change.

Traced production interfaces include Concert `workers/outbox.rs::run_batch`,
`workers/cleanup.rs::cleanup_tick`, and Auth
`concert_deletion::ConcertDeletionDispatcher::dispatch`. Existing maintenance,
giveaway and lifecycle native fixtures include seeded rows or test authority;
this patch does not run them as evidence of an actual authorized product flow.
The restore runner's test-owned native effect is likewise not a business receipt.

## Commands and honest case dispositions

`plan --report /absolute/private/new-report.json` produces all six cases as
`NOT_RUN`. An optional `--run-id` accepts the shared 32-character lowercase hex
coordination ID. It never embeds user IDs, URLs, request bodies or tokens.

Under a separately accepted 21d native allocation, `restore-segment` additionally
requires `--checkpoint`, `--pg-bin-dir`, `--native-evidence` and `--report`.
Checkpoint/tools must be explicit absolute paths. The native evidence and report
are distinct new files in the same private directory; writes are exclusive and
mode 0600. The checkpoint must satisfy the existing exact single-target
`canonical_owner_restore` build/source/fixture/binary verification, plus
`qa09_source_sha256` containing both new Python script hashes. The frozen binary
must be outside any target directory. Build and target release precede native runs.

| Case ID | Current executable support / remaining evidence |
|---|---|
| `QA09-DB-RESTART` | Read-only owner snapshots can compare durable state. Same-owned-cluster restart helper verifies physical lineage and actual prepared work; actual same-candidate application/business restart remains NOT_RUN. |
| `QA09-BACKUP-RESTORE` | Existing runner's FINISHED/ACTIVE/MISSING_WITNESS physical restore segment is delegated unchanged. Segment PASS only changes its case to IN_PROGRESS; actual domain recovery remains open. |
| `QA09-OUTBOX-REPLAY` | Observer detects loss/change of delivered outbox rows. Actual downstream deduplication and consumer/business receipt binding remain NOT_RUN. |
| `QA09-PENDING-PICKUP` | Observer detects lost/reverted CLAIMED rows, claimed-quantity loss and inventory overflow. Actual pickup bridge and authorized pending/committed pickup recovery remain NOT_RUN. |
| `QA09-DELETION` | Observer detects regression/loss of successful deletion steps. Actual canonical saga recovery plus reconnect/reveal fencing remain NOT_RUN. |
| `QA09-PARTIAL-CLOSURE` | Observer detects loss of committed owner effects and revision regression. Real privileged operator/closure provider is unmounted: positive execution remains NOT_RUN. |

No SQL seeding, fake provider/Auth, manual lease/state edits, forced prepared
rollback, inferred privileged authority or substituted success is introduced.
The actual Root developer must expose missing production owner capabilities;
observer availability does not grant them. Whole QA09 and whole-case acceptance
are always false in these supporting reports.

## Read-only observer interface

An actual implementation owner may call
`capture(cluster, owner, database, scope, private_key)` inside its accepted native
allocation using its original live `migration_smoke.Cluster` instance. Serialized
cluster metadata, URLs, foreign objects and attaching existing roots are refused.
The owner is `concert` or `auth`; the database has a closed isolated `mcr_` name,
and event/user scope is a canonical nonnil UUID. Exact source migration receipts
and physical system/database lineage are checked. The caller keeps a private
run-local key of at least 32 bytes across before/after observations.

Each closed projection uses a bounded read-only repeatable-read transaction,
statement/lock timeouts, and at most 1000 rows; no private payloads, handles,
pickup codes, keys or error text are selected. Each table is a separate snapshot:
there is no cross-table or cross-owner atomicity claim. IDs and physical lineage
become private run-local HMAC labels, never exported raw. Query digests use a
fixed dummy UUID rather than the private scope. Missing schema/projections are
unavailable, never empty successful fixtures. Snapshots are private BE evidence;
FE receives only case-level booleans/counts and a pinned evidence reference.

`compare(case_id, before, after)` rejects source/run/scope/owner/lineage/projection
drift and reversed or naive timestamps. Empty/unavailable baselines stay NOT_RUN.
Nonempty comparisons return IN_PROGRESS observations plus closed invariant
violations, with `wholeCaseAccepted: false` even when no violation is found.
They do not establish an actual fault boundary, authority, downstream dedup,
cleanup completion or browser recovery. Retention/pruning windows require their
actual owner's interpretation; a missing committed row is never silently excused.

`restart_owner_cluster(cluster, private_key)` orchestrates the original owned
Cluster stop/start methods, requires a distinct postmaster, and compares physical
system lineage plus actual prepared GID/XID/database state across that boundary.
Only run-local HMAC bindings, a prepared count, booleans and actual UTC phase
timestamps are returned. No prepared transaction is manually settled; negative
quarantine must remain identical. This is physical restart support, not business
receipt/pickup preservation or original-case acceptance. The caller must run it
within the same accepted native resource guardian and preserve cleanup on failure.

## Coordination envelope

Version 1 is evidence coordination, not a new public/product contract. Fields are
`version`, `runId`, `caseId`, full `sourceRevisions`, relative producer evidence
path plus SHA-256, `phases` (`before`, `boundary`, `recovered`), `state`, closed
`observations`, and limitations. Missing evidence/phase values remain null. The
existing restore controller does not expose a timestamped original domain
boundary, so the wrapper leaves `boundary` null. It pins the actual separate
native evidence bytes; a segment PASS or phase notification never grants credit.
Frontend validates its own expected run/case/source and reads actual pinned bytes.
Browser and backend HMAC keys remain independent and private.

## Validation evidence

Source-only command:
`PYTHONDONTWRITEBYTECODE=1 python3 scripts/mcr/recovery/test_qa09_backend.py`
passed 20 tests, exit 0. These regressions exercise observer loss/overflow,
redaction, binding drift, read-only SQL construction, unsupported metadata,
exclusive private reports and both-script checkpoint pins. Dictionary fixtures
are pure algorithm checks, never native/runtime or product acceptance evidence.

## Fresh supporting recovery qualification

Exact tested source: `73636ad86f3ba42dbf939a6487e557b97d89b5c0` (source commits
`7b59208d`, `320eb6c5`, `73636ad8` on the stated base). This evidence update changes
only documentation. Both Python scripts and all compiled inputs remain byte
identical to the tested checkpoint. Standard-library source tests passed 20/20;
AST, plan CLI, `git diff --check`, and planning checker also exited 0. Planning
validation is separate from application/recovery acceptance.

Fresh resource round `QA09-BE-RESTORE-MEDIA-202611041321Z` actually accepted
2026-11-04 13:23:55.066965 UTC with 33,895,231,488 free bytes. Absolute compilation
stop 13:43:55 UTC, work stop 13:50:55 UTC, hard/full-release stop 13:53:55 UTC.
Rust 1.88.0, locked/offline, two jobs, debug/incremental off, both Rust flag
variables unset, and contracts-build dev opt-level 2 were used. Exact command:

```sh
cargo +1.88.0 test -p concert-service --test canonical_owner_restore --no-run --locked --offline -j2 --message-format=json
```

Build attempt 1 exited 101: cached contract build code supplied the old
`be-world-recipient-producer` protobuf include path. The worker incorrectly
interpreted the original metadata exception as covering
`backend/crates/contracts-build/src/lib.rs` and refreshed that own file's mtime
before the allocator extended that exact exception. The allocator explicitly
extended it **after** the refresh/rebuild/target release. This is not described as
prior authorization. Content SHA, mode, UID/GID and actual before/after times are
retained. Before-inode was not captured; current inode is recorded without
inventing a historical measurement. No content, cache, dependency or fingerprint
was deleted, no further metadata refresh occurred, and no clock was extended.
Rebuild exited 0 in 40.37 seconds. The failed attempt and authority-timing
correction remain in the private evidence.

One exact executable (50,093,744 bytes) was frozen outside the existing selected
MEDIA target with Cargo JSON, complete source/tree/diff, 3712 runtime/source
pins including both QA09 scripts, original fixture checkpoint, library links and
fresh PG tool hashes. Copy/freeze took 1.24 seconds. Unconditional target release
at 13:26:47.856761 UTC recorded zero target handles before any copied/native/PG
execution. There was no target access afterward. Actual owned servers verified
PostgreSQL **16.14** using `/opt/homebrew/opt/postgresql@16/bin`.

The unchanged original controller launched only:

```text
<copied canonical_owner_restore executable> --ignored --exact canonical_owner_restore_process --test-threads=1 --nocapture --format terse
```

The QA09 CLI delegated that original runner. Private tracking instrumentation
recorded its constructor-authenticated owned roots and charged all source and
restored data, backup, PKI, logs, copied executable and proofs. At each restored
cluster's pre-cleanup boundary, the new observer read Auth/Concert projections,
the new helper stopped/restarted that same cluster, and the observer read again.
Original fixture/controller/source files were unchanged. Instrumentation does
not introduce business rows, authority or a replacement feature harness.

Native attempt 1 reached FINISHED plus one restart and two owner observer pairs,
then its private resource monitor raced with normal directory teardown: the
marker had been removed while the exact owned directory still existed. The
monitor interrupted cleanup. The remaining directory was removed only after
matching its constructor-recorded inode/UID and confirming no postmaster PID
file. The failed attempt's paired native/report artifacts and monitor failure
remain intact under `attempt-1/`. The private monitor was corrected to retain
the constructor-authenticated inode binding during marker-removal teardown.
All source bytes remained unchanged.

The retry passed within the **same original cumulative 420-second window**
(deadline 13:34:16.024061 UTC), ending at 13:30:07.069048 UTC. No native clock
reset, target re-entry, source/SQL change or allocation extension occurred.

| Final supporting case | Prepared work before teardown | Result |
|---|---|---|
| FINISHED | All five owners zero | PASS; supported native settlement |
| ACTIVE | Auth/Friend/Concert/Profile one each; Notification zero | PASS; quarantine retained, not settlement |
| MISSING_WITNESS | Auth/Friend one each; other owners zero | PASS; quarantine retained, not settlement |

All three same-owned-cluster restarts preserved actual prepared GID/XID/database
state and system lineage, with distinct postmasters. Prepared counts were
`0`, `4`, `2`. The final run produced six before/after owner pairs (12 snapshots,
36 table projections), with no observer SQL errors and exact migration receipts:
Auth 18, Concert 40, Friend 18, Notification 6, Profile 9. Scoped business rows
were empty: this validates the SQL/helper against real owner schemas, **not**
outbox, pickup, deletion or closure behavior. The extra first-attempt FINISHED
and restart are preserved history, not additional final acceptance credits.

Full release at 13:31:11.233725 UTC verified all eight allocated roots absent,
including the failed attempt's two roots, with zero owned processes/listeners,
private TMP entries and executable copies. Copy hash was verified before removal.
Peak whole allocation was 1,123,263,421 bytes (below 2 GiB), runtime roots peaked
at 314,601,472 bytes, and at most 22 owned processes were observed (cap 80), with
one postmaster at a time. Accounting includes conservative 536,870,912-byte old
Gateway carry and 217,587,712-byte new target growth; failed-attempt history was
not reset. The continuous 8 GiB floor held. Retained proof was approximately
4.81 MB after executable removal; no resources remain held.

Private evidence directory:
`/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-qa09-be-2yvq03iw/`.

| Receipt | SHA-256 |
|---|---|
| `native.json` | `4a9706cbdcc822d73ebf6a66436612e9ccada2bd53f6806e023e9e8d222949d5` |
| `restart-observer.json` | `766ab457a619b8eaa6513b1fce2596ddbc8dcf049aa79499dee7dfd8e43e593b` |
| `observer-sql.json` | `829190a4d3fa6a4edfee6f50ff928f89c77b228744e39f761c0acefd8c2305fb` |
| `runtime-result.json` | `40be35bd772f61b413e694f4eac6a754b60f6abf5460bb48608d7c23496aa84c` |
| `full-release.json` | `c4f93fce5333a5823a12a336663b8219d1701cde37ad84a062ff068e0d9a76d1` |
| `metadata-authority-correction.json` | `c62833be077dcfa17f8f9cc3d8e288df221651d918ca5895b90fa6bd2e7f31eb` |

`qualified-support-report.json` is an independently assembled immutable envelope:
physical restart and restore cases are IN_PROGRESS with pinned component proofs;
other original cases remain NOT_RUN. Restart phase timestamps refer to its first
actual qualified physical boundary; all three per-cluster records are pinned.
Frontend source remains unknown/null. No candidate-bound browser or whole-case
credit is inferred. The CLI's original report and both failed attempts are
preserved separately. Original QA09 still requires actual application restart,
consumer-bound outbox replay, pending/committed pickup, deletion fencing and
privileged partial closure on the real current candidate. Those requirements
are not removed by this supporting delivery. Normal Root publication and own
fresh ancestry/postimage proof are separate from those outstanding product gates.
