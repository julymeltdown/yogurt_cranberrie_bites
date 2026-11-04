# BE42 Age account-deletion owner

Status: the scoped Age owner compiled and passed its populated native scenario,
including a physical PostgreSQL restart and a fresh test/RPC process. Production
aggregate composition and the complete Auth deletion worker remain unverified.
This document does not reopen accepted BE26 provider verification or certify
whole PrivateData deletion.

## Source and ownership

The retained Age worktree starts from canonical
`f08d44c9a3add9695e9c39e1375f19e2037f10a4`. The unchanged provider ABI dependency
is `acee2fac57847ec42656c991f4db7170a552d8fd` (00c aggregate source and migration
0072 on its canonical seed). Its six aggregate paths are dependency intake,
not authored Age changes. Root is the serial backend publisher.

Root also authorized the unchanged coordinator correction
`cfa8793e1697d886d875a5d59a1912a5daa12c0b`, applied here as
`bb0fb785802bdf5a609cb78aef88f359c5aa04a4`. Its only hunk borrows the SQL
`source_binding` bind so that the later reauthentication can still read it.
The aggregate file SHA256 is
`0763d58008a6ff688db6deea690068249e420430a8e3b1efcda8c7b250cca841`.
This is unchanged owner dependency intake, excluded from authored Age scope.

The Age owner authors `age_provider/account_deletion.rs`, its module export,
the necessary `scoped.rs` user-fence checks, forward migration 0066, new
`age_deletion_native.rs`, scripts under `backend/scripts/mcr/age_deletion`,
and this document. The only compatibility adjustment to accepted BE26 tests is
adding `account_private_data, world` to the existing `age_claims.rs` reexports,
because that harness compiles the Age module inline. The claim reader,
verification/trust rules and immutable prior migrations remain byte-identical.

## Actual owner effect

`AgeAccountDeletion` implements the typed `PrivateDataCategoryOwner` protocol
for `AgeClaims` only. It accepts only PrivateData commands and the exact original
deadline, revalidates the actual Auth source before work and before native commit,
and acquires the common canonical user fence before Age row locks. Raw user IDs,
external verifier credentials and physical-arrival approval cannot authorize it.

Each native transaction deletes at most 64 verification rows and their associated
claims, including pending callback state, session/attempt correlation, provider
provenance, exact age and next birthday. Child claims are deleted before parent
verifications. The final page removes the now-unused Age owner row. Other users
and events are outside the deletion predicate. Immutable native settlement
witnesses and command receipts remain with their existing audit owners and
retention policy; this adapter never rewrites their trust or settlement meaning.

Forward migration 0066 stores exact command/source binding, saga, bounded erasure
counters, monotonic verification UUID cursor and revision in `concert_age_deletion_progress`, referencing the existing
common fence. This is category progress, not another authorization/fence store.
An incomplete page commits real progress and returns revision zero/complete false.
A terminal page proves absence of all Age private/owner rows in the same native
transaction, then returns a positive persisted revision. Terminal replay returns
that same revision without rewriting the receipt. The receipt cannot be rebound,
removed, truncated or updated after completion. Changed actor, saga, target,
nanosecond retention, source or deadline fails closed.

Production scoped Age writers and projection authority use the same canonical
user advisory lock before event/claim locks and hold it through native commit.
Permanent deletion fences deny subsequent access. Insert/update triggers on Age
owners, verifications and claims additionally reject recreated sources and late
callbacks under the same fence; they do not grant deletion authority. Deletes
remain available to the actual owner for bounded continuation.

## Composition

Root supplies the actual production `DeletionAuthority` and the Concert pool:

```rust,ignore
let age = AgeAccountDeletion::category_owners(pool.clone(), authority.clone());
// Register AgeClaims exactly once with the other disjoint actual owners.
// Never replace CorePreferencesAndAvatar or manufacture other category ACKs.
```

The factory returns a typed one-category registry. Root owns shared config/main
composition. This leaf cannot acknowledge the seven-category aggregate or the
eight-target Auth saga.

## Meaningful validation

The new ignored native fixture creates populated Age rows in marker-owned Auth
and Concert stores. It obtains deletion provenance through the actual Auth
delete-user use case and production mTLS Auth source RPC. Only Age verification
rows and a BE15 audit row are synthetic fixture data; no commercial age provider
is contacted. It exercises source loss before commit with full rollback,
cancellation while waiting on the first shared user lock, bounded incomplete
progress, exact replay, wrong actor/user/saga/target/retention/source/deadline,
late source recreation, immutable terminal receipts, retained BE15 audit,
physical PostgreSQL restart, a fresh RPC/test process, and continuation of an
incomplete second user after that restart.

`freeze.py` pins source/migrations and selects the exact executable from successful
Cargo JSON. `lease.py` requires an externally accepted named allocation and an
independent guardian before children. The target must be unconditionally released
before `run.py` starts copied tests. The runner owns a new socket-only PostgreSQL
cluster, verifies both migration ledgers, restarts it, checks prepared transactions
and removes only its marker/inode-validated cluster. Resource receipts and final
command results are recorded below. The first failed attempt is retained as
history; the later copied-native allocation passed.

### First actual qualification attempt

The tested source was `f309c37c684b0006ae3bfcb552615b736bfa116e` with the
unchanged coordinator correction above. The standing 1805 offer was amended
before actual acceptance to the bounded warm-target policy: 12 GiB start,
9 GiB free after all outstanding reservations and own worst-case growth,
8 GiB continuous floor. Earlier failed 16 GiB preflight checks were not
admissions. Both actual admission checks used the amended policy.

Actual acceptance was 2026-11-04 18:22:32.266892 UTC. The independent guardian
preceded children. Rust 1.88, locked/offline, jobs 2, debug/incremental disabled
and contracts-build opt-level 2 compiled the exact `age_deletion_native` target.
Four explicitly permitted service build-script timestamps were refreshed with
byte/hash/metadata receipts. A pinned private controller adapted the source
freeze/run helpers to the exact allocation; its bytes and arguments are retained
with the attempt. It created no source archive or cold target.

Cargo exited 1 with nine Wall import/method errors. The exact source exports
`concert_domain::mural::tools_b` and `concert_application::mural::tools_b`, and
both module files exist. Cargo nevertheless reported both upstream artifacts as
fresh while their public metadata lacked those exports. This is evidence of
stale shared-target artifacts, not a successful Age compilation. No executable
was frozen and no native or PostgreSQL test ran. The proposed byte-identical
timestamp refresh of the two upstream module inputs was not performed: it was
outside the four-file metadata grant during this attempt. The allocator later
authorized that narrow repair for a fresh future lease only.

The target was unconditionally released at 18:28:45.266400 UTC with zero
handles/compiler children. FULL release was 18:29:28.315901 UTC, with NEW and
workflow reservations zero and no native process/PG root. The original acceptance,
450-second compile cutoff and failures were preserved; no target re-entry or
cache/history deletion occurred. The original failed logs, source/tool pins,
timestamp receipts and baseline remain in local evidence
`mcr-age-deletion-native-569rqzsa/FULL.json` and its `failed-build-1` directory.

Measured positive target growth was 202,440,704 bytes; final new outside evidence
was 2,129,920 bytes, including the private controller. Peak charged whole storage
was 6,647,021,568 bytes, minimum free space 16,058,511,360 bytes, maximum observed
processes 7 and RSS 1,749,893,120 bytes. Retained historical target charge advances
from 896,724,992 to 1,099,165,696 bytes for the next lease. These figures are the
failed qualification's resource outcome, not functional acceptance.

### Successful compile and freeze

The fresh `AGE-DELETION-RETRY-MEDIA-20261104T1831Z` turn actually accepted at
2026-11-04 19:59:56.579451 UTC on source
`feed4f801e15279d0ac7b4c4a69a9a9af01e2e32`. Its first build failed because
shared Auth application artifacts came from another checkout and exposed a
different `ReadError`. Failed logs were retained. A permitted byte-identical
timestamp refresh of the local Auth domain/application inputs made Cargo rebuild
those artifacts; source bytes, the original acceptance, target baseline and
growth accounting were preserved. Rust 1.88, locked/offline, jobs 2, debug and
incremental disabled, and contracts-build opt-level 2 then compiled successfully.

The frozen executable is 33,324,736 bytes with SHA256
`a0127e7139c5837f71fbfa1f48be94e3afa43156e7e22d5a689d354fe0319461`.
The target was unconditionally released with zero handles/compiler children at
20:03:03.124601 UTC. Native work did not start in this turn: subtracting the
original outstanding reservations and full own growth cap left 9,464,688,640
bytes, below the accepted 9 GiB residual floor. The guard was preserved and the
turn reached FULL at 20:05:00.897089 UTC, with reservations and workflows zero.

Evidence is `/private/tmp/mcr-age-deletion-native-hjhvzr36/FULL.json`, including
the failed build, exact source/tool pins, successful compile, frozen executable
and unconditional target release. Positive target growth was 617,508,864 bytes;
peak whole storage was 7,301,935,104 bytes, minimum free 12,452,483,072 bytes,
maximum processes 7 and RSS 2,986,016,768 bytes. Cumulative retained target growth
became 1,716,674,560 bytes. This closed turn was not reused for native work.

### Successful copied-native qualification

Fresh allocation `AGE-DELETION-COPIED-NATIVE-20261104T2010Z` accepted at
20:09:04.493663 UTC, using that same source and existing executable. It permitted
no target access, Cargo, installation or new executable copy. The immutable
executable and migration inputs were linked into private evidence. Accepted
limits were 640 MiB total new storage, including 512 MiB PostgreSQL and 128 MiB
outside output; 10 GiB whole storage; 48 processes and 4 GiB RSS; 11 GiB start,
9 GiB after all outstanding growth, and an 8 GiB continuous floor. Work had
240 seconds with a separate 240-second cleanup reserve. An independent guardian
preceded children.

The runner exited zero and recorded PASS at 20:09:30.844601 UTC. Both FIRST and
RESTARTED executed `age_deletion_populated_auth_retry_cancel_restart`: one passed,
zero failed in each process. PostgreSQL 17.10 physically restarted from PID
31323 to 32744. Auth's 18 migrations and Concert's 51 migrations matched their
ledgers, and prepared transactions were zero. The marker-owned cluster was
removed. Fresh verification found no controller, guardian, native child or
PostgreSQL process; FULL was recorded at 20:12:19.502925 UTC, with workflows and
remaining growth reservations zero. The next owner and allocator received that
literal release before this documentation update.

Version conformance limitation: the original offer specified PostgreSQL 16.
Both actual acceptance receipts instead pinned the PostgreSQL 17.10 tool path,
and the native result reports that actual version. This is a deviation from the
original offered environment, not evidence of PostgreSQL 16 conformance. The
responsible developer's disposition is a scoped runtime PASS on PostgreSQL 17.10;
PostgreSQL 16 qualification remains NOT_RUN. The version difference must remain
visible in publication and cannot be resolved by relabeling the result. No
additional run or acknowledgment is required merely to correct the handoff.

Evidence is `/private/tmp/mcr-age-copied-native-yjl5vsg2/FULL.json`, with
`frozen/native.json`, `frozen/native/FIRST.stdout`,
`frozen/native/RESTARTED.stdout` and `run-result.json`. Peak whole storage was
7,382,155,264 bytes, sampled peak new output including PostgreSQL 80,220,160
bytes, and positive carry growth 135,168 bytes. Minimum free space was
18,810,568,704 bytes; maximum observed processes 16 and RSS 281,395,200 bytes.
All previous evidence and the 1,716,674,560-byte retained target charge remain.

This proves the scoped populated Age behavior described above, using the actual
Auth repository and mTLS source. It does not exercise dispatch by the complete
Auth deletion worker, commercial age-provider acceptance, all PrivateData
categories or production aggregate wiring. This document-only descendant does
not claim that the frozen executable was rebuilt from its updated document.

## Separate BE41 work

The separately authorized public Lifecycle implementation is retained on
`codex/mcr-be41-lifecycle-public-production` at
`f6ed155080c72454e5a1eafc521878a4895076b3`. Its Rust compilation, native restart
scenario and Gateway runtime remain NOT_RUN. Production integration still lacks
actual cleanup/retention handles in the Operations composition. The original
BE41 blocked result remains blocked; Age qualification does not resolve it.
