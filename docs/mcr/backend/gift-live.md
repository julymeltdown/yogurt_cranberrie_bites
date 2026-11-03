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
Existing domain/application/community store, migrations, proto, lib registration,
configuration, manifests, lockfiles and routers remain outside this grant.
Product-root owner retains module export and all actual HTTP/RPC/Gateway composition.

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

## Typed root/authority seams

Root captured input: `GiftIdentity {actor,event,session_family,request_id,deadline}`,
then `GiftRequest::from_verified(identity, operation, method, target, content_type,
idempotency_key, exact_body)`. The intended runtime replies use closed
`GiftReply::Json {status,body}` and C04 public/private projections. Root handles
no-store, original signed parent, origin/browser intent and transport authentication.
No Gift route or feature flag is enabled by this source checkpoint.

Canonical authority owner 207 reports its guard is still IN_PROGRESS/source-only.
Its intended scope must preserve the actual sealed parent/original deadline, hold
actual Auth/Friend fences, and durably register the exact Concert transaction in
both owners BEFORE protected effects. Scope lifetime includes transaction terminal
state and HTTP-body completion; unknown/cancelled settlement remains quarantined.
ReadCurrent observations alone do not satisfy this contract.

Media owner warns that BE37 pair stamps cannot replace uploader policy semantics:
media relationship revision is event-admission restriction revision; authority
revision is a stable actual per-actor security scalar. Await its binding-only
`capture_owner_context_in` helper, then use actual `PgMedia::link_owned_in` with a
Gift-scoped MediaAuthority. Uploader-only authority deliberately denies Link and
must not be loosened or reused as a Gift permission.

## Validation and remaining work

Authored protocol tests cover all thirteen route/body shapes, authority injection,
missing/duplicate fields, method/event/identity mismatch, strict queries, mutation
keys/content type, code/revision limits, raw input preservation and expired original
stop. These tests are NOT_RUN until an existing physical target is explicitly
allocated. No Cargo target, PG cluster, install or global cleanup has been used.

Remaining implementation: actual request-scoped canonical guard integration and
settlement, mandatory Gift media authority, orchestration/public-profile projection,
bounded cursor handling, exact safe error/receipt responses, real PostgreSQL
stock/expiry/idempotency/claim interleavings, real owner and server-entry qualification.
Mocks or isolated units will not certify live Gifts or the two-user IN02 journey.
