# BE42 Age account-deletion owner

Status: source implementation in progress; build, PostgreSQL and restart validation
are **NOT_RUN** until a fresh finite resource allocation. This document does not
reopen accepted BE26 provider verification or certify whole PrivateData deletion.

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
command results will be recorded after that allocated run; current native status
is NOT_RUN.

## Separate BE41 trace

Read-only tracing found retained `event_lifecycle/http.rs` START/RESUME/job-response
logic and durable initiating actor, but no public ProductRPC/Gateway lifecycle
mount or production `FencedLifecycleAuthority`. The proposed public remainder
requires exact routing/closed bodies, current event/settings revision and phase,
the canonical operations-role fence, reviewed retention and public tests. There
is no lifecycle source grant in this Age assignment. Accepted bounded BE41 effects
are not evidence that the public lifecycle flow works.
