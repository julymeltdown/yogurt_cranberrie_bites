# BE41 actual Mural freeze owner

Status: bounded component qualification PASS on corrected source
`3fa068013024c4000fe4226c717126f49c899467`; committed publisher handoff pending.
The first fixture failure and its cleanup are retained below. This additive
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

## Validation and resource boundary

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

Actual command forms (all corrected-run commands exited 0; prior interruption
is recorded separately below). Cargo used the explicitly permitted
`--config profile.dev.package.contracts-build.opt-level=2` and environment
`CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0`, with the named shared target:

```sh
python3 backend/scripts/mcr/event_cleanup_mural_owner/freeze.py --snapshot "$proof/source-before.json"
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p concert-service --test event_cleanup_mural_owner --no-run --message-format=json --config profile.dev.package.contracts-build.opt-level=2
python3 backend/scripts/mcr/event_cleanup_mural_owner/freeze.py --before "$proof/source-before.json" --cargo-json "$proof/cargo.jsonl" --out "$proof/frozen" --accepted-offer "$offer"
# Release shared target before the following command.
python3 -B backend/scripts/mcr/event_cleanup_mural_owner/run.py --frozen "$proof/frozen" --accepted-offer "$offer" --target-growth-bytes "$target_growth" --carry-bytes "$retained_prior_bytes"
```

Full production authority, Root composition, remaining owners and whole BE41
acceptance stay NOT_RUN. This component is qualified with explicit authority
fixtures, actual native mural effects and unchanged retained engine code.

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

Fresh retry offer `BE41-MURAL-OWNER-RETRY-BACKEND-202611040854Z` grants at most
360 seconds build/freeze and 240 seconds copied native, including the 60-second
cleanup reserve. Its whole 1 GiB cap includes all retained prior evidence and
prior target growth. The runner accepts explicit `--carry-bytes` for this retained
prior allocation, in addition to current frozen target growth and new proof/PG.

## Corrected qualification receipt

The fresh retry was explicitly accepted on final clean full source
`3fa068013024c4000fe4226c717126f49c899467`; no failed run was silently retried.
The source producer itself is unchanged from the first successful compilation.
Fixture correction `b0b4d9b5c08f1f00228e24c7e79fe592962cb72d` pauses only the
first pixel-authority call; `3fa06801` additionally pins retained carry accounting.
Only this documentation changes after the qualified source revision.

- Locked/offline Rust 1.88 build: exit 0, Cargo duration 1.05 seconds.
- Fresh preflight/build/freeze: 57.748 seconds, within 360 seconds.
- Copied native workflow: exit 0, 2.955 seconds, including restart and teardown.
- Ten Rust tests passed: one source pin, genuine SQLx preparation/reapply,
  six actual mural/engine cases, restart preparation and post-restart verification.
  All 37 actual migrations have genuine successful SQLx checksum receipts.
- Three Python admission tests passed, exit 0:
  `python3 -B -m unittest discover -s backend/scripts/mcr/event_cleanup_mural_owner -p test_preflight.py -v`.
- Exact EXE SHA-256:
  `0d1c9a1f4c62abe7015f6430d3b390a43507ede53205900abde176841805e9a1`.
  Size: 14,645,328 bytes. It includes the actual additive producer through the
  dedicated path shim, with actual compiled retained library types.
- Compiled producer/test/support source pin:
  `779ce430943ac7a9bd31fc478cd37ceded50dcc356c5a82d6e82998f2a2aa78e`.
- Native report SHA-256:
  `eb0bc71979ae6978ccf67e6066fe4bb9588ccbc3f72b5719f4b4058f28db3a91`.

Private evidence root:
`/private/tmp/mcr-be41-mural-owner-retry-9bsy991i`. The complete source/runtime,
contract/manifests/lockfile and actual copied migration hashes are recorded in
`source-before.json` and `frozen/freeze.json`; Cargo JSON and each native command's
exit/output/hash, real postmaster identities and full resource receipts are retained.
`frozen/native.json` is PASS with cleanup=true. Its actual cluster
`/private/tmp/mcr-mural-owner-pg-kfog6mi9` is removed; process inspection found no
owned test or postmaster. Prepared transactions: zero. The first failed cluster
is independently verified absent as well.

Target release was explicit and occurred before copied tests/PG, with zero open
handles and compiler processes. No Cargo or target reads followed release.
Retry positive target growth was 320 bytes; retained prior carry was 35,842,027
bytes. Peak sampled whole outputs, including retained carry, evidence and PG,
were 126,237,441 bytes, below the 1 GiB cap. Minimum sampled native free disk was
32,769,351,680 bytes; initial free disk was 32,882,135,040 bytes. All allocated
runtime resources are returned. No production service or real operator was enabled.
