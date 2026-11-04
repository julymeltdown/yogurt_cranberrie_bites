# Remaining relationship and social privacy delivery

Status: IN_PROGRESS. Original BE28–BE30 and BE37 remain open; this record does
not certify IN03, public HTTP, WebSocket, browser-cache or release acceptance.

## Source and ownership

Starting commit: `3da28fcb`, branch `codex/mcr-be-social-remaining`, isolated
checkout `be-social-remaining`. The prior Auth/Friend owner-store checkout remains
immutable at `dc70d62049d8e8430c88b9ebfc6f071740020538` for other developers'
joined-process qualification. No source archive, manifests, lockfiles, contracts,
Auth startup or canonical-authority implementation changes are assigned here.

Initial write scope is this record and new Friend-service
`tests/concert_social_remaining*.rs` / `tests/concert_social_remaining_support/`
files. Existing implementation edits require an explicit single-writer transfer.
That transfer is ownership coordination, not a code-review or approval gate.

## Original criteria reconciled against retained behavior

| Task | Retained implementation and evidence | Actual remaining boundary |
| --- | --- | --- |
| BE28 | `relationship/social.rs` and `crypto.rs`: managed-key AES-GCM, normalized version-bound encrypted handles, removal tombstones, secret-free metadata and redacted debug. Existing real-PG qualification is recorded in `relationship-policy.md`. | Managed-secret composition and actual public transport are not bound. Do not duplicate encryption or invent a default key. |
| BE29 | Requester consent, recipient decision, reciprocal platform/version-bound grants, expiry/revoke and BE15 receipts/outbox already exist. | `SocialEligibility` has only unavailable and test implementations. A real fenced introduction/claim adapter and registered producer/root lifetime are required. |
| BE30 | Reveal rechecks membership, restrictions, pair/owner revision, current eligibility, expiry and exact reciprocal handle versions. Dedicated response allowlist sets no-store on success/errors. | No production caller constructs `SocialService`; no registered social routes. Real owners, eligibility and response-lifetime composition remain required. |
| BE37 | Bidirectional owner facts, tombstones, durable invalidation and guarded owner scope exist. Retained block removes follows and pending requests, and unblock does not restore them. | Follow/accept read block state before acquiring the deletion-fence subject locks. A concurrent uncommitted block can become visible while that later write waits, leaving a stale authorization decision. Add native contention proof and serialize before checking, without changing published migrations. |

Source trace covers `MCR_IMPLEMENTATION_PLAN_V2_EN.md` rows BE28–30/37 and privacy
section 8.3, the project assessment/execution plan, retained relationship evidence,
Friend persistence and migrations 0011/0015/0016, and guarded authority source.
Historical injected-authority producer tests are persistence evidence, not real
Auth/Friend/intro/claim or browser acceptance.

## Current validation

Local adapter qualification: PASS at compiled source `600a65c966e06696bfd1b6fcb7bd3752f2103093`.
Original BE28–30/37 runtime delivery remains IN_PROGRESS. See the exact native
evidence below; local fixtures do not certify the public owner/provider join.

## Implemented successor scope

The explicit single-writer transfer covers exactly nine paths: Friend
`src/adapters/out/persistence/follow_repository.rs`, its new
`tests/concert_social_remaining.rs` and `tests/concert_social_remaining_support/owned.rs`;
Concert `src/relationship/eligibility.rs`, `src/relationship/mod.rs` (one export),
`tests/social_remaining.rs`, `tests/social_remaining_support/{owned.rs,native.py}`;
and this document. Base remains `3da28fcb`; application/test checkpoint is
`7b2205c4` (including `5e6eeb4a`, `bd7de61f`, `7cf9d383`). No migration changes.

Friend mutations reuse the existing sorted advisory subject locks from migration
0011 before their block snapshot. Accept discovers the request pair without a
row lock, acquires those subject locks, then rereads the request FOR UPDATE.
This matches block's subject-before-request order and avoids the acceptance /
block cancellation deadlock. Both plain and receipt-backed follow, pending
request creation, block and unblock use the same order. Existing receipt replay
semantics are retained; a historical receipt is not fresh relationship authority.

`PgSocialEligibility` consumes the caller transaction, existing published exact
introduction revision/owner, sorted canonical age-owner roots, and the latest
claim per canonical user before checking status or provenance. It requires
verified age for both sharing participants, applies the introduction audience
bounds to its requester, and clamps the result to request, owner, provider,
introduction, claim and birthday deadlines. It checks database time after SQL and
preserves shorter lock/statement timeouts. SocialService already pins this result
into its deferred transaction-end guard. No secret-bearing DTO is added.

The canonical `from_guarded` constructor derives actor/event/session/request and
original expiry from an acquired `OwnerGuardScope`. `GuardedAuthority` still
requires the actual producer transaction to have been registered by its owner.
This module does not auto-register an unbound transaction or invent a settlement
receipt. The current retained SocialService opens internal transactions; a real
producer/root bridge must register those transactions and settle their actual
outcomes before any public response can be qualified.

### Retained source provenance and prerequisites

The private canonical claim mapping is adapted from BE26
`src/adapters/out/age_provider/claim_reader.rs` at base `3da28fcb`, SHA256
`f4bb5672dce733616373113bf59a28e0b12b9c90c3b2017235a2c420c9f87b1f`.
It uses the existing concert-domain `assess` / `AgeBounds` policy; no alternative
age arithmetic, identity store or provider callback is introduced.

| Unchanged dependency at the base | SHA256 |
| --- | --- |
| BE26 `age_provider/schema-v1.sql` | `1c2e786250e6a4e61b26c4232211ef48d2b4cb7a2e51b258c57b697d1e6a46d5` |
| BE27 `repository/intro_schema_v1.sql` | `fa42cf9028f7eb6caaf20e4b8c34c0ad192396e867d813dfdf917cb1d3f6448c` |
| Friend `0011_user_deletion_fences.sql` | `73be9a37e9b2049ea1dae45c9f4d42505af14ce9c70b1b7358ab8cb307466765` |

BE26/BE27 schemas remain isolated prerequisite artifacts, not registered runtime
migrations. The native runner applies them explicitly only inside its owned test
cluster. Authority owner207 confirmed that the real verifier implementation,
provider credentials/issuer/audience/signature/nonce verification, qualified
availability source, runtime schema and route composition are not yet delivered.
`SharingVerifierStatus` is a trusted server port with an unavailable default;
fixtures implement SQL-held synthetic status solely for local qualification.
An absent source, unsupported/newer claim, unknown owner, expired deadline or
unavailable provider cannot grant sharing.


## Initial native qualification, 2026-11-04

Compiled source: `600a65c966e06696bfd1b6fcb7bd3752f2103093`, including adapter
checkpoint `09fd15caf62f80fb0d0241d4d7b31e76cb1b7566` and the published Party
identity fix `f0d0663a`. The preceding clean `bf92d72a` build failed with E0599
in that Party reader; no Social source correction was needed for the rebuild.
The unfinished `social_live` bridge and transaction hooks were absent from this
source and are excluded from the following qualification.

Build command (exit 0, Rust 1.88.0, offline/locked, two jobs, no incremental or
dev/test debug information):

```sh
cargo test --manifest-path backend/Cargo.toml --locked --offline \
  -p friend-service --test concert_social_remaining \
  -p concert-service --test social_remaining --no-run --message-format=json
```

The allocated BACKEND target was released after copying and hashing the exact
Cargo-emitted executables and recording every relevant source file hash. No
Cargo or shared target access occurred during the subsequent database run.

| Frozen artifact | SHA256 |
| --- | --- |
| Friend test executable | `c2394a4379963457ae28389a97499e9853b76655da35a8809ce5031c7abcd962` |
| Concert test executable | `4646f65d96853419a45b29badcdfecd139feba4488d8af5e8b4584fcd1a2fcff` |
| Native JSON report | `8ee502c758739bdf86d181aa9ec555b27f8fcca0c681d9b452fd48255d157891` |

Local artifacts and exact source manifest:
`/private/tmp/mcr-social-initial-ba5xsvyn/artifacts.json`; native report:
`/private/tmp/mcr-social-initial-ba5xsvyn/native-report.json`.

```sh
python3 backend/services/concert-service/tests/social_remaining_support/native.py \
  --artifacts /private/tmp/mcr-social-initial-ba5xsvyn/artifacts.json \
  --report /private/tmp/mcr-social-initial-ba5xsvyn/native-report.json
```

Exit 0 on PostgreSQL 16.14 (Homebrew), an owned socket-only cluster. All five
Friend and six Concert cases passed both before and after a physical postmaster
restart (22 executions, zero failures). Friend's exact 17 migration ledger was
checked both times. Persisted counts remained Friend `7|2|9` (blocks, follows,
invalidation journal) and Concert `6|15` (introductions, claims) across restart.
The native report confirms all processes stopped and the owned cluster root was
removed. No retained production database or frozen dc70 store artifact changed.

Friend cases cover committed block versus plain and receipt follow, pending
request creation, accept/block lock ordering and cancellation, block rollback,
bidirectional blocking and unblock non-restoration. Concert cases cover exact
introduction/revision/age bounds, latest unsupported claims without fallback,
provider outage, verified requirements for unbounded sharing, source-lock
retention, original deadline under lock contention, and retained SocialService
consent/encryption/reveal with current claim invalidation.

These remain synthetic owner/provider fences over real local SQL and retained
service logic. The real Auth/Friend scope join, independently verified age
provider, runtime intro/claim composition, public routes, seven-principal QA04
matrix, WebSocket revocation and cached-product UI are NOT_RUN for this increment.
No original task or persistent goal is completed by this adapter qualification.


Publication reconciliation incorporated peer root/operator source through
`95afbfc9`. The nine assigned adapter paths remain byte-identical to the qualified
candidate except this evidence document. The frozen native report continues to
name compiled source `600a65c9`; it does not claim recompilation or qualification
of newer root/profile callback changes. Assigned Rust formatting and whitespace
checks passed again after the merge. Those runtime joins remain open above.


## Runtime bridge successor (IN_PROGRESS; qualification NOT_RUN)

After the initial qualified publication, exact ownership was transferred for new
`src/social_live/**`, `tests/social_live*.rs`, `tests/social_live_support/**`,
Social migration `0036_social_effect_witness.sql`, additive transaction observer
and Intro-edit helper changes in retained `relationship/social.rs`, and an
additive default `CommandProducer::completed` hook plus normal/verified-replay
pre-COMMIT invocations in retained `repository/receipt.rs`. Root retains module
export, operation registration, router/configuration and resolver composition.
The default completed hook preserves historical behavior and supplies no witness
or settlement proof for producers which do not override it. No lookup hook was
added: the Social producer uses execute, not the retained lookup path.

Authored runtime source covers all thirteen frozen C04 operations, exact parent
protobuf/context/guard/body binding, closed route/query/JSON input, managed
handle/command/cursor key injection, signed actor/event/filter-bound pagination,
explicit metadata projections and only the dedicated private reveal DTO. It
reuses retained SocialService and BlockCommand; no copied consent/encryption or
idempotency implementation is substituted. All replies pass through Root's held
scope and no-store delivery/EOF path. Real positive entry remains NOT_RUN.

The observer registers the actual SQLx transaction with both owners before
policy/effect work. Immutable 0036 witnesses occur only after successful local
mutation or projection. Verified receipt replay has its own projection witness;
raw committed xid without exact persisted native identity/scope/effect witness
remains UNKNOWN. A deferred exact successful-receipt check rejects a witness
followed by failed/pending work. Source also handles BE15 savepoint subtransaction
xmin rather than equating it to the top-level xid; the visible-row/in-progress
proof and actual savepoint rollback require the new native qualification.

`revoke_intro_consent_in(c,event,intro_id,owner,new_revision)` is for the trusted
BE27 write adapter after actual Event FOR UPDATE -> Intro FOR UPDATE/mutation.
It requires same-transaction source revision and pending Intro receipt, revokes
older bound requests/grants and preserves the retained Intro invalidation result.
BE15 queues that exact resource/revision in the existing outbox in the same
transaction. No extra queue or fabricated Social receipt is introduced. The
helper's own-visible-xmin SQL function is a dependency from migration 0036.
Actual dependent Social cache/WebSocket invalidation consumption is still open.

Additional tests and an owned frozen-binary runner are authored for hook timing,
verified replay, effect/receipt/witness rollback, native commit/abort/in-progress,
missing/wrong/immutable witnesses, savepoint rollback and Intro edit atomic
revocation/outbox/replay/non-restoration, plus retained BE15 regression tests.
Rust parsing/formatting, Python AST and whitespace checks pass; these successor
sources have NOT yet compiled or run. The initial 600a65c9 report above does not
qualify them. No persistent goal completion or successor publication is claimed.


### Persisted provenance and QA04 continuation

The original BE26 source invariant distinguishes new-verification availability
from validity of an already persisted canonical claim. The initial fixture's
`available=false` represented an unavailable trusted source; its report's broad
"provider outage" wording must not be interpreted as a requirement to revoke a
valid claim when new verification is temporarily disabled. The successor adds
explicit immutable `ConfiguredSharingVerifier` accepted issuer/revision
configuration. It creates no claim or user/session authority, and it cannot be
mutated during held scopes. New verification outage alone leaves current verified
claims usable; latest unsupported/replaced/expired claims still fail closed.
Missing trusted provenance configuration retains the unavailable default.
A fresh native regression for this distinction is pending, alongside the six
other eligibility/service cases. There is no commercial-provider activation gate
on unrelated World use or a previously valid accepted claim.

The backend QA04 entry point now exists under
`tests/authorization/social_matrix.rs` and its exact support directory, with only
an include hook in the assigned `social_remaining.rs`. It requires seven actual
canonical principals and the runtime owner's private isolated seed plan and calls
registered public operations only. Missing-plan preflight was executed: expected
exit 2/NOT_RUN, zero HTTP requests. The live ignored test is explicitly excluded
from synthetic native runs. Successful HTTP alone remains PARTIAL; unavailable
WS/cache/schema/process-binding gates are stated rather than replaced with fake
principals or fixture authorization. Current real HTTP/WS/cache matrix: NOT_RUN.

The next frozen build request covers `social_live`, `social_remaining` and
retained `command_receipts`: six pure input/cursor/recovery-shape tests, nine native bridge/
observer/witness/Intro/read-exclusion tests, seven native eligibility/service tests and seventeen
retained receipt tests. No additional target lease has been acquired for it yet.

### Response-lifetime source exclusion successor

Source tracing found a gap before runtime qualification: service SQL transactions
commit before Root's actual HTTP response completion. Canonical Auth/Friend
exclusions alone do not retain local Event, Social, Intro or age-claim sources.
Root explicitly requested a separate `SOCIAL_READ` child, not reuse of a public
profile child or preparation of the tracked producer effect transaction. Root
settles tracked native effects before releasing read children; preparing the
effect transaction itself would deadlock that order.

The new assigned `social_live/read_child.rs` acquires compatible source locks
before the projecting transaction commits, records a durable child intent before
PREPARE and attaches the child to the actual scope. It never registers that
read-only prepared child as a native effect. Root retains it until settled
response completion/recovery. Source identities stay private and in memory.
The closed proposed recovery shape is `SOCIAL_READ {query: ChildQuery,
child_nonce, native: ChildNativeIdentity, valid_until_unix_micros}` with fixed
GID `mcr-social-read:{nonce.simple()}`. Current SDK absence is a pre-PREPARE
failure, not an alternate child kind or successful runtime fallback. Root owns
the SDK enum, ledger kind migration and CompositeChildSettler dispatch; no such
shared files are modified in this patch.

The child takes Event SHARE, sorted participant SHARE, and for private reveal
sorted canonical age-owner SHARE followed by Intro, reciprocal grants and handle
SHARE. NOWAIT prevents waiting behind a writer which is waiting on the projector.
It deliberately takes no pair/request lock because retained projection holds
UPDATE there. Retained Social, block, Intro and lifecycle writes take Event
UPDATE first; canonical age replacement takes age-owner UPDATE. Recovery checks
immutable system/database/native xid identity and proves actual ABORTED state;
a missing GID or expired response alone is insufficient. It shares the existing
101-slot local prepared-child capacity budget and requires 256 prepared slots.
Root coordination of that resource namespace and the new closed kind is pending.

Source validity must cover the entire original Root deadline. A shorter-lived
claim, birthday, Intro or consent deadline refuses delivery rather than extending
it. Ordinary owned request metadata list/detail now uses the existing read policy
field so READ_ONLY does not incorrectly disable metadata reads; mutation and
private reveal checks retain their stricter policy. No general private-field
authorization or participant/admin bypass is introduced.

Two additional native cases cover actual prepared source locks surviving the
projector commit, exclusion of Event/age-owner/member/Intro/handle/grant writes,
identity mismatch and idempotent exact-abort recovery, and missing-GID
in-progress/committed/aborted distinctions. These use explicit synthetic recovery
metadata and cannot certify Root intent admission or real HTTP EOF. The runner
enables prepared transactions only in its owned private cluster. Fixture seeding
uses retained PENDING-to-APPROVED transitions, exact request-bound grant expiry
and an explicitly synthetic deadline. All successor Cargo/native execution is
still NOT_RUN at this checkpoint; the earlier published adapter evidence remains
limited to its exact source and artifacts.

### First runtime compile and corrective native pass

Clean source `9fa66b90dadd988a3e411c2c0342f57beac4b850` compiled all three
requested test executables, exit 0, Rust 1.88, locked/offline, two jobs,
debug/incremental disabled. An initial stale protobuf include path was resolved
by the allocated `contracts-build` package-only cleanup. Frozen artifacts and
886 source hashes are under `/private/tmp/mcr-social-runtime-p4uduuqu/`.
The shared MEDIA target was explicitly released with zero open handles before
starting the copied executables on owned PostgreSQL.

| Frozen executable | SHA256 |
| --- | --- |
| `live-tests` | `04d97614f26a85e7dae9cb9e1f2c71e120878f0e998c0ca499d6432658b9b3cd` |
| `remaining-tests` | `6715fc3d2504316da60b942d0ec0d1016d3cbb610806eb178cb9e625dac2eb8e` |
| `receipts-tests` | `f111e07302c03da7adcfea3014fbbbe62a91c839ca5afcc022a9b1ad1bb7ef55` |

Six pure cases passed. Native qualification FAILED in `before-restart-live`:
the owned SQL xmin helper incorrectly assumed a child xmin could not exceed the
parent xid, subtracted an epoch and passed a huge future xid to PostgreSQL. The
report is `native-report.json`; restart and subsequent native groups were NOT_RUN.
Owned cluster shutdown/removal passed. No native success or full-runtime handoff
is claimed from this compile. Full stdout was not retained by the first failure
wrapper; the successor runner records sanitized per-group results before raising.

The corrective source extends an own visible child xmin forward from its parent
using a bounded modular distance. Invalid/bootstrap/frozen, old committed and
invalid future candidates return false; no error becomes positive evidence.
Actual savepoint effect/rollback checks remain required, and no reset/wraparound
cluster qualification is claimed. The PostgreSQL 16 implementation documents the
top-level `pg_current_xact_id` and recent-xid status boundaries in
[xid8funcs.c](https://github.com/postgres/postgres/blob/REL_16_STABLE/src/backend/utils/adt/xid8funcs.c).

Root granted exactly the retained reveal query's `Interaction` to `PublicProfile`
field correction to honor the frozen READ policy in READ_ONLY. All explicit
reciprocal/private checks remain. A new native case checks both valid reveals,
denied current-version handle mutation, block/unblock non-restoration, revocation
and actual expiry in READ_ONLY. The next inventory is six pure cases and
9 + 8 + 17 native cases per phase (68 across restart).

The runtime constructor now accepts only immutable `SocialClaimProvenance`
(`Unavailable` or an explicit `ConfiguredSharingVerifier` snapshot), rather than
an arbitrary mutable trust port which could revoke after the projector commits.
Unavailable trust denies sharing without disabling own metadata or World.
Configured snapshots must remain retained until held scopes drain. New
verification availability is separate from persisted accepted provenance.

Read-child SQL budget/native identity/abort recovery logic was adapted from the
retained MCR `adapters/out/public_profile/local.rs`, source commit
`cabec4135a03ea090ec3cc2f9c7e135f4503e7dc`, SHA256
`014e4cf67384dfb8bed16ba17c65b3df61fa70afeaf38ae955a99d479a0f4f80`.
The destination is the new `social_live/read_child.rs` with its own closed kind,
GID and source locks, existing SQLx/platform dependencies, and the new native
source-exclusion/recovery tests. No legacy rendering or source archive was used.
Root accepts the shape and combined 101-slot local read-child budget; Root
reserved migration **0042** for shared kind admission, not a rewrite of 0037.
Until that actual SDK/migration/recovery wiring is delivered, unknown-kind
conversion still fails before PREPARE. The corrective successor has not yet
compiled or run at this documentation checkpoint.

### Root SDK handoff and corrected SQL probe

The corrective SQL-only diagnostic at clean `7db560be` passed six checks on an
owned PostgreSQL 16 cluster: top-level effect, a real newer savepoint xmin,
savepoint rollback, a released child while its parent remains open, rejection of
committed rows, and rejection of invalid/bootstrap/frozen xids. Evidence:
`/private/tmp/mcr-social-runtime-p4uduuqu/sql-fix-probe.json`. Cleanup passed.
This is a SQL diagnostic, not fresh compiled-successor or full native acceptance.
The fixture-only `1b465415` correction resets synthetic trust availability for
each serial case so an outage case cannot contaminate later/restart cases.

Root explicitly handed off committed SDK source
`25a8742638d47f940b44df638d7cca968ad96af1`; it was merged as `9aa2be2d`.
The merge carries the Root delivery ancestry; the resulting delta against that
Root commit contains only the assigned Concert paths and this document. Root's
shared handoff is source-ready and had owner-reported SQL/restart evidence; its
Rust qualification was still NOT_RUN when received. It is not treated as a
qualified production binary here.

The owned adapter now constructs the actual typed
`ChildRecoveryReference::SocialRead`, checks the exact original deadline and GID,
and supplies `SocialReadSettler::new(pool)`, implementing Root's
`SocialReadIntentSettler`. The production composer must install it through
`CompositeChildIntentSettler::with_social`. The private native runner now applies
the received immutable 0037 and new 0042 kind-admission migrations explicitly.
The existing pure recovery-shape case also checks the actual SDK round trip.
No Root file was independently edited by this owner. Fresh compile/native
qualification of this merged source is pending; actual canonical owner scope,
HTTP EOF, seven-principal authorization, WS and cached UI remain separate gates.

### Qualified merged Social source and next runtime boundary

Clean source **`4b6d603a01e4637cbd550eb6fc077f7e5ba35de3`** passed the
three requested test-target build and copied-binary qualification. The build used
Rust 1.88, locked/offline, two jobs, debug/incremental disabled. Initial Cargo
exit 101 was a stale `protoc` include path from `be-world-recipient-producer`;
the allocated `contracts-build` package-only repair succeeded and the identical
build then exited 0 in 23.18 seconds. No broad cleanup was used.

Evidence directory: `/private/tmp/mcr-social-4b-v3zy9j7j/`. It contains
`preflight.json`, `source.json` (923 hashes), both Cargo JSON/stderr/result
records, `scoped-clean.json`, `artifacts.json`, the three copied executables,
`release-preflight.json` and `native-report.json`. MEDIA was unconditionally
released with zero open handles before the private database run.

| Copied executable | SHA256 |
| --- | --- |
| `live-tests` | `ad4ec8816d45410f9e5df88bf5004d3fe267c367f7e6899e12cd4a36ba28e200` |
| `remaining-tests` | `8fb5b5cfedeff458dd4de96724416132b0aa532f69a775c78c3c36d267d68fac` |
| `receipts-tests` | `e8a0c67ef74ccd6046e8c204173b4a820394d267d98b73c516406bf3d6db8fa1` |

`python3 backend/services/concert-service/tests/social_live_support/native.py
--artifacts /private/tmp/mcr-social-4b-v3zy9j7j/artifacts.json
--report /private/tmp/mcr-social-4b-v3zy9j7j/native-report.json` exited 0.
Six pure cases passed; nine live, eight retained eligibility/social and seventeen
command-receipt native cases passed both before and after physical PostgreSQL
16.14 restart: **68 native executions, zero failures**. Persisted receipt,
outbox and witness counts survived restart. Owned cluster shutdown/removal
passed. Report SHA256:
`6e5c8abebe5d7eeb822eecd079a0d7f526e9c6cc1e29d0bfb43744b5df37ea91`.
This supersedes the failed 9fa native checkpoint only for this exact source.

The run qualifies actual SQL savepoint evidence, prepared source exclusions and
exact-abort recovery, and retained READ_ONLY consent/revocation behavior under
the explicitly synthetic authority/provider/ledger fixtures. It does not certify
an acquired canonical `OwnerGuardScope`, Root intent admission and HTTP EOF,
external provider verification, the full production migration set, seven-role
HTTP authorization, WebSockets or cached product UI. Root receives the typed
`SocialReadSettler`/immutable provenance adapter and BE27 receives the qualified
same-transaction Intro invalidation helper; production composition stays with
their assigned owners. The original goal remains active.

The separate QA04 parameter fix in 4b validates `Platform` parameters against
the frozen enum and UUID parameters against non-nil canonical UUIDs. A new
`tests/social_live_support/http_flow.py` provides the next actual two-principal
HTTP flow for all thirteen frozen Social operations, same-key replay, explicit
reciprocal reveal, handle-change invalidation, revoke, event block/unblock
non-restoration, cancel and handle removal. Python syntax and frozen create
status (200) were checked; actual HTTP remains **NOT_RUN**. This successor
script/documentation is outside the qualified 4b source pin and does not alter
the qualified Rust/SQL implementation.

The driver requires an explicitly disposable private runtime plan and canonical
principals; it refuses redirects, uses direct connections, bounds responses,
checks no-store and secret canaries, and emits no private response/token/ID.
The plan must label claim provenance `SYNTHETIC_ACCEPTANCE_FIXTURE` or
`VERIFIABLE_PROVIDER`; the driver cannot attest the supplied owner evidence.
Even an HTTP pass remains PARTIAL for provider provenance, canonical global
Friend blocking, seven-role authorization, Root lifetime, WS and cached UI.

### HTTP contract correction and captured-response qualification

The next requirement audit found a real source mismatch: `social_live/producer.rs`
returned 201 after create, while frozen `createConcertSocialRequest` requires
200. The owned producer now returns 200 for every successful Social operation,
matching all thirteen frozen success statuses. Rustfmt and diff checks passed;
fresh successor Cargo/native qualification remains pending. The earlier 4b
native checks do not certify its public response status.

Both actual HTTP harnesses now invoke the retained offline C04 `Contract` oracle
through new `tests/social_live_support/contract_oracle.py`. It checks captured
targets, response schemas/statuses and existing semantic constraints, rejects
duplicate keys/non-finite JSON, and records all consumed oracle/schema source
hashes. No new schema implementation, dependency, package installation or remote
schema fetch was introduced. This consumes existing contract tooling, not
frontend mocks; source and dependencies remain in the repository.

Eight offline harness probes passed, exit 0: valid frozen approved response,
valid platform target, and refusal of undocumented create 201, an extra private
field, duplicate JSON keys, non-finite JSON, a foreign event and an invalid
platform. Evidence: `/private/tmp/mcr-social-4b-v3zy9j7j/http-oracle-probe.json`.
These are validator checks with zero HTTP calls. Actual registered-route HTTP,
canonical seven-role authorization, Root EOF/cancellation/restart, WebSockets
and cached product UI still require runtime evidence. No completion is inferred
from the improved harness. Root composition remains the next actual prerequisite.

### Explicit Social composition transfer

Root transferred these exact shared slices to this isolated writer: the
`pub mod social_live` export in `src/lib.rs`, `pub mod social_runtime` in
`src/config/mod.rs`, new `config/social_runtime.rs` and
`tests/social_runtime_config.rs`, Social-only registration/input dispatch in
`product_rpc/{mod.rs,query.rs,owner_media.rs}`, and Social-only Gateway routes in
`api-gateway/src/adapters/in/http/concert_product/mod.rs`. Root stopped these
slices until explicit return, while retaining main, product_runtime singleton
unions, all other producer variants, manifests/locks, CI and serving orchestration.
The persistent goal still reports BLOCKED through the available goal API; this
new aligned assignment is being executed without falsely completing or replacing it.

Published `origin/master` was fetched at
`29b1779e84106fcbe1c59f973cb5992d83c4fc00` (including `babbc3f4`) and merged into
the Social branch. Root clarified that Party `5d662b` is a patch hash, not a Git
commit. The exact returned five-path Party patch at
`$MCR_EVIDENCE_ROOT/be-party-runtime-central-return-c7d8eff6/five-central-files.patch`,
SHA256 `5d662bef90afad48a8fd7a1e436040b506fecee4f8e01f34daca4c31a3723d09`,
applied unchanged in separate compatibility commit
`871b55a4cdb9c8074cdefdd5afd061240d3ef5c6`. That is the Social shared-slice
patch base. Party source `c7d8eff6`, return record `7d3e21a3`, and Root 25a SDK
are preserved. The imported Party patch is not claimed as authored Social work.

The new validated factory consumes explicit `ManagedSocialKeys`, an immutable
accepted issuer/revision or unavailable trust, a bounded TTL, and Root's existing
pool/instance/fence/delivery publisher. It returns the actual scoped producer,
canonical owner wrapper, native witness resolver and SocialReadSettler. It
creates no registry, independent singleton, provider claim or synthetic fallback.
Root must install the resolver and read settler in its shared unions before
serving, retain the same registry identity and drain old trust snapshots.

The actual closed RPC operation set now includes exactly thirteen Social cases;
the authenticated original frame is decoded strictly before owner acquisition.
Gateway routes retain inherited Feed/Party/media/commands and add Social private
error protection, exact mutation receipt ownership and bodyless DELETE/browser
intent rules. Command conflicts, invalid transitions and retired idempotency
are projected to their distinct frozen error reasons. Successful creates use 200.
The source adds pure managed-material/trust/TTL/closed-route checks, strict RPC
DELETE/reveal query checks and Gateway frozen-path/header checks. Qualification
of this coherent successor is pending a fresh named allocation; no new target
has been created or accessed, and actual serving/HTTP/EOF/WS/cache remain NOT_RUN.

### Qualified Social composition successor (fe86cafc)

Qualified source: `fe86cafc34cb8cc11e5b9573d4a64e3c868a3732`, with
Social composition commit `94a3715ce12bdd4b3f9a9595309b62e5ea49f62f`
and the narrowly scoped formatting restoration in fe86. The build consumes the
unchanged Party compatibility commit 871b55a4 and Root 25a SDK. All 1,146
frozen source/manifest/lockfile hashes still matched after execution. The
subsequent handoff commit changes only this evidence document.

SL offered and this writer explicitly accepted one exclusive, bounded MEDIA
allocation for exactly seven test executables. Rust 1.88, locked/offline,
`-j 2`, debug info disabled and incremental compilation disabled were retained.
The first Concert attempt exited 101 after 1.30 seconds because the reused
`contracts-build` artifact referenced the former checkout's proto directory.
The explicitly permitted package-only `cargo clean --locked --offline -p
contracts-build` exited 0; no broad clean, new target or application source fix
was used. The retry and Gateway library build both exited 0, in 36.77 and
26.08 seconds respectively. Exact commands and Cargo JSON/stderr are retained
under `/private/tmp/mcr-social-fe86-1ibd8q0l/`:

```text
cargo test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p concert-service --lib --test social_live --test social_remaining --test command_receipts --test social_runtime_config --test product_runtime --no-run --message-format=json
cargo test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p api-gateway --lib --no-run --message-format=json
```

All seven executables were copied and hashed outside the target. MEDIA was
unconditionally released with zero open handles and the owned Cargo processes
finished, before any copied-only tests began. `all-artifacts.json` SHA256:
`57c40bb1d961282fdcc7084f7682da962424d00cea793ed5eab9feccb929080e`.
No target allocation remains held.

| Copied executable | SHA256 |
| --- | --- |
| `concert-lib-tests` | `5fff5d9325e9b1fd1bb6ffea3d98a6c87988fde234f4a731a58fe18ed739d41a` |
| `config-tests` | `78e73eb7ba48f88f1fdb6053de0444298c63e3151bf2962194b6a27477042765` |
| `gateway-lib-tests` | `14ca7862b7d2a2cf9383a326d3a0d27faeec73035df6a2f34b58151dd9e823c3` |
| `live-tests` | `b30787d78482c9d29185f2b10be980209dc0febd763bd55e827c87cadd15b597` |
| `product-tests` | `badec5454da27e569bdb069c4916d771fb532e185f3dcad4224ffb3ef83e2645` |
| `receipts-tests` | `72015f2942290bce0b20629bcfd2ee3c8cc88014352c20e03582849547113799` |
| `remaining-tests` | `8ea9387b01cdf6dcef2bc93f242799e97a51d9c13b0569ebf2b8fe08cbcdb870` |

```text
python3 backend/services/concert-service/tests/social_live_support/native.py --artifacts /private/tmp/mcr-social-fe86-1ibd8q0l/artifacts.json --report /private/tmp/mcr-social-fe86-1ibd8q0l/native-report.json
```

The owned native runner exited 0: **six pure cases and 68 native executions
passed** (nine live, eight retained Social/eligibility, seventeen command-receipt
cases, each native group before and after physical PostgreSQL 16.14 restart).
Receipt, outbox and witness counts survived restart. The owned cluster was shut
down and removed; the report and independent path check confirm its absence.
`native-report.json` SHA256:
`c7c5d420dff53a4a1e6a5f3d37b1c55be3fb7d78e5fc7e2f7cb2fb2cd029b18a`.
The exact selected prerequisite schemas are hashed in that report; this does
not qualify the complete production migration chain.

Copied binaries separately ran with `--test-threads=1 --nocapture`: all five
`config-tests`, all six `product-tests`, five `concert-lib-tests
product_rpc::query::tests`, three `concert-lib-tests social_live::` pure cases
and seven `gateway-lib-tests concert_product::` cases passed, exit 0 for each
group (**26 focused executions**). Two native cases in the library-only Social
filter were correctly ignored; their actual native counterparts passed in the
owned database run. These checks cover the actual closed thirteen-operation
registration, managed trust/material/TTL, strict DELETE/reveal inputs, retained
Party/product routes, exact Gateway receipt/error protection and browser intent.
`focused-report.json` SHA256:
`1da54ff7f98dcd75fdb18ca1d1ea40b288d2593cac1cbcb4c73c2a7de8988fbb`.

The exact eight transferred shared paths are packaged in
`/private/tmp/mcr-social-central-fe86-gcej2zlq/social-central.patch`, based on
871b55a4 after the unchanged Party patch. Patch SHA256:
`afe2d5fc975c3f9c97151f2691f8149e15a22f377399b6544e4b40be80155778`.
The return manifest lists every path and postimage hash. The separately packaged
owned Social core delta preserves the previous source/hook/migration handoffs;
no frontend, contract registry, main, product_runtime union, manifest, lockfile,
CI or source-document changes are included in either Social patch. Root owns
serial integration, resolver/read-settler unions and actual serving composition.

This is qualified implementation and adapter evidence, **not ACCEPTED public
Social delivery**. Synthetic authority/provider/ledger fixtures do not establish
an actual acquired canonical OwnerGuardScope or real provider provenance.
Canonical Root intent admission, full-lifetime HTTP EOF/cancellation/restart,
registered-route thirteen-operation HTTP, seven-role authorization, global
Friend-block behavior, WebSockets and cached product UI remain NOT_RUN. The
READ_ONLY mutation error's generic denial projection also remains a known
contract limitation pending the actual runtime flow. The original BE28–30/BE37
scope is unfinished; its goal API still reports BLOCKED and has not been
falsely completed or replaced.

Delivery note: `$MCR_EVIDENCE_ROOT` redacts the machine-local evidence directory;
the returned patch SHA256 above remains the exact artifact identity.

### Owned READ_ONLY error correction successor (not yet qualified)

Root confirmed unchanged adoption of the exact returned `CommandProducer::completed`
receipt hook as an Operations dependency. Its `receipt.rs` blob remains
`5c52fe67de67fbe9d3a3208efb460d5cb9006dd6`; no receipt hook redesign or shared
slice edit is part of this successor. All eight returned shared postimage hashes
still match the qualified return manifest.

The owned Social producer adds a NORMAL-only phase gate inside the actual
registered command transaction. It registers the native transaction first,
uses the original Root deadline for SQL wait bounds, locks the event FOR UPDATE,
and checks current actor membership/canonical eligibility before recording an
explicit READ_ONLY denial. A private local status discriminator maps that denial
alone to frozen HTTP 409 / EVENT_READ_ONLY. Missing membership, owner denial,
owner outage, generic transition, conflict and other errors retain their existing
projection. Full retained target/pair/consent/revision authorization remains
mandatory after a successful gate; this supplies no operator/system capability.

Frozen NORMAL operations are set handle, create request and decide request.
READ and SAFETY operations bypass this additional gate. Post-command projection
uses a separate read-hook view so a later event phase transition cannot be
misreported as a new command-phase rejection after a successful write. Existing
metadata/reveal source retention and canonical native registration are retained.

New owned tests bind all thirteen write-policy classifications to the frozen
operation registry, distinguish explicit READ_ONLY from generic denial/error
projection, and add one synthetic-authority native case for current actor denial,
owner outage, nonmember refusal, READ/SAFETY bypass, exact event-lock exclusion
and no fabricated receipts/outbox/witnesses. The native runner expects ten live
native cases (70 native executions across its two phases), rather than nine.
These new checks are NOT_RUN pending a fresh named resource allocation. Rustfmt,
Python runner syntax, fixture path resolution, explicit shared-file preservation
and diff checks passed; none is claimed as compile or native acceptance.

The fe86 frozen seven executables and previous reports remain the qualified
baseline. This successor has not accessed a build target, started a runtime,
changed a migration or altered any returned shared slice. Root still owns serving
composition and integration. Actual HTTP/EOF/QA04/WS/cache and the full original
goal remain unfinished.

### Qualified owned READ_ONLY correction (ba9d595c)

Source `ba9d595c5befa49bb6a71189ad37f6919803d094` is now qualified for
its owned local phase gate, error projection and retained regression scope.
Implementation commit 0085569e and exact-runner-inventory correction ba9d595c
are descendants of e084a7b6. This checkpoint supersedes the preceding successor
NOT_RUN statements only within the qualifications and limits below.

SL offered SOCIAL-409-BACKEND-202611040502Z; this writer explicitly accepted
the exact clean ba9 pin, existing BACKEND target, four Concert test executables
and hard 1 GiB whole additional allocation. Fresh preflight verified 38.33 GiB
free, zero BACKEND handles and no Social-owned active compiler session. The
expected peak was 640 MiB. Rust 1.88, locked/offline, j2, debug0 and incremental0
were retained. Exact command:

```text
cargo test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p concert-service --lib --test social_live --test social_remaining --test command_receipts --no-run --message-format=json
```

The first attempt exited 101 for the reused contracts-build proto path from the
prior BE41 checkout. The permitted contracts-build-only clean exited 0. An
external disk-monitor scan then raced a disappearing compiler file; the writer
stopped only its owned compiler process group, preserved the interrupted attempt
and corrected the external monitor. The final attempt exited 0 in 24.06 seconds,
with 82.71 seconds of elapsed allowance recorded. No Rust source fix, pin change,
new target, Gateway/service MAIN, broad clean or shared-path modification occurred.

All four executables and Cargo/source records were frozen outside the target.
The exact 928 source hashes matched after native execution. BACKEND was
unconditionally released with zero handles and the owned compiler exited BEFORE
copied-only tests. No target access occurred during the owned runtime.
Evidence directory: `${SOCIAL_BA9_FROZEN_ROOT}/`.

| Copied executable | SHA256 |
| --- | --- |
| `concert-lib-tests` | `5e2028dfd0736faa66eb81783181f383a36bd3d6dc5864e0d8c083e7c555d0eb` |
| `live-tests` | `d9cd9ae16461e7c3b4105442e93f58489eb58cdb4c07df1854d428c8209f8ed3` |
| `receipts-tests` | `97a4d94715a1a70a4789faa81129a53a8ce87c9eb7a2ac9ab767a69468088f22` |
| `remaining-tests` | `0ce7bbf1a01b49cf05f1d6785e8f695941581d4e4a40571c866d1781615001b1` |

The copied runner exited 0: **eight pure cases and 70 native executions passed**
(ten live, eight retained Social/eligibility and seventeen receipt cases, each
native group before and after physical PostgreSQL 16.14 restart). The new native
phase-gate case passed in both phases, covering current actor denial/outage,
nonmember refusal, READ/SAFETY bypass, event-phase lock exclusion and absence of
fabricated receipts/outbox/witnesses. Persisted counts survived restart. Owned
cluster shutdown/removal passed and its root was independently confirmed absent.
A copied library filter additionally passed its two pure regressions, with its
one native case correctly ignored because that case ran in the owned suite.

`native-report.json` SHA256:
`d8df86932753c7be57c8654562ab276c1c864471366e2fbe24fdf0b0ba51b32d`.
`focused-report.json` SHA256:
`b2497bf69537931af583ea1e1fe91997df2dd0b64340b1d17e7df1ec72302037`.
`all-artifacts.json` SHA256:
`26264d52a4db7aa7f9b5f901b6660f78b26d2006264ad0c8acd6f16aad2a010c`.
Budget monitor recorded peak additional allocation **294,170,624 bytes
(280.54 MiB)**, below 1,073,741,824 bytes; no budget abort. This includes the
fixed final target growth plus copied output and the owned PostgreSQL root.
Its monitor did not inspect the released target. The observed sampled peak is
not a guaranteed measure of unobserved transient peaks between samples.

`qualified-successor.json` records the exact three-path patch, source/postimage
hashes and all build/repair/interruption/release/native/budget receipts. It is
based on the e084 handoff and contains only owned producer, test and runner
changes. All eight returned shared postimages and receipt blob
`5c52fe67de67fbe9d3a3208efb460d5cb9006dd6` remain unchanged. Root may serially
integrate this qualified successor; no main/SL review or acknowledgment gate is
introduced. Root's changed assembly still needs its own combined evidence.

The new native authority is explicitly synthetic. This is not proof of an
actual acquired canonical OwnerGuardScope, real provider acceptance, full
production migration qualification, live thirteen-operation HTTP, seven-role
QA04, full-lifetime EOF/cancellation/restart, global Friend blocking, WebSockets
or cached UI. Those runtime gates and the original goal remain unfinished.

Root serial intake preserves the above worker source/hashes and all previous Root history. This appended qualification is from handoff `66197e2459caddb5dbd43928c9060a6c563384ee`; only the local artifact prefix is made portable. Coherent Root compile and the remaining public gates are NOT_RUN.
