# Canonical event feed reads

Status: IN_PROGRESS. No live read acceptance is claimed.

## Objective and completion criteria

BE-PUBLIC-FEED-READ delivers event-scoped post list/detail and comment reads
through the existing Post owner store, current actor/subject/event authority and
actual C03 public-author projection. Completion requires implementation,
meaningful guarded/native/HTTP validation, documented evidence, committed direct
non-force master publication, remote ancestry verification and concrete review
fixes. Post, comment and reaction writes remain required later work.

The chat has an incompatible unfinished historical BE13/BE14 goal. It remains
truthfully blocked; this objective does not reset or falsely complete it.

## Ownership and source

The exclusive checkout is `be-world-recipient-producer`, branch
`codex/mcr-be-event-feed-read`. Initial base was
`f4b4080897a6b1ad835fa4e5e9372f82d58e489e`; source was refreshed to `b8b5a939`
before edits and `2ce8b60fdc4b14a14da6fc19a6099b26820df0fc` for the first
adapter qualification. Existing Post persistence, commit fences, Concert feed application,
opaque cursor codec and C04 serializer are retained. No source is copied from
historical product UI or inherited `/api/feed` routes.

The frozen API is `contracts/generated/mcr-domain.openapi.json`: listConcertFeed,
getConcertPost and listConcertComments. A detail request has no query parameters;
its initial comment page uses the default limit 20. Subsequent comment pages use
the listConcertComments cursor binding. PublicProfile permits only user_id,
display_name, nullable bio and canonical AvatarState.

## Required producer boundaries

Canonical owner authority owner `01a1a207-f012-7343-a24b-12b9c544973f` supplies real
Auth/Friend exclusion held through the protected future and transaction commit.
Post must additionally receive qualified current event/participant authority.
A supplied digest, JWT, workload credential, participant row or recent observation
is not this authority. Existing `Owner::new` continues to deny access until the
real production constructor is qualified. Mutation handlers stay unavailable.

Public-profile owner `01a1a205-3f89-78c3-aa67-719c4bb40b8d` supplies the canonical
viewer/subject/event projection. World SELF_PROFILE is not reusable author
permission. FeedResources::author returns PublicAuthor with exact subject UUID
and schema-qualified bytes. Missing author, link or POST-media authority fails
closed; no invented author, URL or success-shaped empty response is permitted.

Root composer `01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08` owns product RPC, Gateway
HTTP, original ingress authorization capture, config/TLS and startup registration.
Typed Context carries bindings only. Production composition must retain the
qualified guard across read, enrichment, final fences and response assembly.

## First validated checkpoint

The compiled groundwork adds `feed::live_read::ReadFeed` for canonical page,
detail and comment orchestration; preserves opaque cursor bindings and immutable
upper/after windows; validates exact C03 author identity/allowlist; and clips the
whole read future to the original absolute deadline. Post read-wire conversion
rejects missing/noncanonical bindings, invalid filters/limits and future windows.
The read settlement registry seals admission, drains explicitly finished work,
and refuses positive settlement for cancelled/unknown work or another process.
It is not owner authority or PostgreSQL transaction settlement by itself.

Production author adapter `feed/resources.rs` is pending and unexported until the
canonical guard and public-profile modules land. Its presence is not compilation
or runtime qualification. The additive qualified Post protocol is preserved as
an external pending artifact while the independent adapter checkpoint retains
the legacy proto. Production Post reads remain closed; no feature switch is enabled.

## Validation

Rust 1.88, `--locked --offline -j 2`, development/test debug 0 and incremental 0:

```sh
cargo test --locked --offline -j 2 -p concert-service -p post-service \
  --test feed_live_read --test concert_feed_live_read --test feed_wire \
  --no-run --message-format=json
```

The Cargo JSON-selected binaries and 1,428 source files were frozen outside the
shared target before execution. The 16 new deterministic adapter cases and three
retained wire regressions passed: **19 passed, zero failed/ignored/filtered**.
The standalone wire build first found a stale cross-checkout proto helper;
scoped cleanup of `contracts-build` removed 20 files (816.3 KiB), and the final
three-target compilation and execution passed. No whole-target clean, third
build tree or dependency install was used.

`backend/scripts/mcr/feed_live_read_smoke.py --adapters-only` checks exact source
and binary hashes, executes that exact inventory with a stripped environment,
and always labels live owner/HTTP/native checks NOT_RUN. Its six Python preflight
cases pass, including substituted binaries, changed sources and skipped tests.

Evidence is external at `/private/tmp/mcr-feed-adapter-20261104/`:
`binaries-v2.json`, `rebuild.jsonl`, `source/`, `adapter-report.json` and
`reproduced-adapters.json`. The initial adapter report SHA-256 is
`ab081dc0e81230c1e23d53d062739d66c8423a47c7d513d8cb4677c2bda0db71`.
Pending qualified IDL is `qualified-protocol.pending.proto`; it is not a live
protocol qualification pin. BACKEND was unconditionally released to the named
media owner after frozen copies and a zero-handle check. No target or PG lease
is retained while awaiting the positive integration prerequisites.

Guarded/native PostgreSQL/live HTTP checks: **NOT_RUN**. Canonical owner guard,
Post qualified read admission/transaction settlement, actual public author
producer integration and root HTTP composition remain required. Post/comment/
reaction writes remain later required scope. This checkpoint is not C04 live
feed acceptance, BE31/BE32 completion or IN01 acceptance.

## Unpublished qualified producer source

The branch now contains a concrete Post production composition, compiled at the
checkpoint below. Concert policy/resources and the joined production path remain
unqualified. The earlier 19 Rust cases cover only the published adapter checkpoint.

Post request admission requires the sealed verified read proof, independently
verified Concert caller, exact original actor/event/session/request/deadline and
mandatory Auth/Friend views plus the opaque Concert root capability. Real mTLS
clients check that root capability, enroll actual authors through the root's
held local subject policy, and check actual independent Auth/Friend epochs,
revisions, per-user security generations and block state. Every successful remote
reply must carry cache-control:no-store. Missing metadata fails unavailable.
The actual compiled canonical SDK prerequisite is published at `888cb6bd`.

The four read query bodies now accept an already-open SQLx transaction. Existing
public wrappers preserve validation and transaction ownership. Qualified reads
capture PostgreSQL full xid, PID, backend start, cluster system identifier and
database OID on the exact transaction passed into those query bodies. Existing
row SHARE locks, per-row admission and checked_commit fences remain in place.
No write or receipt access is granted by the concrete read authority.

The new, exclusively allocated Post migration is
`0032_concert_feed_read_tracking.sql`; its module source is `read_tracking.sql`.
A separate committed control transaction records the exact actual identity and
root binding before Track. Each owner's attempt is committed before its call,
and each received acknowledgement is committed before continuing to the next
owner. Protected reads and the same-transaction witness start only after both
acknowledgements are durable. The producer pool needs at least two connections.
The producer role needs explicit permission to read pg_control_system. Missing
schema/permission fails closed; there is no runtime DDL or URL identity fallback.

A lost Track acknowledgement remains an attempted registration, not an abort.
Only the authenticated actual owner may request settlement, using the exact
stored identity/binding and its durable registration reference. Actual PostgreSQL
ABORTED can resolve a lost acknowledgement without requiring the locally
received UUID. COMMITTED additionally requires the exact committed witness.
Missing witness, forgotten xid, unavailable or different store, different process
instance and any uncertain outcome remain UNKNOWN. Post never supplies positive
scope-absence evidence: the separately retained Concert root must seal its exact
scope and finish the actual Gateway HTTP response before owner release.

The positive generated services have a guarded-only constructor. The dedicated
four-method service requires whole-envelope Concert-to-Post actor delegation;
the shared settlement service admits only Auth/Friend. The original six-method
PostConcertFeedService remains unavailable. No fixture authority constructor or
configuration fallback is introduced. Links/media remain unavailable until their
actual owner authority is qualified; that gap cannot be filtered into false empty
success.

Concert feed policy/resources source remains unexported and uncompiled, pending
the actual joined PublicProfile reader. The generic comment filtering adapter
is exported for its independent adapter test, but is not compiled yet. Policy and projection
share the same retained local child guard; they do not open a second relationship
pair transaction. Comment filtering validates the complete raw owner batch before
filtering authors and retains the original scan boundary. The viewing actor's own
profile need not be public. Actual public-author projection is still required.

The qualified importing IDL and Post two-pass build are on this task branch.
The composer delegated the exact Post build path and owns matching Concert/root
composition. Remaining Concert exports/compilation, real guarded transport,
joined HTTP acceptance and final direct publication remain open. The Post
compilation and native storage-only results are recorded below. No live Feed,
BE31/BE32 or IN01 acceptance is claimed by these source changes.

The task also contains a scoped Concert-to-Post transport using the dedicated
signer, fixed Post mTLS identity, all four qualified envelopes and the original
scope deadline. It supplies the actual scope views/root capability and requires
no-store responses. Mutation and receipt methods retain unavailable defaults.

Five authored native Post storage cases cover a lost Track acknowledgement
followed by actual rollback, committed-without-witness refusal, and durable
acknowledgement ordering plus exact same-transaction committed witness, and
expiry maintenance that preserves live/unknown outcomes. These
explicitly use fixture acknowledgements to test the producer store; they cannot
qualify real owner exclusion or guarded HTTP. They require an explicitly leased
PostgreSQL controller; their actual storage-only execution is recorded below. The frozen runner's
--post-native-storage mode pins one exact library binary/source inventory and
executes all five exact cases, refusing zero execution or skipped success.
Its ten Python preflight cases pass. Concurrent qualified reads reserve a
bounded half-pool permit before opening their protected connection, leaving
control connections for durable acknowledgement writes.

Owner Check/Track transport failures and invalid root capabilities fail the whole
read. Only explicit checked eligibility/block facts or the root callback's
reserved NOT_FOUND subject-denial response are filterable; an invalid guard
cannot manufacture an empty successful page. Qualified comment scans reserve
one nonactor subject slot for the parent author, scanning at most 99 rows with
real SQL lookahead while keeping the public requested limit/cursor binding.
Expired pending records are examined in bounded batches against actual owned
PostgreSQL status; only proven aborts or commits with the exact witness free
pending capacity. Expiry never supplies an outcome or releases an owner guard.
Different producer instances remain unknown and require explicit recovery.

The fifth authored storage case uses 101 distinct comment authors at equal
creation timestamps, with denied authors at both the first row and scan boundary.
It checks actual SQL 99-row scans plus lookahead and continuation for no skipped
or duplicate eligible records; the public requested limit remains 100. Its
cfg(test)-only lock-holding policy fixture does not qualify canonical exclusion.
Future before dates are accepted under the frozen query schema: cursor AAD keeps
the original value, while the first internal snapshot is clipped to current time.
The adapter case verifies fixed-snapshot continuation and rejects changed before
bindings. These new Rust assertions still require actual execution.

The pending adapter inventory is now 20 Rust cases (9 facade/filter, 8 Post
codec/registry, 3 wire), distinct from the historical 19-case published receipt.
The additional case preserves a filtered comment scan boundary and checks that
a projection outage, malformed public author or foreign-parent raw batch cannot
be hidden by author filtering. No new Rust PASS claim is made before execution.
The native runner requires --native-config private JSON tied to the disposable
controller marker/token, directory inode and exact socket-only database URL.
Rust fixtures independently check those bindings and actual server directories
before creating their isolated schema.

SDK2 prerequisite `c4ad561c` is now consumed from published master. Static
comparison confirms the scope/view/subject APIs used by this Feed slice retain
their signatures. SDK2 adds durable child recovery intents and separates its
settlement mutex from admission; this Feed slice does not implement HeldChildGuard
or supply a ScopeLifecycle. Those root/Profile implementations remain with their
owners. The merge changed no owned Feed/Post producer source. No Cargo target or
PostgreSQL controller was allocated or accessed for this compatibility check;
this compatibility check itself did not execute Rust, native or joined HTTP checks.


## Qualified Post compilation and native storage checkpoint

Exact clean source: `4552ccf95695d8a3ff311a3a65b9a767e27ecba0`.
The Post build isolates canonical authority messages in `OUT_DIR/owner_authority`
before generating importing services. This prevents tonic's imported service pass
from overwriting the canonical message output. The transport can access its
crate-private read admission module. No default toolchain, dependency install or
additional target was used.

Rust 1.88 with the existing MEDIA target, locked/offline jobs 2, dev/test debug 0
and incremental 0 compiled Post's library and `concert_feed_live_read` integration
binary with **zero warnings**. The copied executables passed the exact eight Post
adapter cases and three owner-client cases. This is not the unexecuted 20-case
Concert/Post/wire inventory. MEDIA was unconditionally released after copying
both binaries and 1,511 pinned source files and verifying zero open target
handles. Native execution subsequently used only the external copies.

The five exact ignored native storage cases then executed individually against
an exclusively owned PostgreSQL 16.14 socket-only instance: **5 passed, zero
failed or skipped**. Actual full-xid status, missing/exact committed witness,
lost acknowledgement, durable owner acknowledgements, expiry maintenance and
the filtered 99-row comment continuation were exercised. Fixture owner
acknowledgements and the test-only policy do not qualify real canonical owner
RPCs, exclusions, root composition or Gateway HTTP response completion.

The controller verified zero prepared transactions and zero leftover test schemas,
stopped and removed its marker/token/inode-bound instance, and removed its private
connection configuration. Cleanup: **PASS**. No shared PostgreSQL instance or
foreign data directory was used.

External evidence: `/private/tmp/mcr-feed-qualified-20261104/` contains
`post-v3-cargo.jsonl`, `post-v3-cargo.stderr`, `post-binaries.json`,
`source-manifest.json`, `post-checkpoint.json`, `native-binaries.json`,
`native-report.json` and `native-controller-report.json`. Native report SHA-256:
`e73c97c6f92222c1ba61bb0e27c5f733c0ff6b11e14dd34e76a37114d1064f31`.
The copied library SHA-256 is
`522912cbd9e50159fb0afc7920022ab647a38fc3d59722ced6f0e94702d76c74`;
the copied integration binary SHA-256 is
`3495bc1b7c518ebe9828b0cbfbe10d0f7b2db04dc967b93c97358ac2a0e3be79`.

Real guarded integration and live HTTP remain **NOT_RUN**. Before publication,
the importing Post IDL also requires the coherent Concert build/include fix owned
by participant `01a1a209-80da-7e53-a295-b64cfe09c1d7`; publishing it alone would
expose the same canonical overwrite in Concert. The remaining independent
Concert nine adapter cases and three wire cases need their own fresh target lease.
No live Feed, BE31/BE32 or IN01 acceptance is claimed.


## PublicProfile dependency consumption

Published PublicProfile increment `9119073a` is consumed by merge `067e18e6`.
The merge preserved the owned Feed/Post source byte-for-byte. Feed policy and
resources are now exported with `QualifiedReadFeed::connect`, which composes the
actual scoped Post transport, current PublicProfile reader and author-filtering
adapter. This is source only until a fresh Concert compile and joined checks.
The constructor accepts a real retained OwnerGuardScope, fixed Post mTLS identity,
its dedicated read signer, current reader and dedicated cursor codec. Root still
owns actual HTTP completion and settlement; this constructor does not register
or enable a product capability.

A denial ambiguity was found during integration: the SDK uses PermissionDenied
for both ineligible subjects and sealed/stale/mismatched scopes. Feed now treats
PermissionDenied from the SDK as a whole dependency failure; only reserved
NotFound may filter an unavailable nonactor subject. SDK owner `01a1a207` has the
request for a distinct true-subject-denial result. PublicProfile owner `01a1a205`
has the matching correction request for its current broad PermissionDenied to
ResourceUnavailable conversion. Until corrected, that upstream mapping remains
a qualification blocker. No invalid whole scope may become an empty successful
feed response.


## Native Post join dependency

Root composer `01a1a1fc` granted exactly one Concert dev-dependency and its
existing lockfile dependency edge. Commit
`8496730c9abb93cdb0b1cd4d9d4f18b36e954386` adds
`post-service = { path = "../post-service" }` under Concert dev-dependencies and
`post-service` to Concert's existing Cargo.lock package entry. No package,
version, feature or other manifest/lock input changed. TOML parsing, existing
package/path checks and `git diff --check` passed; no Cargo target was accessed.
This enables source composition of the actual Post runtime in the owned native
Feed tests. It is not compile or joined runtime evidence. The actual root
callback implementation and coherent participant-owned three-pass Concert
generation fix remain prerequisites. No substitute positive authority or root
constructor is claimed.


## Concert generation prerequisite consumed

Merge `afa7cc2e44bf0c4c982459b807ae32da4449548e` consumes participant owner's
published `cb057d43fb4535c4a9b5867f228caef64e5b556b` generation fix. Canonical
lease output and its guarded include both use `OUT_DIR/owner_authority`; the
main pass generates SELF alongside the importing Post service, and the separate
Participant pass uses its own output directory with explicit external package
paths. Static inspection and ancestry verification passed. The merge preserved
all owned Feed/Post producer and adapter test source bytes. The participant
owner's compilation does not qualify this branch's importing Post IDL.

A short existing-target lease is requested for the exact three integration
executables: nine Concert facade/filter, eight Post adapter and three wire cases.
This will also compile the newly exported real policy/resources/read composition.
No shared target has been accessed while queued. Real guarded/HTTP checks remain
NOT_RUN, and the unpublished SDK/PublicProfile denial fixes are not yet consumed.


## Real native join staged

Root's published `95afbfc9` is now consumed by merge `ca35b63c`. Its actual
source-bound operator constructor qualification is separate from Feed acceptance.
Duplicate source-cherry conflicts were resolved by retaining the root's published
operator files verbatim. Feed/Post source was unchanged by that merge.

Own commit `635d7efe` stages a real five-owner native component case under
`feed/native_join`: actual signed Gateway parent, canonical Auth/Friend mTLS owner
stores, root ledger and subject callback, local Concert prepared transactions,
actual Profile projection, and qualified Post runtime/settlement. The fixture uses
the public source-bound ProductReleaseBundle constructor and real
PublicProfileReader::connect; it does not copy the private product-art bypass.
Copied helpers retain source SHA-256 provenance from PublicProfile `db92f895` and
canonical `3e22615e`. The signed parent uses the actual release event.

The task-owned native controller creates five exact databases and private mTLS
certificates under one marker/token/inode-bound PostgreSQL instance. It pins the
copied library and complete relevant source/resource hashes, runs the exact
ignored case, requires zero remaining prepared work and verifies migrations,
then stops/removes only its owned cluster. Its report always marks live HTTP
NOT_RUN. The Rust case and controller have not executed: **NOT_RUN**; formatting,
Python syntax and diff checks alone are not compilation or native evidence.
A separate library-test compile is needed for this cfg(test) component in
addition to the queued 20-case adapter build. No target or PostgreSQL lease is held.


## Canonical denial dependency evidence

The current consumed SDK `scope.rs` is byte-identical to both published status
fix `b0c95de3` and the owner's compiled clean `e5f922dd` snapshot. Direct inspection
of `/private/tmp/mcr-canonical-denial-swldhben/native-report-1.json` confirms five
actual native phases passed, including `subject_denial_never_masks_whole_scope_failure`.
Owned-cluster cleanup passed and its root was absent. Report SHA-256:
`6e1242c2f81e20a4282521942574fbb168b08b00e7e8d6c602e606426656be9c`.

The overall report is **FAIL**, because a sixth Product error fixture did not
reach producer dispatch; that owner is diagnosing it. The five bounded passing
phases do not qualify that error wrapper, outer HTTP, or this Feed join. Our
three adapter executables and five-owner native component still need fresh
compilation/execution; no PASS is inferred from unchanged dependency bytes.
Native Feed source inventory now also explicitly pins the character registry;
a read-only check found 1,945 source/resource hashes including the real release,
map/source binding, PNGs and Post tracking migration, with no database started.


## Current PublicProfile join failure

The PublicProfile owner compiled exact `d8eb13bf` and reports 22 copied component
checks passing, but its first actual reader join failed with exit 101 at the
viewer's local policy (`DependencyUnavailable`), before author projection. The
second joined case did not run. Direct inspection confirms the overall report
is FAIL, `joined-owner-reader-hold` exited 101, and the owned server stopped and
root was removed. The report is
`/private/tmp/mcr-public-profile-frozen-y3epk5s_/native-report.json`, SHA-256
`812bd19d77adfde991e9506bcc9959c68eebbde82c38e1b3dbc5a1bb899a7d0c`.

The configured prepared-transaction capacity was 256, so a lower controller limit
has not explained this failure. PublicProfile owner `01a1a205` owns the focused
cfg(test) diagnostics at source `31691c4d` and the actual fix/qualification.
This is a concrete shared-path blocker for our five-owner native Feed case,
which uses the same real local-policy reader. Its failure has not been
reproduced or diagnosed by this Feed task. No Profile, Post/callback or HTTP join
acceptance is inferred from the constructor or 22 component checks. The separate
three-target adapter build remains ready; no resource is held awaiting this fix.


## Failed reader reproduced without a rebuild

Feed reran only the failed reader case from the owner's exact copied `d8eb13bf`
library (SHA-256
`7de7960a470bd48b9eafaed769b60e8dde5a652b6fcc0da086a64756f27b57fa`).
All 665 runtime migration/resource inputs selected from the owner's pin were
verified unchanged. A fresh owned socket-only PostgreSQL instance used capacity
256 and logged error statements. The same case exited 101; cleanup passed.
No shared target or foreign database was accessed.

PostgreSQL reported SQLSTATE `23514`, `relationship authority expired`, at
PREPARE TRANSACTION. Its context was the deferred
`concert_relationship_deadline_guard()` in migration 0006. That trigger requires
the transaction-local `mcr.relationship_authority_until`; current PublicProfile
local budget setup only installs `statement_timeout`. The fix belongs to owner
`01a1a205`: bind the owning transaction setting to the already-held original
scope/owner deadline, preserving the guard and migration. No new deadline,
trigger bypass or migration edit is authorized by this finding.

Diagnostic receipt and private logs are external at
`/private/tmp/mcr-feed-qualified-20261104/profile-sql-diagnostic/`. This is an exact
failure reproduction, not passing qualification. Its controller removed only its
marker/inode-bound owned cluster. The five-owner Feed join remains NOT_RUN until
the owner's real fix and a fresh Feed build/execution.


## Current 20-case adapter checkpoint

Exact clean source `236a283937b391104fe4ba09222edf4c134a2cef` compiled the three
integration targets with Rust 1.88, locked/offline jobs 2, dev/test debug 0 and
incremental 0. This includes the importing Post protocol and exported real Feed
policy/resources/read composition against consumed root and PublicProfile source.
The compile passed with zero errors and zero Feed/Post warnings; 16 warnings
belong to peer World/Party paths and are recorded separately in `warnings.json`.
The lib unit-test target was not compiled by this narrowly allocated run.

Fresh free disk was 52,446,256 KiB. Within the explicitly allocated BACKEND target,
only the stale contracts-build package was cleaned (20 files, 816.3 KiB), then the
build finished in 28.64 seconds. Cargo JSON, 1,584 source files and all three
executables were copied outside the target. An actual zero-open-handle check
preceded unconditional release; there was no subsequent target access.

The frozen runner then executed exactly **20 passed, zero failed/ignored/filtered**:
nine Concert facade/filter cases, eight Post adapter cases and three wire cases.
Future-before fixed snapshots and the raw-batch author-filter boundary case are
now executed, superseding their previous NOT_RUN labels for this exact snapshot.
The actual owner/HTTP/native join remains NOT_RUN in this adapter report; it does
not qualify the staged cfg(test) five-owner library case or PublicProfile's
uncompiled local deadline correction.

External evidence: `/private/tmp/mcr-feed-adapter-current-20261104/` contains
`cargo.jsonl`, `cargo.stderr`, `binaries.json`, `source/`, `source-manifest.json`,
`warnings.json`, `target-handles.json` and `adapter-report.json`. Report SHA-256:
`ba8ed3a5731574fedaf32b23f65c438cb9faef863842831d579570551b2c4c47`.
The historical Post-only native five-case result remains tied to `4552ccf9`.
No positive native/HTTP acceptance or target ownership is carried forward.


## Narrow deadline fix consumed

PublicProfile owner `01a1a205` explicitly granted the exact eight production
lines from `c04070be` with the existing unavailable error mapping for this
consumer. Commit `6d9ef8c436732e208a51142429e4659f0443c4e4` changes only
`public_profile/local.rs`: it pins the exact sealed absolute expiry through the
existing relationship::pin_deadline helper before PREPARE. The existing SQL
helper takes the least of current and supplied deadlines. No diagnostics,
private fixture, migration or trigger change was imported, and ongoing path
ownership remains with the PublicProfile owner.

Consumer file SHA-256:
`328a355ec2782e27208e2044c25447fea142654fcb013493efdbb99823f5ba22`.
Formatting and diff checks passed. Rust/native checks for this changed source are
NOT_RUN; the earlier adapter result qualifies the unchanged `236a2839` snapshot.
The next bounded request is a single Concert library-test build, followed by
frozen copies and a separate owned PostgreSQL run after unconditional target
release. No prior target grant is retained or reused.
