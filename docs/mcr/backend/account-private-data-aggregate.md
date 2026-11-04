# BE42 PrivateData aggregate coordinator

Status: **SOURCE IN PROGRESS; CURRENT NATIVE NOT_RUN**. The qualified core leaf remains a separate
published component. This coordinator makes its success one category of a
closed Concert target and withholds whole PrivateData acknowledgement until
every required category has an exact durable owner receipt.

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
`331bd48ec27b7f7927c8700302c3daec3a252d804e6a38aaed2f8d9e29967052`;
this corrected restart sequence is **NOT_RUN** and inherits no native result.

The same native fixture now includes a cancellation witness. A test-only owner
opens a real transaction, writes a sentinel, and waits beyond the coordinator's
original deadline. The test requires `Unavailable`, no terminal owner or whole
receipt, a rolled-back sentinel, and only the real core category remaining
complete. This cancellation case is also **NOT_RUN** on the current source.

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

A fresh named allocation ran the ignored test on source `04a51b2b` against an
owned PostgreSQL 16 cluster, then physically restarted the server and ran the
restart phase through a newly constructed empty-owner coordinator. Both phases
exited `0`. The first process erased the actual core rows, committed Core
`COMPLETE` with six `INCOMPLETE` category rows, retained the common and core
fences, and withheld the whole receipt. The restart process read those exact
six incomplete categories, cursor and fence values from durable state without
invoking the core owner again. It then admitted one exact guarded
`INCOMPLETE` to `COMPLETE` transition and required the other four categories
to remain incomplete. The
rewrite, deletion, source rebinding, terminal transition and premature
whole-ACK checks all passed. The frozen test executable was
`5448a70c0d41a7af6713ec02a1891ffee9f911ceeadac3c0cca0b19f58eb416b`.

The owned evidence is under
`/Users/lhs/Desktop/mcr-evidence/be42-private-data-aggregate-rebased-202611041802Z`.
Earlier failed receipts are retained there as separate failure directories;
they are not counted as qualification. The test ran with an owned cluster and
the controlled test authority. This result does not claim the canonical Auth
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
other-user negatives are **NOT_RUN** pending a fresh named allocation; the
source-level changes do not inherit the earlier result. Publication and the
complete multiuser target remain separate Root and provider-owner handoffs.

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

The serving-composition branch at `915fca43` imported an earlier aggregate
postimage (`af2d67ac`) that still moved `source_binding` into the query before
the asynchronous replay read. The owner source at default `b42f86ee5` keeps
`&source_binding` there, matching the already-qualified `04a51b2b` aggregate
postimage and avoiding an ownership change during cancellation. The
serving-composition source has not been qualified for this coordinator and
must not replace the current owner copy without a coherent intake and a fresh
native run. Composition source status is **NOT_RUN**.
