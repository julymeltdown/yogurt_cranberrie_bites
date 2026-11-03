# C05 Friend durable invalidation journal

## Assignment and delivery boundary

Task `C05-FRIEND-INVALIDATION` is an owner-local producer/replay increment on
`codex/mcr-be-friend-invalidation`, based on delivery revision
`f837bef676cd26869c26c172a699a91b7842d832`. The frozen contract is
`docs/mcr/world-invalidation-contract.md`, revision 1, published at `9204e1e5`;
the corresponding wire definitions are `backend/proto/relationship/v1/relationship.proto`.
The product specification remains `MCR_IMPLEMENTATION_PLAN_V2_EN.md`.

Status: `CONTRACT_READY` for the owner-local producer/replay increment. Native
qualification passed; delivery integration remains separate.
This increment does not expose a service endpoint, alter startup, authenticate
workloads, advance a World checkpoint, implement a consumer fence, or certify
IN01. Those boundaries remain `NOT_RUN`. A valid invalidation never grants
permission or replaces a current relationship observation.

Writable paths are new migration `0016_concert_invalidation_journal.sql`,
`friend-service/src/concert_invalidation/**`, its module export in `src/lib.rs`,
`tests/concert_invalidation.rs`, `tests/concert_invalidation_support/**`,
`backend/scripts/mcr/{friend_invalidation_smoke,test_friend_invalidation_smoke}.py`,
and this document. Existing mutators, migrations 0001–0015, shared contracts,
manifests, lockfiles, other services, and frontend remain unchanged.

## Production behavior

Migration 0016 adds an independently generated permanent journal epoch, a
singleton committed head, and an append-only retained journal. Existing 0015
observation epoch/revision are separate and cannot be used as replay cursors.
The journal carries only an epoch, safe integer sequence, stable random signal
UUID, closed reason, and one user UUID or a normalized pair of distinct user
UUIDs. It contains no profile attributes, handles, secrets, or permission grants.

AFTER STATEMENT transition-table triggers cover every inserted, updated, or
deleted `user_blocks` row and deletion tombstone. A block pair is invalidated in
both directions. Updating identity columns invalidates both old and new scopes;
multiple occurrences of a scope within one statement are deduplicated. Empty
transition sets emit nothing. Repeated writes may over-invalidate safely; this
journal is not a deduplicated business event feed. Account-deletion updates and
tombstone creation use the same transaction as the existing repository mutation.
No repository mutation code is copied or changed.

Existing 0015 `friend_relationship_*` statement triggers run before new
`friend_world_*` capture triggers, following PostgreSQL's ordering for equal-kind
triggers. Allocation explicitly locks the existing relationship revision row
before the journal head, including direct owner-local journal inserts. The head
increments under that transaction lock; there is no SQL sequence, MAX-based
allocator, or visibility window between mutation and journal commit. Rollback
reverts both rows and counter. Concurrent writers serialize, so a late commit
cannot introduce a lower sequence behind an acknowledged cursor.

Head identity and journal rows cannot be updated, deleted, or truncated by normal
DML. A deferred indexed lookup requires a retained signal for every allocation
before commit. In particular, `INSERT ... ON CONFLICT DO NOTHING` cannot suppress
a signal while committing an advanced head. Source-table TRUNCATE is rejected:
PostgreSQL supplies no row transition table for it and this contract has no
owner-wide scope. Explicit scoped DELETE remains captured. A database owner can
disable triggers; privileged schema tampering is not an authorization boundary.
There is no remote acknowledgement, pruning, or retention truncation.

## Read API and operation budget

`friend_service::concert_invalidation::PgJournal` provides `read_head` and
`read_after`. Query constructors reject zero UUIDs, out-of-range sequences, and
limits outside 1–256. Wire conversions additionally require canonical lowercase
UUID strings. Replies echo every query field. Unknown reasons, malformed scopes,
gaps, duplicate identities, and incomplete pages fail closed. Wrong epoch and
future cursors return `FailedPrecondition`; malformed input is `InvalidInput`.

A page and its head are selected in one SQL statement and therefore one committed
MVCC snapshot. Results must contain the exact contiguous range from `after + 1`
through `next`, bounded by `min(limit, head - after)`. An empty result succeeds
only at the current head. Conversion enforces the 128 KiB protobuf response cap.
The complete journal is retained for bootstrap/replay, independent of observation
revision changes.

The caller supplies one absolute monotonic stop, no more than one second away,
and an explicit cancellation token. Existing ambient deadlines can only shorten
it. The same budget covers pool acquisition, read-only setup, SQL, conversion,
and completion. Server statement timeout uses the remaining budget; failures
are sanitized. Cancellation and deadline expiry remain corresponding failures,
never empty success. Each borrowed reader connection is marked close-on-drop,
so a cancelled SQL future cannot re-enter the pool with pending protocol state.
This is an owner-local bounded failure guarantee, not a network cancel-ack or a
World fence release guarantee.

## Provenance and dependencies

All new journal/reader/test code is authored for MCR. Existing headless Friend
repository behavior is used through `PgFollowRepository` and its public ports;
no historical rendering or frontend code is imported. Upstream foundation
provenance remains in the existing backend provenance records. No dependency or
lockfile changes are required. Existing `sqlx`, `tokio`, `uuid`, `prost`, `tonic`,
`platform`, `chrono`, `thiserror`, and `migration-runtime` supply the reader and
qualification code. Frozen source/binary hashes and migration SHA-384 receipts
are recorded in the native report.

## Qualification and limits

The smoke runner creates a new marker/inode-owned private PostgreSQL 16.14
cluster, uses only its Unix socket on a private 0700 directory, disables TCP,
checks database/user/data-directory/version identity in the Rust fixture, and
removes only the owned cluster after stopping it. It accepts copied executables
outside Cargo targets. It never attaches a pre-existing database or inherits
provider credentials. Failure paths retain sanitized reports and still attempt
owned cleanup.

The upgrade test applies original SQLx migrations 0001–0015, seeds real block
state through the repository, applies 0016, compares every original receipt,
reapplies the complete set, and verifies independent epoch/head initialization.
Native scenarios then exercise actual repository block/unblock/deletion,
direct-SQL identity changes and deletion, append-only guards, rollback,
suppressed-insert rollback, concurrent commit and rollback ordering, full and
partial pages, epoch/future rejection, explicit cancellation, inherited deadline,
privileged retained-gap corruption rejection with exact-row restoration,
and stable replay after PostgreSQL restart. Harness unit checks prove refusal
and cleanup only; they do not count as native application evidence.

A targeted static review found no new journal/reader defect. It identified an
existing, separate `unblock`/account-deletion lock inversion: `follow_repository.rs`
UPDATE can lock a block row before the recommendation refresh trigger acquires
subject advisory locks, while `apply_user_deletion` acquires subject advisory
locks before updating block rows. The pre-existing path can deadlock and abort
one transaction. Main acknowledged separate ownership of that correction;
this increment has no grant to modify those mutators. A rolled-back mutation
must still emit no committed journal signal. This is an existing limitation,
not a claim of successful concurrent account-deletion acceptance.

### Validation receipt — 2026-11-04

- Rust `+1.88.0`, `--locked --offline`, two jobs, dev/test debug info zero,
  `CARGO_INCREMENTAL=0`; standing exclusive MEDIA target lease. Fresh target
  handles were zero before compilation; free space was 8.67 GiB initially and
  8.94 GiB before the final review-fix test build. No third target or dependency install.
- Build: `cargo +1.88.0 test --locked --offline -p friend-service --test concert_invalidation --test concert_relationship --test unit --test architecture --test contract --no-run --message-format=json` — exit 0 after replacing an unavailable macro with the existing runtime migration loader. The first build exited 101 because retained SQLx features do not expose `migrate!`; no manifest change was made.
- Final native-test rebuild: `cargo +1.88.0 test --locked --offline -p friend-service --test concert_invalidation --no-run --message-format=json` — exit 0. Exact executable paths came from Cargo JSON; copies and SHA-256/source manifests were frozen outside the target. `lsof` reported zero target handles before PostgreSQL started.
- Frozen new query/wire/budget test: 1 passed, exit 0. Retained Friend units:
  87 passed, exit 0; architecture: 2 passed, exit 0; relationship guard/shape:
  2 passed, exit 0.
- `python3 -m unittest discover -s backend/scripts/mcr -p test_friend_invalidation_smoke.py -v` — 5 passed, exit 0. The forced-failure fixture deliberately prints a FAIL report while verifying owned cleanup; the unittest run succeeds.
- `python3 scripts/mcr/check-plan.py` — exit 0; planning provenance only,
  including 13 original archive checksums. `git diff --check` — exit 0.
- Retained contract executable: exit 101, **6 passed / 14 failed**. Each of the
  14 container-backed cases stopped at the existing unavailable Docker socket
  `/var/run/docker.sock`. These cases remain `NOT_RUN` at their database/runtime
  boundary; they are not certified by the journal qualification. No Docker
  remediation or unrelated source edit was performed.

Native command, from repository root, with explicit copied executables:

```sh
python3 backend/scripts/mcr/friend_invalidation_smoke.py \
  --binary /private/tmp/mcr-friend-invalidation-f837bef-20261104/frozen-3/friend-service \
  --repository-test /private/tmp/mcr-friend-invalidation-f837bef-20261104/frozen-3/concert_invalidation \
  --relationship-test /private/tmp/mcr-friend-invalidation-f837bef-20261104/frozen-3/concert_relationship \
  --pg-bin-dir /opt/homebrew/opt/postgresql@16/bin \
  --report /private/tmp/mcr-friend-invalidation-f837bef-20261104/native-3.json
```

The runner requires a new report path when repeated. Result: exit 0,
**8 journal native tests + 1 unchanged native relationship/revision test passed**,
PostgreSQL **16.14**, restart **PASS**, cleanup **PASS**. Every native test was
invoked exactly with `--ignored --exact NAME --test-threads=1`, and each report
requires `1 passed; 0 failed`; ignored/default test discovery is not acceptance.
No `RUST_MIN_STACK` override was inherited or used. Migration CLI verify,
reapply, verify-after-restart, and retained-owner migration all exited 0.
Every original 0001–0015 source checksum and applied SQLx SHA-384 receipt stayed
unchanged. A fresh 0016 head starts at zero without inventing historical signals;
existing relationship state remains available to bootstrap/current reads.

Report SHA-256:
`86ce97f3ad9e51987faba221a74cc9b8b91c62a20df7145ed6efb9e51b30bb6f`.
The final report records exact source hashes against implementation checkpoint
`414d0c2a` (the test-only review fixes were uncommitted during qualification), migration receipts,
commands and executable identities. It is a local evidence artifact, not a
production release receipt. Frozen artifact hashes:

| Copied executable | SHA-256 |
| --- | --- |
| Friend service | `bd1b5d995b9d77ed6fe97a0a882cc9ba67ff57cc598e9754b631efdfeb49b2c8` |
| Journal native tests | `5d929dccd792d83d9f008f6066f4e9edbe3319ca62ef4bc419c57763420e561f` |
| Retained relationship tests | `df8a2b696dde90e36a4642385e7f3dc00674e553d9736c6562cdac7ac5f1df14` |

An earlier native attempt passed upgrade and repository mutation checks, then
failed a direct-SQL fixture that omitted existing required `created_at`. Only
the test fixture was corrected. Its report (`native-1.json`) records exit 1 and
cleanup PASS; it is not counted as qualification. All three actual owned clusters
were stopped and removed. No active database or Cargo child is required by the handoff. The MEDIA lease
was explicitly released to Main after the final build copies were frozen and
zero target handles/build children were freshly verified. No further target
access is authorized or needed by this owner.

No public RPC, production stack, real multiuser flow, or release gate is accepted
by these owner-local checks. The broader persistent BE13/BE14 goal remains
unfinished; this journal increment does not complete it.

## Committed handoff ancestry

Implementation and initial native evidence: `414d0c2a`, directly descended from
base `f837bef676cd26869c26c172a699a91b7842d832`. The follow-up test/documentation checkpoint addresses all three concrete
qualification findings and records lease release. Source hashes in `native-3.json`
match every final runtime source, contract, manifest and lockfile. Delivery integrates these owned commits serially; only the delivery
integrator may push to `julymeltdown/mcr:master`.

### Concrete qualification review fixes

Three P2 test gaps were corrected after the first passing native report. The
final native report replaces that earlier qualification evidence; production
journal/reader code remains identical to `414d0c2a`.

1. Cancellation waits until `pg_stat_activity` confirms the reader backend is
   actually blocked on the head table, then cancels it. The blocker stays held
   until that exact backend disappears within the inherited budget plus cleanup
   allowance, proving absence of both running SQL and idle transaction residue.
   Deadline cases also drain before the blocking lock is released. This does
   not claim an immediate network cancel acknowledgement.
2. Corruption checks cover both a missing tail and an interior missing record
   with enough later rows to fill limit 2. The full but noncontiguous page must
   return `Unavailable`; both exact records are restored before continuing.
3. Concurrency checks cover source-first and direct-journal-first transactions,
   each under commit and rollback. After observing the second source writer
   blocked, the journal-first writer makes its own source mutation under a
   bounded timeout. This exercises the allocator's independent revision-before-
   head ordering and rejects a cycle even if ordinary source-trigger order alone
   would have hidden it. Committed pages stay contiguous in all four cases.

The final build exited 0; final native command exited 0 with 8 journal tests and
1 unchanged owner test, restart PASS and cleanup PASS. No general second review
or Main approval was imposed as a delivery gate.
