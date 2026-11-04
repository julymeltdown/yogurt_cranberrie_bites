# BE42 PrivateData aggregate coordinator

Status: **SOURCE IN PROGRESS; CURRENT TWO-PHASE NATIVE PASS**. The current source
pin is `160b31909`, with this documentation committed at `f83022b4f`. It carries
the `d6b2ea271` guard correction and current evidence update. The qualified
core leaf remains a separate published component. This coordinator
makes its success one category of a closed Concert target and withholds whole
PrivateData acknowledgement until every required category has an exact durable
owner receipt.

## Closed category registry

The required Concert categories are derived from
[the field inventory](account-private-data-field-inventory.md):

| Category | Required owner meaning |
| --- | --- |
| `CORE_PREFERENCES_AND_AVATAR` | Existing `PrivateDataCoreDeletion` adapter |
| `INTRO_TEXT` | Personal Intro text, discovery projection and private author input |
| `AGE_CLAIMS` | Age verification/claim evidence and private derivatives |
| `MEDIA_AND_MEETING_PHOTOS` | Media bytes/metadata and meeting-photo private linkage |
| `BEACONS_AND_RALLY` | Precise meeting beacons, physical guidance and rally state |
| `PARTY_AND_GIFT_INPUT` | Party applications/membership private input and Gift private child data |
| `MURAL_PRIVATE_METADATA` | Private author/cooldown metadata while retaining public artwork |

Auth, Profile, Post, Friend, Notification and Sharing are separate canonical
deletion targets. They are not represented as local Concert category success,
and this document grants no foreign-database writes.

## Durable protocol

`PrivateDataAggregateCoordinator::apply` accepts only the common
`WorldDeletionCommand` with `WorldDeletionTarget::PrivateData`, a mandatory
`DeletionAuthority`, and the original monotonic deadline. It verifies source
again before committing progress and before a final whole acknowledgement.
It also reauthenticates immediately before each owner category dispatch, so a
source revoked between two categories cannot cause a new owner effect.
The common user-first advisory lock and fence are required before receipt rows
or category effects.

The aggregate establishes that fence through the common helper before its first
owner invocation. This step commits independently, then the aggregate rechecks
the same identity and fence revision before writing progress. A fresh PrivateData
saga therefore does not depend on another target having created the common row.

`concert_private_data_owner_progress` binds saga, user, actor, target,
full-second plus nanosecond retention, identity/command/source digests and
fence revision. `concert_private_data_category_progress` binds one closed
category and version to the same exact command/source. Both receipts reject
update/delete/truncate rewrites, user or category substitution, category
replay, and attempts to reuse another command as a whole acknowledgement.
The category foreign key includes the exact command, source and fence tuple,
so a child receipt cannot silently detach from its aggregate receipt.

A category may return bounded progress and resume later, but only an exact
receipt with matching command, source and category version can become
`COMPLETE`. Missing or unconfigured owners become durable `INCOMPLETE` entries.
The whole progress row is marked complete only when all seven category rows
are complete; the migration independently rejects an earlier completion.

A category first recorded as a durable missing owner has no owner revision.
When a real owner later becomes available, that row may transition exactly once
from `INCOMPLETE`/null to `COMPLETE` with its verified positive revision. The
coordinator includes the exact saga, category, version, command, source, fence
and ordinal in that update. Terminal rows remain immutable, and the SQL guard
permits no deletion, rebinding, revision regression or second completion.
Once a category is terminal, both its owner revision and completion timestamp
are frozen. Only an exact idempotent replay, which changes neither field, may
observe it. An owner that returns non-terminal progress is stored as an
unrevised `INCOMPLETE` row; only a complete owner result can supply the one
positive revision. Repeating that same nonterminal result leaves the row
unchanged and retryable; a positive revision on an incomplete row is invalid.

`PrivateDataCategoryOwners::production` registers only the actual core owner.
The absent Intro, age, media, beacon/rally, Party/Gift and Mural owners remain
retryable and cannot produce a positive PrivateData acknowledgement. Test-only
typed owners may prove protocol behavior but never an owner category or
production readiness. A complete aggregate row requires all seven exact
category receipts; the coordinator checks that condition before emitting the
positive acknowledgement, and the SQL guard rejects a premature completion
independently.

The aggregate counts completed category rows only for the exact saga, command
and source tuple. The category table permits at most one row per closed category
and ordinal, so a positive count of seven necessarily represents all seven
distinct terminal categories. A repeated owner invocation cannot count twice;
it must match the existing terminal row exactly or fail.

A stored whole receipt is also reauthenticated before replay. The coordinator
reconstructs the exact command, calls the configured `DeletionAuthority` again,
and requires the same source binding before returning a positive aggregate ACK.
Database state alone is never accepted as current authority.

The bounded retry cursor advances only after a category receipt commits.
Missing owners leave their own durable `INCOMPLETE` row without converting
another category's success into a whole target success. A process restart can
therefore resume from the same source-bound receipt without rerunning a
completed category or acknowledging hidden work. The cursor does not exclude a
missing category from later selection: every incomplete category is retried,
so a real owner configured after an earlier round can supply the one exact
terminal receipt rather than being stranded behind the cursor.

The current native fixture leaves six missing categories durable in its first
process. After a physical PostgreSQL restart it constructs an empty-owner
coordinator, reads those six incompletes, and replays without rerunning core.
It then applies one guarded missing-to-complete transition and requires five
remaining incompletes, no whole ACK, and an unchanged core fence. The test
postimage is
`db4d6195b38ed65370d4163e3307e577cae4ac39a7ad6ec9c7340a71c52fdb36`.
The current sequence passed against the PostgreSQL 16 fixture described below.

The same native fixture now includes a cancellation witness. A test-only owner
opens a real transaction, writes a sentinel, and waits beyond the coordinator's
original deadline. The test requires `Unavailable`, no terminal owner or whole
receipt, a rolled-back sentinel, and only the real core category remaining
complete. It runs only in the first process before a second process projects the
durable restart state. This cancellation case passed on the current source: the
primary user retained one Core completion, the distinct cancellation user
retained one Core completion with no terminal probe row, and the aggregate
remained unacknowledged.

A missing core owner returns `Unavailable`; a missing non-core owner remains
retryable `Incomplete`. No empty registry or synthetic category receipt can
satisfy the aggregate completion check.

`progress(user_id)` treats persisted metadata as a lookup key, never authority.
After reconstructing the exact common command, it authenticates that command
again through the configured `DeletionAuthority` and requires the returned
source binding to equal the stored binding before exposing any progress. A
rebound, unavailable or malformed source therefore remains a failure rather
than a resumable success.

## Validation boundary

Source tests cover closed registry membership, exact source/category binding,
nanosecond command binding, redaction, and the core-only production registry.
The common command hash preserves full nanoseconds at the Rust boundary. The
canonical Auth source returns the retention value it read from PostgreSQL's
`timestamptz`, so an actual source tuple is microsecond-aligned. The native core
fixture uses `.123456000Z`; `0055` remains immutable, and this fixture does not
claim that a sub-microsecond PostgreSQL timestamp can be represented there. The
authority boundary rejects a request whose original audit value differs from
the stored value at microsecond precision.
The ignored native test requires an owned PostgreSQL database
`mcr_private_data_aggregate`. It runs the actual core adapter, records a real
core category completion, leaves missing categories `INCOMPLETE`, verifies the
aggregate is not acknowledged, checks guard rejection and proves the existing
core fence remains. The regression also checks the retained timestamp to the
nanosecond and exercises one direct missing-to-complete SQL transition followed
by rejected revision and binding rewrites.

A fresh named allocation ran the ignored test on historical source `04a51b2b`
against an owned PostgreSQL 16 cluster, then physically restarted the server
and ran the restart phase through a newly constructed empty-owner coordinator.
Both phases exited `0`. That historical process erased the actual core rows,
committed Core `COMPLETE` with six `INCOMPLETE` category rows, retained the
common and core fences, and withheld the whole receipt. The restart process
read those exact six incomplete categories, cursor and fence values from
durable state without invoking the core owner again. This result belongs only
to frozen executable
`5448a70c0d41a7af6713ec02a1891ffee9f911ceeadac3c0cca0b19f58eb416b`
and does not qualify the current source.

The current source `160b31909` has passed the 11 pure source tests from its
preceding frozen executable. A first actual
PostgreSQL attempt on `e34d1c8ae` exposed that the category guard accepted a
terminal timestamp on an `INCOMPLETE` row. The guard now rejects both terminal
time rewrites and non-null terminal time on incomplete rows. Its evidence is
preserved at
`native-failure-incomplete-terminal-field-20261104T193920Z`. A second attempt
reached the assertion but used `completed_at + 1 microsecond` on a `NULL`
value, which was a no-op. That failure is preserved at
`native-failure-null-increment-20261104T194206Z`. The fixture now writes a
concrete `clock_timestamp()` to the incomplete row and expects rejection. That
fixture change is committed at `a9a6bf2d2` but needs a fresh compiled native
executable and a fresh two-phase PostgreSQL restart. No older executable or
pass is reused for this pin.

After the two corrective commits, the allocator issued
`PRIVATE-DATA-FIXTURE-REBUILD-MEDIA-20261104T1943Z` as a compile-and-freeze-only
successor with a 1 GiB new-growth cap and a 24 GiB whole cap. The same charge
model used by the earlier admitted BE42 round treats the pre-existing shared
target as centrally retained prior-owner carry, while this turn separately
accounts for its own worktree, evidence, and bounded new target growth. The
exclusive worktree, current evidence, historical evidence and shared Git store
measured 4,034,658,304 bytes before admission. The shared target measured
19,117,899,776 bytes, is retained infrastructure rather than a fresh copy, and
must not be charged twice. The round remains pending a literal current lease;
neither calculation is a native result.

The allocator then issued the fresh
`PRIVATE-DATA-FIXTURE-REBUILD-MEDIA-20261104T1955Z` compile-and-freeze-only
turn. It accepted at `2026-11-04T19:56:33.774227Z` on source `5a824824d` after
both guards passed. The exact native test selected from Cargo was copied
outside the shared target and released directly to Age before documentation:
`7f760f8049cdcf8b85507f3baea61cd7554e6b8f7a570c557434333d6e6ef6e4`,
13,222,096 bytes. The target grew 13,254,656 bytes and finished at
19,128,852,480 bytes. Compilers, readers, PostgreSQL processes, services and
owned children were all zero at release. This is compile and freeze evidence
only; it does not itself establish native qualification.

After the 1955 compile-only result released the target, the allocator issued
`PRIVATE-DATA-TWO-PHASE-NATIVE-20261104T2015Z` as a fresh native-only successor.
Both admission guards passed before PostgreSQL started: the independent guard
reported source `160b31909`, a 19,691,917,312-byte free-space baseline, zero
compiler and native processes, and zero target handles. The controller reported
the exact source, frozen executable and runner pins, with 256 MiB for PostgreSQL,
48 native processes, 4 GiB native RSS, 240 seconds of work, 360 seconds through
the FULL receipt and 120 seconds for cleanup. The target and compiler limits
were zero for this round.

The corrected runner SHA-256 is
`b4ff9016089f64fde4ea10d3665fca3828d121e2a1fdd45d8650ae3cb712a285`.
The first run used its predecessor and passed both test processes, but its final
ad hoc SQL dump was not scoped to a user and doubled the primary user's Core row
with the cancellation user's Core row. The test assertions themselves used
user `4200`, but the dump was ambiguous evidence. It is preserved at
`native-pass-2005-run1` and `native-current-unscoped-dump-preserved`; it is not
used for the final current result. The corrected runner queries main user
`00000000-0000-0000-0000-000000001068` and cancellation user
`00000000-0000-0000-0000-00000000106a` separately.

The fresh corrected run used frozen executable
`7f760f8049cdcf8b85507f3baea61cd7554e6b8f7a570c557434333d6e6ef6e4`
(13,222,096 bytes), source `160b31909f368b12463b310bb699c2440dab8e51`,
and one owned PostgreSQL 16 database `mcr_private_data_aggregate`. Both
processes exited `0`; each log reports one passed native test. After the
physical server restart, the primary user's exact rows were Core `COMPLETE`
plus five `INCOMPLETE` categories (`AGE_CLAIMS`, `MEDIA_AND_MEETING_PHOTOS`,
`BEACONS_AND_RALLY`, `PARTY_AND_GIFT_INPUT`, and `MURAL_PRIVATE_METADATA`),
with cursor `7`, no aggregate ACK, Core `COMPLETE`, and exactly one Core fence.
The first process also proved the Core category earlier; the controlled test
owner made `INTRO_TEXT` terminal in the restart process. The cancellation user
had one Core completion, no ACK, and no probe row. PostgreSQL was stopped and
its owned data root removed at `2026-11-04T20:11:25.593840Z`. The runner ended
at `2026-11-04T20:11:25.594091Z`, with no remaining compiler, native or owned
PostgreSQL process.

Raw first-phase and restarted-phase logs, exact exit files, the PostgreSQL
startup/restart/cleanup records, source and executable pins, and the corrected
SQL result are under `native-current`. This result qualifies this coordinator's
two-phase durable protocol on the frozen fixture. It does not supply six real
production owner effects, canonical Auth-to-Concert transport, combined serving
composition or whole BE42 acceptance.

The owned evidence is under
`/Users/lhs/Desktop/mcr-evidence/be42-private-data-aggregate-rebased-202611041802Z`
and the current guard/fixture evidence is under
`/Users/lhs/Desktop/mcr-evidence/be42-private-data-aggregate-ready-20261105`.
Earlier failed receipts are retained as separate failure directories; none is
counted as qualification. The historical test used an owned cluster and the
controlled test authority. These results do not claim the canonical Auth
transport, the real production non-core owner effects, or a complete
PrivateData deletion.

## Resource-procedure exception

The nominal target release at `18:13:26.959451Z` was followed by additional
Cargo retries at `18:14:50Z`, `18:15:29Z`, `18:16:17Z`, `18:17:15Z`, `18:18:01Z`
and `18:18:50Z` to correct the owned test fixture. Those re-entries were not
retroactively granted by the capacity coordinator. The original acceptance at
`18:10:45.710193Z`, every retry, the final target release at
`18:19:38.678139Z`, and the FULL receipt at `18:21:24.235327Z` remain preserved.
This is recorded as a resource-procedure exception. It does not rewrite source,
erase a failure, or convert later activity into authorization.

The native fixture is intentionally narrow: it runs the actual core adapter and
does not simulate successful non-core owners. Whole PrivateData remains
**INCOMPLETE**, canonical Auth-to-Concert transport is **NOT_RUN**, and the
Intro, age, media, beacon/rally, Party/Gift and Mural production owners remain
unintegrated here. The earlier native result on `04a51b2b` remains bound only
to that executable and fixture sequence. The current restart-order, cursor and
other-user negatives passed on source `160b31909` and frozen executable
`7f760f80`; the result does not inherit from the earlier executable. The
earlier SQL-guard failures remain preserved, and the corrected fixture was run
rather than inferred from a migration-only source edit.
Publication and the complete multiuser target remain separate Root and
provider-owner handoffs.

The production composition is owned separately by the backend integrator. It
constructs the canonical core-only registry and may add each real provider
only after that provider has passed its own coherent qualification. The
coordinator's closed registry will reject duplicate category insertion. A
missing provider stays durable `INCOMPLETE`; the aggregate cannot turn the core
owner or an unavailable constructor into a seven-category acknowledgement.

The current provider interface pins called out by that integrator are
Intro `40a35a60`, Age `9762780a`, Media `117e7d34`, Party composite `cc4770dd`
with Gift cleanup `37b61bd2`, Mural `8f7013d6`, and Meeting `cc4770dd`. These
are composition inputs and are not counted as coordinator native evidence.
The Party/Gift composite becomes terminal only when both actual child effects
are terminal; Meeting remains a separate `BEACONS_AND_RALLY` category.

The current serving-composition branch imports aggregate blob `b33e7425`, which
retains the owned borrow fix (`&source_binding`) from `cfa8793e`. Its historical
native blob `6a698ca9` is not the current owner native postimage. The owner
source at `709f506b4` has aggregate blob `1e191147`, adding the pre-owner
reauthentication, and native blob `64945802`. The provider owner confirmed
that it will not overwrite the six owner postimages or fold them into a stale
serving copy; coherent intake must carry the full immutable owner pin. Current
composition/native status remains **NOT_RUN**.
