# BE-GIFTS-LIVE production producer

Status: IN_PROGRESS. Business/status source compiles at `e9b761fb`; all 29 focused checks pass. Canonical-owner Gift business flows and live HTTP acceptance remain NOT_RUN.

The current 29-test proof is pinned to `e9b761fb90bc02db570c2aee8f999d3a8b48f993`,
with exact executable and 1,639 source hashes. It supersedes the earlier local
20-test proof for baseline `2363d2cc`; neither proof certifies real HTTP delivery.

## Assignment and source

Sole producer owner: chat `01a1a267-37a0-7723-a4e9-a3a6f5b27f52`.
Isolated checkout `/Users/lhs/Desktop/mcr-worktrees/be-gifts-live`, branch
`codex/mcr-be-gifts-live`, fresh master base `1cbb1823c506e7fe04d6754f9698ee855f98bc40`.
The original BE41 BLOCKED goal is unfinished and incompatible; it remains unchanged.
This assignment continues manually, without goal replacement or false completion.
BE41 source/branches and published `a93a70ec` are preserved.

Exclusive NEW scope: Concert `src/gift_live/**`, `tests/gift_live*.rs`,
`tests/gift_live_support/**`, backend `scripts/mcr/gift_live*.py`, and this document.
Root subsequently granted exactly migration `0033_gift_command_targets.sql` for
immutable failed/pending command-parent recovery. Other migration paths remain
unassigned. The additional grant is not yet returned or published.
Those paths were verified unoccupied across all registered worktrees before writing.
Existing domain/application/community store, migrations, proto,
configuration, manifests, lockfiles and routers remain outside this grant.
Product root granted exactly one additional line, `pub mod gift_live;`, in this
isolated checkout's `src/lib.rs`; all existing lines are preserved. All other
module exports and actual HTTP/RPC/Gateway composition stay root-owned.

Objective: canonical session/relationship/resource authority and actual server
orchestration for the frozen C04 giveaway/reservation/pickup operations over retained
stock/idempotency/claim logic; relevant native/SQL and production-owner validation,
provenance/evidence, committed non-force scoped master publication and fixes.
Historical community `eac0b6ae` is source input, not current live acceptance.

## Current implementation

The strict typed protocol boundary implements all thirteen frozen C04 operation
names/routes, exact method and event/UUID correlation, required idempotency keys,
closed body/query schemas, duplicate-key rejection, bounded requests and original
wall/monotonic deadlines. Exact raw body/target/key remain available to the actual
owner scope. Actor/session/event come from captured transport identity, never body
fields. Constructing this identity is correlation, not authentication. Pickup code
requests and replies intentionally have no Debug/Serialize derivation.

Local Concert resource adapter holds actual event/settings/cutoff locks on the
caller's connection. Safety cancellation remains possible after feature shutdown;
ordinary writes deny READ_ONLY. Physical place selection calls the actual BE08
adapter through a nested savepoint, preserving locks to the outer commit, checks
current map/place revisions, review/provenance/open state and meeting expiry.
This adaptation follows BE09 `meeting/beacon.rs::select_place` at the stated base.
No virtual coordinate implies physical arrival. Media linking requires a mandatory
owning adapter; no permissive fallback or direct media SQL bypass is supplied.

Gift cursors use dedicated-key AES-256-GCM, domain-separated authenticated bindings
for actor/event/collection/filters/limit and management giveaway. Timestamp/row IDs
are ciphertext. Fifteen-minute expiry, noncanonical encoding, tampering and
cross-user/collection reuse are rejected. This adapts the retained
`feed/cursor.rs` at base `1cbb1823`, without borrowing Feed filter semantics.

Explicit safe C04 projections omit internal reservation code metadata. Actual
`PublicAuthor` bytes pass the existing `feed/wire.rs::author` C03 allowlist and
exact-subject check; no owner profile fallback is invented. Final success replies
are schema-checked for the exact operation/status. The schema-cache pattern is
adapted from retained `feed/wire.rs` at the same base. Error reasons/statuses match
`contracts/domain/v1/policy.json`, including expired reservation 410 and invalid
pickup code 403. Plaintext code projection consumes the one-shot `IssuedCode`.

## Typed root/authority seams

Root captured input: `GiftIdentity {actor,event,session_family,request_id,deadline}`,
then `GiftRequest::from_verified(identity, operation, method, target, content_type,
idempotency_key, exact_body)`. The intended runtime replies use closed
`GiftReply::Json {status,body}` and C04 public/private projections. Root handles
no-store, original signed parent, origin/browser intent and transport authentication.
No Gift route or feature flag is enabled by this source checkpoint.

Canonical authority SDK is published and compiled at `888cb6bd30846d891286b92a9a66e921e8d1ce78`; actual full owner/HTTP acceptance remains open.
Its concrete source API accepts root-supplied `Arc<OwnerGuardScope>`;
`register_transaction_in` registers the actual same-connection Concert transaction
and `GuardedAuthority` refuses unregistered transactions. `scope.run` never
releases. The scope must preserve the actual sealed parent/original deadline, hold
actual Auth/Friend fences, and durably register the exact Concert transaction in
both owners BEFORE protected effects. Scope lifetime includes transaction terminal
state and HTTP-body completion; unknown/cancelled settlement remains quarantined.
ReadCurrent observations alone do not satisfy this contract. Concert internal
gRPC serialization is not browser HTTP body completion. Root retains an opaque
delivery capability and exact Gateway-only terminal callback; only that callback
may finish the response scope. The published SDK is incorporated; root HTTP completion and Concert PublicProfile reader dependencies remain unqualified.

Media owner warns that BE37 pair stamps cannot replace uploader policy semantics:
media relationship revision is event-admission restriction revision; authority
revision is a stable actual per-actor security scalar. Use its binding-only
`capture_owner_context_in` helper, published at `30be9c9c4eaf5886b2c9d5850417474101b05aef`,
then use actual `PgMedia::link_owned_in` with a
Gift-scoped MediaAuthority. Uploader-only authority deliberately denies Link and
must not be loosened or reused as a Gift permission.
Public-profile owner subsequently published Concert's
`PublicProfileReader::read(&GuardedSubject)` assembly at `9119073a`, with component
validation pinned to `52000b75`. Actual joined canonical/local-child and final
HTTP qualification remains unrun. Gifts incorporates that published revision.

## Validation and remaining work

### Unpublished business-runtime candidate

New `GiftRuntime` source parses all thirteen operations through actual
root-supplied `Arc<OwnerGuardScope>` and retained `PgGifts` mutations. It validates
the exact scope/request binding, runs under the original scope stop and never
releases guards. The constructor requires actual PG, receipt/pickup keys, a
dedicated cursor key, sanitizer and mandatory `GiftProfiles` port accepting only
sealed `GuardedSubject`. No default profile, UUID-only authority or positive
canonical fixture is supplied. `gift_live/profile.rs` implements the port directly
for the actual published Concert `PublicProfileReader`; no independent client or
fallback projection is constructed. The combined Gifts adapter compiles at `e9b761fb`.
Actual joined canonical/local-child and final HTTP qualification remains an
external prerequisite before activation.

Claim requires root's sealed stable keyed network bucket. Root confirms the
current transport has no such field, so absent context returns unavailable before
effects. This is unfinished delivery, not a scope cut. Completed code-issuance
replay without its one-shot response returns `409 INVALID_TRANSITION`; it neither
regenerates a code nor reports SUCCEEDED as pending. `202` is PENDING only. Actual
command-status transport integration remains coordinated with root. Ordinary
snapshot/business operations are independent of the private-disclosure seam.

Private `Pickup` and `IssueCode` currently return unavailable before any store
effect. Their positive code paths are authored but explicitly gated until the
actual SDK3 `GiftPrivate` child, Gift native prepare/recovery and root before-byte
validity clip are qualified. Canonical Auth/Friend exclusion alone does not hold
local reservation, meeting or physical-place locks through delivery. No
`LocalPublic`/avatar child is relabeled as Gift authority. The frozen future child
contains exact query/native identity, nonce, closed PICKUP/CODE kind, giveaway and
reservation IDs, and a disclosure `valid_until`; no code/hash/meeting text enters
the recovery reference. Business DTO expiry values remain their original values.

### Command status and immutable parent correlation (source candidate)

`GiftRuntime::command_status(actual_scope, RegisteredOperation, key)` validates the
sealed outer `getConcertCommandStatus` GET binding, empty-body digest and exact
once-decoded closed query. It accepts only the seven registered Gift mutations.
It never fabricates an original mutation or an expected-target browser field.
The readonly producer joins a real receipt to its immutable stored target/root,
locks and validates the current aggregate and relevant reservation, checks actual
owner/recipient roles, then uses real canonical/local authority and the retained
safe receipt repository. Historical committed outcomes can be returned; no
plaintext code or private meeting is recovered from a receipt.

Migration 0033 consumes an exact transaction-local context installed only after
the actual live request's canonical/local authorization. Receipt INSERT stores the
immutable actor/event/operation/key/target/giveaway/receipt correlation on the same
transaction, before the command-effect savepoint. Thus FAILED or PENDING Reserve
keeps its actual root even if no reservation row was created. Whole-transaction
rollback removes both rows. Exact duplicates are idempotent; changed correlation,
mapping UPDATE/DELETE and root rebinding are denied. There is no code/body/default
UUID or request-hash inference. Live context installation first checks both actual
tables and the enabled receipt trigger while retaining table locks against DDL;
missing or disabled migration fails before receipt admission. Successful live
return also requires the exact persisted receipt mapping. Historical unwrapped
headless producers receive no mapping and are not promoted to a live status grant.

`gift_live_targets.py` executes only 0033/native SQL on a new owned socket-only PG,
including restart and cleanup. It does not use Cargo or a shared target. The first
run exposed a test-harness PL/pgSQL `SELECT` without a destination after four cases;
the harness was corrected to `PERFORM`. The next run passed ten cases and cleanup.
The final run passed all eleven cases, including an exact duplicate, with PG
16.14, command exit 0 and cleanup PASS. After incorporating published Profile/root
changes, the complete Concert migration set was rerun at clean committed source
`f4418be168939912a600046b3c70bc74bfbe4dfa`: eleven cases PASS and cleanup PASS.
Committed `target-sql-evidence.json` records that source and all exact migration
hashes; 0033 SHA-256 is
`f3f07358a78b7d515788f6fe289b6a42bb9b0951035d26cb754278c44844bb12`.
This is standalone SQL proof. Separately, the Rust correlation/status caller
compiles at `e9b761fb`, and its three native correlation tests and status-query
test pass as part of the 29-test focused run below.

The candidate media adapter captures the uploader's exact admission/security
semantics, durably registers the same transaction, validates actual self interaction
policy, domain transition, settings/cutoff and the exact requested image/target.
New create targets are admitted only through validated server-derived create intent
before retained row persistence. Actual `PgMedia::link_owned_in` remains the media
writer on the same connection, so later Gift failure rolls back the link. Media
owner's announced `prelock_target_in` extension is not yet published and must be
implemented explicitly on that exact pin; no default-linked fallback is assumed.

Frozen C04 requires `created_at DESC,id DESC` and a fixed upper snapshot; retained
Gift list/management use ascending order. New Gift-owned queries implement the
contract without changing retained stores. They validate aggregate/header identity
and counters, reauthorize each selected subject, bind cursors to operation as well
as actor/event/collection/filters/limit, and keep the upper boundary encrypted in
version-2 tokens. Reservation creation time is read from the actual retained
aggregate. Root resolution uses an internal unlocked hint, then locks Gift root
before rechecking/locking the exact reservation, matching mutation lock order.
Management invokes retained durable DB-clock readiness/expiry transitions before
filtering and projecting; large-page/deadline behavior still requires native proof.

The candidate incorporates published canonical SDK2 `c4ad561c`, whose exact full
acknowledged transaction deduplication replaces repeated network tracking; no local
PID/connection cache is added. Five compiled/frozen binaries cover 8 protocol,
5 cursor, 5 response, 4 local resource, 3 native query, 3 native correlation and
1 status-query test (29 total). Native query/correlation tests
exercise actual SQL plus retained Gift mutations with synthetic test-only policy;
they do not construct canonical scope/subject fixtures or certify Auth/Friend.

### Current runtime/local SQL qualification

At exact clean source `e9b761fb90bc02db570c2aee8f999d3a8b48f993`, Rust 1.88.0
compiled the four Gift integration-test binaries and Concert's lib-test binary
with locked/offline dependencies, two jobs, debug 0 and incremental 0. Build
command: `cargo +1.88.0 test --locked --offline -j 2 -p concert-service --lib
--test gift_live_protocol --test gift_live_cursor --test gift_live_response
--test gift_live_resources --no-run --message-format=json-render-diagnostics`.
The build exited 0 in 27.3 seconds and produced no Gift-owned warnings. Inherited
Concert lib/lib-test warnings remain outside this assignment.

The initial build failed because the shared target's cached `contracts-build`
embedded the previous Browser checkout's protobuf include root. Package-only
cleanup on the explicitly allocated MEDIA target removed 20 files / 816.4 KiB;
the unchanged command then passed. An earlier cleanup dry run omitted
`CARGO_TARGET_DIR` and created rustc metadata, a cache tag and an empty lock under
the checkout's default target path. Their exact metadata-only tree was verified
created during this run and removed; no compilation used that directory. All
compilation used the one allocated MEDIA target. No toolchain was installed.
All five executables, full Cargo JSON/diagnostics and 1,639 source hashes were
frozen privately before explicit target release, with zero target handles.
Native execution used only those copies after release.

Results (all command exits 0): protocol 8, cursor 5, safe response projection 5,
status-query 1, actual local PG resource 4, native query 3, and native correlation
3: **29 PASS**. PostgreSQL 16.14 used only a marker/inode-owned socket directory,
all current Concert migrations and synthetic policy explicitly confined to the
local query/correlation tests. The latter exercise all seven parsed mutation
contexts, atomic receipt mapping, absent/disabled migration, context rebinding and
reset, current creation-order pagination and durable readiness. Strict private-PG
cleanup passed. `runtime-evidence.json` records sanitized commands, outputs,
executable hashes and limits; `runtime-source-pins.json` records exact source pins.

These checks compile the real canonical/Profile/media adapters but do not execute
the actual Auth/Friend service chain, joined Profile children, image linking,
Gift-specific reviewed-place closure race, HTTP terminal settlement or IN02.
Private pickup/code gates remain closed and Claim still requires root's actual
signed trusted-network context.

### SDK3 Gift-private source candidate (not qualified)

`gift_live/private/` implements the actual SDK3 producer API against SDK source
`0576c359`, PublicProfile unsupported-variant source `56f18703` (local
`929420e0`) and Root ledger/composite-settler source `82ec8b89` (local
`c1ae9126`). These consumed peer sources are not certified by the earlier
`e9b761fb` runtime proof. Gift source checkpoint is `8f8aadc0`. Root's subsequent
minimal source sequence `c9a5081c`, SDK `f646f015`, Root `0797bdd6`, `22590db0`,
`24af845b`, `a975e845` is consumed without editing central files. It supplies
response-wide disclosure clipping, Gateway before-frame expiry checks and signed
trusted-network context source; this is not a joined runtime claim.

Published Media port source `65f90e08` is consumed locally as `b2c1251c`.
The Media documentation conflict was resolved by taking that exact file from
the peer's published `15e7bd8` snapshot, with no authored peer-file changes.
`GiftMediaAdapter` explicitly implements the new prelock port: exact parsed
Create intent or owner Update root is validated and locked before PgMedia locks
its row, and final Link authorization rechecks that same immutable target on the
same transaction. The published media implementation file matches `15e7bd8`.
Actual Gift image-linking runtime remains **NOT_RUN**.

This increment is **IN_PROGRESS**, with compilation and
all newly authored native tests **NOT_RUN** until a target is explicitly leased.

`GiftPrivateRequest::from_request` accepts the exact parsed Pickup or IssueCode
request and actual `OwnerGuardScope`; the opaque value preserves the request
binding and original deadline. `GiftPrivateRuntime::prepare_pickup` and
`prepare_code` take that request and actual scope, returning `GiftDisclosure<T>`.
`into_parts` yields the original business DTO and separate transport clip. The
runtime implements Root's actual `GiftPrivateIntentSettler`; no caller-selected
GID, SQL, callback URL, permissive policy, or replacement scope is accepted.

The producer captures the actual full native transaction identity before local
target locks. Actual canonical actor/owner checks and local event/settings,
pair, admission, participant, root, reservation, map and reviewed-place locks
precede disclosure. Before durable intent acknowledgement, SQL selects only
target/revision/expiry metadata. The exact sealed SDK intent is registered before
selecting private aggregate instructions/identification or the stored code hash,
and before PREPARE. Code disclosure verifies the actual issued code against its
current generation, expiry and keyed hash. This read child deliberately does not
enter generic mutation Track. Business mutations retain their actual Track path.

The prepared child checks the exact cluster/database/full XID and closed GID,
original monotonic stop, database wall clock and content clip. Drop or expiry
does not release it. Recovery accepts only the typed saved Gift intent, requires
the exact full XID to remain in progress before rolling back a matching prepared
transaction, and requires actual full-XID abort afterward. Missing GID, unknown
state, committed state and elapsed time cannot certify settlement. The local
Gift prepared-slot namespace is distinct from PublicProfile's namespace and
requires configured PostgreSQL prepared capacity; this code changes no server
configuration in production.

New owned-PG tests cover native lock retention after Drop, expiry without release,
cluster/database/XID/epoch mismatch, committed versus aborted terminal states,
ordinary pre-PREPARE rollback and two-process restart recovery. Their metadata
is explicitly synthetic and they do not construct a positive canonical scope or
certify Root's durable intent order. `gift_live_resources.py --private-test`
accepts the same frozen Concert lib binary as `--queries-test`, configures only
its own newly created marker/inode-owned cluster, and requires no prepared
children remain before cleanup. These tests are authored, not executed yet.
SDK peer review identified that the initial private SQL budget could widen an
existing shorter statement timeout. The corrected helper clamps that limit,
treats zero as unbounded and leaves the existing lock timeout unchanged. An
additional actual-SQL test checks both retained limits and a real statement
cancellation, plus the zero-to-bounded case; it is also **NOT_RUN** currently.
Source follow-up preserves the actual scope's shorter wall/monotonic deadline
inside the opaque private request. It also requires the already-materialized
reservation policy pair instead of inserting one inside a prepared read. Missing
pair state fails closed. The native competing update changes indexed and JSON
revision consistently and rolls back its test mutation.

Pickup/IssueCode dispatch remains default-unavailable. Root must compose the
actual Gift settler and enforce the returned clip before every positive response
data frame without rewriting business expiries. Actual canonical joined reads,
durable intent/restart orchestration, Root/Gateway before-byte behavior, signed
Claim network context and the two-user HTTP journey remain **NOT_RUN** here.

### Earlier baseline qualification (historical)

Authored protocol tests cover all thirteen route/body shapes, authority injection,
missing/duplicate fields, method/event/identity mismatch, strict queries, mutation
keys/content type, code/revision limits, raw input preservation and expired original
stop. Additional authored tests cover opaque cursors, safe reservation/code/claim
projections, frozen error mappings, and actual local Concert resource SQL locks,
shutdown/read-only distinctions, exact request correlation and denied media hooks.
At source `2590d9982788a7eb3197a49ab712cde6689e131b`, all four focused test
binaries compile with Rust 1.88 locked/offline, two jobs, debug and incremental
disabled. Native results: 8 protocol + 4 cursor + 4 safe projection tests PASS;
4 actual PostgreSQL 16.14 local-resource tests PASS; private-cluster cleanup PASS.
The existing BACKEND target was explicitly allocated, binaries and 1,728 source
hashes frozen privately, then the target explicitly released before native runs.
No new target, installation or global cleanup occurred.
The private Gift resource runner reuses `migration_smoke.py::Cluster` and adapts
the retained `tests/support/community/owned.rs` connection/marker checks at base
`1cbb1823`. Its scope is local Concert resources only; no synthetic Auth/Friend
positive authority is supplied or certified. It runs all current Concert migrations
in its owned socket-only database, verifies source/binary hashes stayed frozen and
requires marker-validated cleanup. The runner completed with exit 0. Committed
`tests/gift_live_support/native-evidence.json` records commands, outputs, binary
hashes and limits; adjacent `source-pins.json` records exact dependency hashes.
The first build failed from a stale shared-target `contracts-build` artifact
pointing to another checkout. Package-only cleanup on the allocated target
removed 20 files (816.3 KiB); the unchanged build command then succeeded.

Static checks: `rustfmt +1.95.0 --edition 2021` on owned Rust source/tests,
Python `ast.parse` on the new runner and `git diff --check` exit 0. The default
stable rustfmt fails because its rustc-driver dylib is absent; the already-installed
1.95.0 formatter succeeds. No toolchain installation or default change was made.

Remaining qualification: actual request-scoped canonical guard/settlement,
real Gift media linking and joined public-profile assembly, SDK3 Gift-private
prepare/recovery and root before-byte clipping, signed Claim network context,
real PostgreSQL stock/expiry/idempotency/claim interleavings, and actual HTTP entry.
The physical-place positive path is compiled but has not received
a new native Gift-specific reviewed-place/closure race test in this slice.
Mocks or isolated units will not certify live Gifts or the two-user IN02 journey.

Current source additionally wraps actual SDK `GuardedAuthority` with same-connection
`register_transaction_in` before every policy check. The SDK requires BOTH owner
Track acknowledgements and durable confirmation before returning. Current SDK
full-identity deduplication is published in SDK2 `c4ad561c` and incorporated here,
with no local PID/connection cache invented here. Exact actor/event/
session/request/operation/method/target/body digest and nonextended deadline are
checked against the actual root-supplied scope. Safety cancellation applies the
retained Safety field at receipt authorization as well as apply, preventing the
retained generic ReadGifts callback from accidentally blocking that exact safety
operation after feature shutdown. Actual canonical-owner execution remains unrun.

Root temporary single-line module-export grant was returned after source checkpoint
`92821595`; the registered export and actual module were subsequently compiled at
`2590d998`. No further lib.rs changes are owned or planned by this worker.

Earlier baseline publication reconciliation incorporated peers’ Avatar/Bootstrap/participant-
proto changes, plus their published SELF artifacts. The native proof remains
explicitly pinned to `2590d998`; no final-whole-tree rebuild is claimed after those
peer merges. Gift source, retained Gift/domain/receipt/relationship code, Concert
migrations, canonical SDK, media adapter and frozen contracts are checked unchanged
against those earlier frozen hashes. The newer `e9b761fb` build above qualifies its
own reconciled dependency tree independently. This checkpoint enables no route or flag.
