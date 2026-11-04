# QA09-BE recovery orchestration and owner observations

Status: IN_PROGRESS. Source observer regressions passed; fresh physical recovery
qualification and normal Root publication are pending. Original QA09 remains open.

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

Fresh Cargo/target/native/PG recovery: NOT_RUN pending a fresh named resource
OFFER and actual ACCEPT. No resources held. Future qualification retains the
16 GiB new-start and continuous 8 GiB disk floors, whole-task byte limits,
absolute deadlines, target release before copied execution, and exact-owned
cluster cleanup. Original negative restore cases retain quarantine until owned
teardown; teardown is not native settlement. Normal publication and independent
remote ancestry/postimage proof remain required before this bounded delivery closes.
