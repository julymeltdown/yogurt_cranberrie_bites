# BE41 actual Mural freeze owner

Status: source implemented; first native qualification failed in a concurrency
test fixture, with owned resources cleaned. Corrected qualification pending. This additive
producer does not activate BE41, establish production operator/system authority,
or accept the complete event-closure/public archive flow.

## Scope and provenance

Base: published `0bd35b223d2590fb2b343171d6d8ac30d37ff857`, containing engine
publication `9acca07b7f9c22d77e018efa4ef2ba830f48a829`. The frozen engine module
Git blob is `fb2ba92bfbb4b76a9c1f8a2950ce64b5c4e85ce4`.

The new source is `backend/services/concert-service/src/event_cleanup_mural_owner/mod.rs`.
The dedicated integration test includes that exact source with a path shim and
reexports the actual `concert_service::event_cleanup_live` and `event_lifecycle`
modules. Root alone owns the eventual `pub mod event_cleanup_mural_owner;`
library export and production composition. No engine, existing schema, library
root, router, worker, manifest, lockfile or contract is changed here.

The authority fixture and lifecycle seed pattern are adapted from
`tests/event_cleanup_support/mod.rs` at the base above. They use explicit fixture
privilege/retention inputs, the real retained READ_ONLY SQL adapter and actual
engine. No synthetic mural owner or synthetic effect table is reused. The frozen
artifact and socket-cluster runners adapt `backend/scripts/mcr/event_cleanup/`
at that same base; destinations are the exclusively assigned
`backend/scripts/mcr/event_cleanup_mural_owner/` files. The producer itself is
newly authored against actual migration `0019_mural.sql` and the frozen ABI.

## Behavior and transaction contract

`MuralFreezeOwner` implements the existing `LocalOwner` trait for `Owner::Mural`
and `Step::FreezeMural`. It requires a supplied native transaction and the real
engine's matching event/job/actor/retention/progress scope. It reads the retained
deadline; it does not set a privilege GUC or treat a settings flag as authority.
The trusted engine establishes actual current authority fences before invoking
the producer. The caller must roll back errors and retain fences through commit.

Lock order is event, job, progress, mural, checkpoint. The engine already holds
the first three. For EDITABLE/PAUSED murals, the producer increments revision,
assigns a fresh checkpoint ID and timestamp, sets FROZEN and advances
`reset_revision` to the new revision. It preserves pixels, palette, retention
floor, author history, region locks and cooldowns. It then inserts the exact
matching FROZEN recovery checkpoint using SQL on that same supplied connection.
Revision exhaustion fails without an effect. An already-FROZEN mural must have
the actual current matching checkpoint, including pixels, palette and creation
time; it is neither advanced nor repaired on replay.

The returned receipt ID and revision are the actual snapshot ID and revision.
Its binding and operation come from the full frozen `BatchRequest`, including
actor/event/job/step/owner/retention/ordinal/cursor/limit. `processed=1` consistently
counts the one verified mural resource, including a preexisting frozen mural.
The engine counts/appends the terminal operation once.

First execution verifies the real effect before the immutable batch row exists:
engine call order is `apply_in -> verify_in -> checkpoint.apply -> append` in
one savepoint/transaction. No substitute receipt table or migration is created.
After commit the actual engine `concert_event_cleanup_batches` row retains the
complete exact binding and snapshot identity. Replay, `ready`, and
`ConvergedOwners` recheck current native state and the retained batch; missing,
altered or stale source truth cannot become convergence. Private author/cooldown
data is neither selected into the checkpoint nor returned in the receipt.

Existing 0019 prevents checkpoint UPDATE, but has no DELETE trigger. This producer
does not expand schema ownership: a deleted current checkpoint is detected as
unavailable on verification even when the engine's immutable batch remains.
The producer performs no pool acquisition, independent commit or network mutation.

## Validation plan and resource boundary

The dedicated test binary has one source-pin test plus isolated native cases for
genuine SQLx migration receipts/reapply; EDITABLE/PAUSED freeze; exact replay;
preexisting FROZEN checkpoint verification; missing/mismatched source;
revision exhaustion; real-effect rollback on error/PENDING/deadline expiry;
receipt/scope rebinding and absent/revoked fixture authority; actual production
pixel writer versus closure/freeze; immutable batch/checkpoint rejection and
lost-source verification; and postmaster restart persistence.

No Cargo target or PostgreSQL process may be used until a fresh named offer is
explicitly accepted. The finite request is one locked/offline Rust 1.88 integration
test build, jobs=2, debug=0, incremental=0; at most 10 minutes, 1 GiB of new outputs
and an 8 GiB free-space floor. Copy exactly the test executable and migrations
outside the shared target and release it before running tests. Native PG16 uses
a fresh owner-marked directory, private Unix socket, no TCP listener and actual
fsync/synchronous commit. The runner checks frozen executable identity/hash,
migration hashes, source pin, meaningful nonzero test counts, real postmaster
restart and owned teardown. Native time cap is four minutes, with work stopped
at three minutes to reserve at least 60 seconds for cleanup. Whole-workflow
accounting includes frozen target growth, all evidence and temporary outputs
and the owned PG cluster.

Planned commands (NOT_RUN until accepted resource allocation):

```sh
python3 backend/scripts/mcr/event_cleanup_mural_owner/freeze.py --snapshot "$proof/source-before.json"
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p concert-service --test event_cleanup_mural_owner --no-run --message-format=json
python3 backend/scripts/mcr/event_cleanup_mural_owner/freeze.py --before "$proof/source-before.json" --cargo-json "$proof/cargo.jsonl" --out "$proof/frozen" --accepted-offer "$offer"
# Release shared target before the following command.
python3 backend/scripts/mcr/event_cleanup_mural_owner/run.py --frozen "$proof/frozen" --accepted-offer "$offer" --target-growth-bytes "$target_growth"
```

Record actual source commit, commands/exits, executable/report hashes, resource
release and cleanup evidence here after execution. Full production authority,
Root composition, remaining owners and whole BE41 acceptance stay NOT_RUN.

First attempt, named offer `BE41-MURAL-OWNER-BACKEND-202611040839Z`: clean source
`af00e848a803a4b1013d0faf3a535f6a60eb44d3` built successfully, exit 0. The copied
source-pin and all 37 genuine SQLx migrations/reapply checks passed. Native effects
were interrupted, runner exit 130, after tracing a fixture deadlock: the actual
pixel adapter authorizes repeatedly, while the concurrency fixture paused every
check. The fixture now pauses only on its first check. The runner also persists
stdout/stderr during execution so interruptions retain partial test diagnostics.
No production source correction or unallocated retry occurred.

First-attempt evidence is retained at
`/private/tmp/mcr-be41-mural-owner-proof-byvab_gz`; `frozen/native.json` records
FAIL and cleanup=true. The actual private PG root was removed, no owned test or
postmaster remained, and the shared target had already been released with zero
handles. Build/freeze took 97.193 seconds and native teardown completed after
74.824 seconds. Peak sampled whole outputs were 87,865,218 bytes, minimum native
free disk 31,747,448,832 bytes. A fresh named offer is required for the corrected
source build and new owned native workflow; the failed attempt is not overwritten.
