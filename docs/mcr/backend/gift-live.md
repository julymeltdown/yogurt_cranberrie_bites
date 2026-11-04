# BE-GIFTS-LIVE production producer

Status: IN_PROGRESS. No compilation, PostgreSQL, canonical-owner or live HTTP acceptance is claimed.

## Assignment and source

Sole producer owner: chat `01a1a267-37a0-7723-a4e9-a3a6f5b27f52`.
Isolated checkout `/Users/lhs/Desktop/mcr-worktrees/be-gifts-live`, branch
`codex/mcr-be-gifts-live`, fresh master base `1cbb1823c506e7fe04d6754f9698ee855f98bc40`.
The original BE41 BLOCKED goal is unfinished and incompatible; it remains unchanged.
This assignment continues manually, without goal replacement or false completion.
BE41 source/branches and published `a93a70ec` are preserved.

Exclusive NEW scope: Concert `src/gift_live/**`, `tests/gift_live*.rs`,
`tests/gift_live_support/**`, backend `scripts/mcr/gift_live*.py`, and this document.
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
revision is a stable actual per-actor security scalar. Await its binding-only
`capture_owner_context_in` helper, published at `30be9c9c4eaf5886b2c9d5850417474101b05aef`,
then use actual `PgMedia::link_owned_in` with a
Gift-scoped MediaAuthority. Uploader-only authority deliberately denies Link and
must not be loosened or reused as a Gift permission.
Public-profile owner reports the Profile producer published at `b09db61e`; the
Concert `PublicProfileReader::read(&GuardedSubject)` assembly is still unqualified
until actual canonical scope/client dependencies are published and validated.

## Validation and remaining work

Authored protocol tests cover all thirteen route/body shapes, authority injection,
missing/duplicate fields, method/event/identity mismatch, strict queries, mutation
keys/content type, code/revision limits, raw input preservation and expired original
stop. Additional authored tests cover opaque cursors, safe reservation/code/claim
projections, frozen error mappings, and actual local Concert resource SQL locks,
shutdown/read-only distinctions, exact request correlation and denied media hooks.
All native tests are NOT_RUN until an existing physical target is explicitly
allocated. No Cargo target, PG cluster, install or global cleanup has been used.
The private Gift resource runner reuses `migration_smoke.py::Cluster` and adapts
the retained `tests/support/community/owned.rs` connection/marker checks at base
`1cbb1823`. Its scope is local Concert resources only; no synthetic Auth/Friend
positive authority is supplied or certified. It runs all current Concert migrations
in its owned socket-only database, verifies source/binary hashes stayed frozen and
requires marker-validated cleanup. The script has not been executed.

Static checks: `rustfmt +1.95.0 --edition 2021` on owned Rust source/tests,
Python `ast.parse` on the new runner and `git diff --check` exit 0. The default
stable rustfmt fails because its rustc-driver dylib is absent; the already-installed
1.95.0 formatter succeeds. No toolchain installation or default change was made.

Remaining implementation: actual request-scoped canonical guard integration and
settlement, mandatory Gift media authority, orchestration/public-profile projection,
bounded cursor handling, exact safe error/receipt responses, real PostgreSQL
stock/expiry/idempotency/claim interleavings, real owner and server-entry qualification.
Mocks or isolated units will not certify live Gifts or the two-user IN02 journey.

Current source additionally wraps actual SDK `GuardedAuthority` with same-connection
`register_transaction_in` before every policy check. The SDK requires BOTH owner
Track acknowledgements and durable confirmation before returning. Current SDK
repeated registration is owner-idempotent; SDK-owned full-identity deduplication
is pending, with no local PID/connection cache invented here. Exact actor/event/
session/request/operation/method/target/body digest and nonextended deadline are
checked against the actual root-supplied scope. Safety cancellation applies the
retained Safety field at receipt authorization as well as apply, preventing the
retained generic ReadGifts callback from accidentally blocking that exact safety
operation after feature shutdown. This adapter is not yet runtime-qualified.
