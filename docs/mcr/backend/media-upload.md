# BE24 authorized image pipeline

Status: **IN_PROGRESS** for BE24 as a whole. The scoped internal producer is implemented.
The focused Concert library check passes; producer runtime qualification remains
in progress after the first native failure. The shared Cargo lease was released;
the corrected producer test binary is queued by delivery. HTTP composition, real fenced authority, scheduled cleanup and the real
FE04/Post/Giveaway/Report journeys are **NOT_RUN**. This is not an IN02/IN03 acceptance.

## Scope and provenance

Base: `20503f4b698a28105e718c36e38cfa8fd8fb4a44`; isolated branch
`codex/mcr-be-BE24`, checkout `/Users/lhs/Desktop/mcr-worktrees/be-media-upload`.
Delivery explicitly allocated the media application/outbound adapter, dedicated tests,
script, this document, and migration `0012_media.sql`. Delivery subsequently delegated
four exact shared registrations, separately committed as `24e999520506887951c67cb6d0562a08e48ca892`:

- `backend/crates/concert-application/src/lib.rs`: media export only.
- `backend/services/concert-service/src/adapters/out/mod.rs`: media export only.
- `backend/services/concert-service/Cargo.toml`: retained `image.workspace = true` only.
- `backend/Cargo.lock`: `image` dependency edge for Concert only, no resolution changes.

The pipeline and values are newly authored. The owned fixture guard in
`tests/media_support/mod.rs` is adapted from BE15 `tests/command_receipts.rs` at the base;
it changes the allowed database to `mcr_concert_be_media`. The retained Profile
`src/adapters/out/media/image_avatar_processor.rs` at the base informed the bounded
encoder-output approach, without importing its avatar/GIF assumptions. Existing BE15
`CommandProducer`/`PgCommandRepository`, HMAC fingerprint keys, receipts and outbox
are reused in place. `platform::cpu::AdmissionClass` is reused for bounded worker
admission and cancellation-safe worker permits. The smoke runner imports existing
`migration_smoke.py` ownership, native PG, redaction and cleanup helpers; it never
attaches a running service or a foreign database. No Bigyeting code, old rendering,
source archive, character original, new external dependency or public asset is copied.

Contracts pinned from `contracts/domain/v1/` at the base:

| Input | SHA-256 |
| --- | --- |
| `feed.schema.json` | `9c8d991df0bb707d7a7a2d43d4b132e7fadb878a52633e1c1f338afc9407f8ed` |
| `operations.json` | `42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223` |
| `policy.json` | `508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe` |

## Behavior and integration ports

`concert_application::media` contains framework-free values and the image sanitizer
port. `concert_service::adapters::out::media::PgMedia` implements initialize, raw PUT,
finalize, receipt recovery, intent projection, authorized READY reads, transactional
linkage, privileged owner erasure and bounded orphan cleanup. C04 remains initialize
POST → authenticated same-origin byte PUT → finalize POST. No presigned host or
external bearer transfer exists.

`Context { actor, event, policy: PolicyFence { event_revision,
relationship_revision, authority_revision } }` must come from server identity and an
exact current BE37 decision, never a browser revision. Each component corresponds
individually to BE37 `PolicyDecision::stamp()`. No unrelated revision or collapsed
integer stands in for that stamp. `MediaAuthority::authorize_in(connection, context,
access, owner, target)` is a trusted transaction port. Access carries the stored purpose
for every initialize/write/read/link/delete operation. Its implementation must resolve
current canonical identity, restrictions, participation, block policy, purpose and
current target state/audience; lock or fence those owners through protected commit;
and reject unknown, stale, expired, revoked or deleted authority. REPORT media requires
current reporter/moderator policy even when linked. The production default is
`UnavailableMediaAuthority`; no synthetic success is composed into the service.
BE37 source `b71ae36` provides `PgRelationshipPolicy::authorize_in`, but its actual
remote `FencedRelationshipAuthority` remains an owning-service integration requirement.
A short-lived owner observation is not a revocation fence.

Initialize validates allowed purpose/MIME, 1..10 MiB and typed SHA-256, creates a private
PENDING row, and returns a safe receipt for projection to UploadIntent. The stable
media UUID derives from actor/event/idempotency key, scoped with `MCR-BE24-init-v1`;
it is correlation, not a secret or permission. `intent` freshly authorizes expiry and
revision for the exact same-origin `/bytes` path. Pending lifetime is 900 seconds;
unlinked READY lifetime is 86400 seconds. These bounded implementation constants
are not a legal retention decision. A global reservation of 10 MiB per live record
caps primary retained image bytes below 512 MiB, and each owner/event has at most
16 unlinked records. Quotas include uncleaned expired rows and fail closed until
explicit cleanup runs. They do not bound PostgreSQL WAL/backups/allocator overhead.

The `body::UploadBody` accumulator accepts chunks only up to the recorded length
and 10 MiB, checks actual digest/length on completion, and poisons overrun reuse.
The real HTTP adapter must authorize the intent before buffering, bound concurrent
bodies and total request time, validate exact Content-Type, canonical authentication,
Origin, browser intent and idempotency key, and use this accumulator or an equivalent
bounded streaming body. Content-Length is not trusted. `PgMedia::put` independently
checks stored owner/event/MIME/purpose policy, actual length/digest and current state.
Only UPLOADED private raw bytes are staged; none are readable through a content port.

Finalize checks UPLOADED/expected revision/current full upload fence, decodes and
reencodes inside the same BE15 receipt transaction, rechecks lifetime and authority
after CPU work, applies expiry predicates in the final SQL publication, replaces raw
staging with sanitized bytes, and commits READY plus
safe receipt/outbox together. Failure/cancellation rolls back publication. Same-key
replay returns the same receipt and never runs another aggregate effect. `recover`
returns currently authorized receipt status only; absence/expiry is not rollback
proof or permission to invent another key. Transport still owns the original absolute
request deadline; local SQL limits only shorten existing limits. CPU permits remain
held until actual blocking work ends after caller cancellation.

`link_owned_in(&mut PgConnection, Context, media_id, Target { resource, purpose })`
requires READY, matching event/owner/purpose, unexpired orphan, full current upload
fence and current target authority. It locks the row and binds it to one exact target
within the caller's Concert database transaction; repeated linkage to that same
target is stable. The first link advances media revision. Caller must keep this exact
connection/transaction until its local aggregate or enqueue commit; an adapter using
a separate pool connection is incorrect. Post content persists in a separate Post
database: this port cannot fence that commit. A qualified cross-service media and
relationship protocol is still required for actual Post publication and reads.
A Feed enqueue/link transaction alone does not prove Post publication atomicity. A changed current fence requires requalification; there is
no hidden rebase of stale upload authority. Physical arrival and inventory state
are never inferred from an image.

Reads serve only sanitized READY bytes and freshly authorize the stored target.
Unlinked media is owner-only. Lifetime predicates are checked again in the final byte read and first-link SQL after
authority awaits, so expiry during owner latency cannot extend an orphan lifetime.
New current policy stamps can authorize reads of already
READY media; the stored creation stamp is not a cache grant. `PrivateImage::headers`
provides exact MIME, private/no-store, nosniff, inline disposition and same-origin CORP.
HTTP composition must attach those headers on success and no-store on denied/error
responses, use no public object URL and avoid cross-user caches. The content port is
not an `img src` credential shortcut. Metadata projection must freshly authorize too.

`erase_owner_in` physically nulls staging/sanitized bytes and target references,
leaving a non-readable DELETED tombstone. The caller supplies separately verified
account-deletion authority, transaction and downstream invalidations. The participant
FK also cascades deletion. A scheduler must explicitly call `cleanup(1..100)` to erase
expired PENDING/UPLOADED and expired unlinked READY objects; this module starts no
background worker. Linked target removal/deletion must reauthorize reads immediately
and invoke its cleanup/erasure integration. Backups/WAL and global deletion workflow
remain owning infrastructure responsibilities.

## Codec bounds

The already locked `image` 0.25.10 workspace dependency has default features disabled
and retained gif/jpeg/png/webp codecs. BE24 accepts only JPEG, PNG, WebP; no GIF/SVG/video.
Signature must match declared MIME. PNG APNG and WebP animation are rejected. Header
dimensions and decoder limits apply before full bitmap allocation: maximum 4096 on
each axis, 16000000 pixels, and a 128 MiB decoder allocation budget. The codec documents
allocation limits as best effort; this is not a process RSS isolation guarantee.
Decode is admitted via one service-shared bounded CPU class, never an async executor
or a new pool per request. Root composition must inject a suitably bounded class;
the native fixture uses one worker and zero queue. Decoded pixels go to a fresh
encoder with a seekable output capped before growth at 10 MiB. No EXIF/ICC/XMP/text
metadata or original filename is passed to the encoder. Orientation/color metadata
is removed; color/orientation fidelity beyond pixel decode is not claimed. Reencoding
is content sanitization, not a separate antivirus scanning service.

## Validation and remaining gates

- `python3 scripts/mcr/check-plan.py`: exit 0, PASS; planning provenance only.
- Direct `rustup run 1.88.0 rustfmt --edition 2021` on owned Rust files: exit 0.
- `python3 -m py_compile backend/scripts/mcr/media_upload_smoke.py`: exit 0.
- Owned PG16 schema runner: all five SQL migrations exit 0 and cleanup PASS.
  First run failed a harness-only wrong column-count assertion (23 instead of 21),
  after successful DDL. Corrected rerun passes schema; producer/restart NOT_RUN there.
- `cargo +1.88.0 check --manifest-path backend/Cargo.toml --locked --offline -p concert-service --lib`
  with the shared target/jobs2/debug0: exit 0. Initial test compile exited 101 in
  retained Auth build before media tests: cached cross-worktree `contracts-build`
  mixed old proto includes with current inputs. Delivery authorized mtime-only
  invalidation of current helper and Auth/Friend/Concert build scripts; no bytes,
  cache directories or resolutions changed. Corrected test compile exits 0; codec output-bound test passes 1/1. Private binary
  was copied and SHA-bound before releasing the target to BE13/14. The first native
  run passes two pure raster/body tests but fails twelve PG producers because the
  SQL timeout clamp supplied interval text instead of PostgreSQL's required numeric
  unit string. A separate owned PG cluster reproduces that exact parameter error and
  verifies the explicit `1500ms` correction. Failed run and cleanup are preserved in
  `tests/media_support/native-evidence.json`; corrected binary/native run pending.
- Exact corrected SQL string extracted from `limit_sql_in` and executed against a
  new owned PG16 cluster: unlimited settings clamp to 1500/5000 ms, shorter caller
  settings remain 100/200 ms, and PostgreSQL's maximum 2147483647 ms settings clamp
  without overflow. All three source-query probes and cleanup PASS in
  `tests/media_support/timeout-clamp-evidence.json`. Rust producer rerun remains
  separately NOT_RUN until the explicitly queued correction compile lease.
- Corrected source `b099411` compiled successfully and was privately pinned as
  `d32fd9fb3192edb066014e29a8670cbb8e9df60c252c11504fc8fd3508a60b26` before
  Cargo release. Native suite: 7 passed, 10 failed, 1 restart-only test ignored.
  Full redacted diagnostics confirm all ten failures reached the same fixture
  cleanup DELETE, which violates BE15's deliberate receipt/event restrictive FK.
  No receipt/event retention invariant was weakened. Fixture cleanup now removes
  only its participant/media, retaining receipts/events until owned cluster teardown.
  Failed native receipts remain in `native-corrected-evidence.json` and
  `native-corrected-diagnostics.json`; fixture-corrected binary and restart pending.
  The native harness now requires exact build receipt/binary SHA/current source
  hashes and source ancestry, preventing a stale executable from certifying newer
  source. Detailed diagnostics and machine-readable test summaries are retained.
  Negative admission deliberately supplies the previous binary after changing the
  Rust fixture source: expected exit 1 and exact source-pin mismatch, before any
  cluster is created. `stale-artifact-refusal.json` records that expected refusal;
  it is not a producer or cleanup failure.
- A continuation audit found an expiry-after-authority gap. Final PUT/publication,
  first linkage, private read and intent SQL now check database wall time after
  owner awaits. Exact source SQL executed against expired synthetic rows changes
  zero rows and returns neither private bytes nor intent metadata. All source-query
  expiry guards and cleanup PASS in `tests/media_support/expiry-query-evidence.json`.
  Delayed-authority read/link/finalize Rust regressions are authored; their corrected
  producer binary execution remains queued and is not certified by SQL-only probes.

`tests/media.rs` covers real PostgreSQL effects with a clearly synthetic transaction-
locked policy fixture, plus retained real raster codecs. Cases include private staging,
MIME/digest/oversize/dimension failures, metadata removal, single-frame enforcement,
foreign IDs/private reads, current-policy denial/unavailability, transactional linkage,
receipt uncertainty/replay, policy revocation concurrency, expiry after sanitize,
privacy erasure, bounded cleanup, and a real PostgreSQL stop/start verification.
The fixture policy is not Auth/Profile/BE37 production authority. The smoke runner
applies source SQL in transactions; it does not certify SQLx runtime migration ledger,
service startup/grants, real HTTP routes, browser/device flows, worker scheduling,
load, process RSS limits, global deletion or release readiness. Those must be
qualified on the integrated candidate, with real owners and shared root composition.
