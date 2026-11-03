# BE31–BE32 feed/content implementation handoff

The bounded Concert feed adapter is implemented and validated. BE31/BE32 remain
**IN_PROGRESS** as whole tasks; production forwarding, authority and root
composition are unqualified. This is not IN03 or release acceptance.

Base: `9f72a71a99091fe89411b180af3404be85d963f1`. Worktree:
`/Users/lhs/Desktop/mcr-worktrees/be-feed-content`, branch
`codex/mcr-be-feed-content`. Source milestones: `016f45c`, `59d6aa4`, `2a7113e`,
followed by the final typed-owner/review-fix commit. Only delivery pushes master.

## Ownership and dependency provenance

Feed changes own only these paths:

- `backend/crates/concert-domain/src/feed/mod.rs` and `tests/feed.rs`.
- `backend/crates/concert-application/src/feed/{mod,content}.rs` and `tests/feed.rs`.
- `backend/services/concert-service/src/feed/{mod,cursor,wire,queue,owner}.rs`.
- `backend/services/concert-service/tests/feed_{cursor,wire,owner,postgres}.rs`
  and `tests/support/feed/mod.rs`.
- `backend/services/concert-service/migrations/0010_feed.sql`.
- `backend/scripts/mcr/feed_smoke.py` and this document.
- Exact `pub mod feed;` additions in the three Concert crate library roots.

No Gateway, existing Post implementation, central manifest/lock, registry,
frontend, asset or source-archive file is authored by this owner. No frontend
implementation is imported into the backend.

Delivery explicitly authorized importing Post owner source commit `e975500`
(base `9f72a71`) as a **test dependency**, locally `bbf1b58`. Its ten paths are
Post domain/application new `concert_feed` modules/tests and exact exports;
`backend/proto/post/v1/concert_feed.proto`; both service build-script appends;
and `docs/mcr/backend/post-concert-feed.md`. Post owner remains the writer.
**Do not integrate local `bbf1b58` as another feed handoff**: delivery integrates
the original owner commit once. The typed feed adapter requires that original
proto/build dependency before its final commit is built. Proto SHA-256:
`f90759e4ada1d14169cf49a3d1b8f3dee14c0e5ac5ad6cb1fbc5751cb83eb165`.

Audited inherited sources at the base: `backend/proto/post/v1/post.proto`,
`backend/crates/post-domain/src/{post,comment}.rs`,
`backend/crates/post-application/src/use_cases/mod.rs`, owner receipt/persistence
ports, and BE15 Concert command/receipt/outbox APIs. Existing ordinary Post lacked
Concert event binding, revisions, media/link metadata and edit operations; no
legacy row receives an invented event/revision. Content remains Post-owned.
All feed application code is newly authored; no inherited implementation is copied.
The native harness imports unchanged `migration_smoke.py` (base SHA-256
`a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8`). Its Rust
fixture guard follows the existing Concert command-test marker/inode checks with
a distinct feed database and environment namespace.

## Contract pins

C04 base inputs and tested SHA-256:

| Input under `contracts/domain/v1/` | SHA-256 |
| --- | --- |
| `feed.schema.json` | `9c8d991df0bb707d7a7a2d43d4b132e7fadb878a52633e1c1f338afc9407f8ed` |
| `operations.json` | `42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223` |
| `policy.json` | `508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe` |

`common.schema.json`, generated `mcr-domain.openapi.json`, C03 PublicProfile and
C06 avatar references remain the same closed contracts. No DTO/registry change.

## Implemented behavior

`concert-domain::feed` provides bounded text, at most four distinct media IDs,
typed links, actual event-bound Post/Comment snapshots, safe revisions/errors
and strict descending `(created_at,id)` positions. Content does not implement
Debug. Creation time, not edit time, determines order.

`concert-application::feed::Feed` binds canonical actor, event, session family,
request, original authority digest and original deadline. Observations retain
separate Auth actor/subject, Friend, event and local relationship revisions.
Digests and snapshots are bindings, never capabilities. Current fences re-query
authority after enrichment; production defaults are unavailable.

Reads reject malformed/cross-event/unsorted owner batches and hidden, deleted or
private rows. Linked targets require actual current owner event/audience/
membership/visibility; POST media must be READY, author-owned and currently
publishable. Revoked links/media disappear. No private social, recipient, beacon
or membership activity is automatically projected. Public author DTOs are exact
C03 allowlists; no placeholder avatar/profile/art is invented.

Owner `scanned_through` and `has_more` survive even when every scanned row is
filtered out. The boundary must stay within the immutable upper/last tuple and
at or below the last returned row. One bounded scan has at most 100 candidates;
there are no automatic pagination loops. Every comment route resolves the
actual current parent, including direct comment-ID mutations. Hidden/deleted
parents cannot be recovered through comments. Thread reads recheck their parent.

Prepared commands cover all eight C04 post/comment/reaction operations. Normal
edits/deletions are author-only; moderator commands remain BE39. An input cannot
choose another reaction actor. Canonical BE15 requests preserve exact registered
path/query/body and original key. Creates derive a stable UUIDv8 from a version
marker plus actor/event/key/operation; changed input with the same key conflicts.
CAS expectations remain in the intent so an identical successful edit can reach
receipt lookup after revision advance. First execution rechecks CAS/phase/media/
links under its qualified transaction/owner fence; replays independently
reauthorize current target and actual original parent. Missing target visibility
never yields historical content.

`feed::cursor` uses AES-256-GCM, OS-random nonces and an injected dedicated key.
AAD binds actor/event/operation/collection/filter/before/limit. Ciphertext retains
version, original upper snapshot, strict last tuple and 15-minute expiry. Tokens
are canonical unpadded base64url. Tampering, wrong scope/key, malformed encoding
and expiry fail closed. A cursor never grants access.

`feed::wire` rejects duplicate/unknown/private fields and noncanonical UUID input,
validates exact C04 schemas, and verifies nested author identity. Safe errors do
not expose upstream text. The typed `feed::owner` adapter preserves original
context, tuple scans, resource IDs and current actual revisions. Post's internal
operation UUID is the BE15 receipt UUID, distinct from the public operation name.
Owner replies must match original actor/event/key/operation/post/comment and
expected outcome shape. Missing receipt does not prove nonexecution. The only
shipped transport is unavailable: no raw channel or actor-forwarding shortcut.

## Durable receipt/outbox behavior and limits

Reserved migration 0010 adds protected **intent work**, not a second Post content
store. It preserves exact original session/request/digest/deadline (including its
text precision), resolved parent/revision and payload. Its composite foreign key
binds all receipt scope fields. Receipt and work commit together as PENDING.
There is no remote mutation inside the Concert transaction and no publication
invalidation before verified owner success.

Qualified acknowledgement checks original scope/target/intent/parent/context and
original expiry, then atomically completes the receipt and inserts metadata-only
resource UUID/revision outbox work. Duplicate acknowledgement creates no duplicate
invalidation. A late work-insert failure rolls back receipt and budget admission.
Stored-target authorization is independent of the incoming target, so an
otherwise authorized key reuse against another target returns conflict safely.

`FencedFeedAuthority` separates current authorization, stored-target authorization
and first-execution invariants. Its only production implementation is unavailable.
Bounded remote Auth/Friend observations cannot fence either database's mutation
through commit. Media's Concert-local transaction cannot alone fence a separate
Post database commit. Post also reports inherited Concert actor forwarding is
unqualified. Root authenticated transport, qualified cross-owner fences, actual
media/link owners, dispatcher/reconciliation and exact failed-owner receipt
mapping remain delivery/owner integration work. No automatic dispatch/retry is
enabled. Uncertain outcomes remain Pending. Reviewed expired-work/account-deletion
payload cleanup remains required; no invented destructive retention policy.
Distributed point-in-time read checks do not claim instantaneous global revocation.

## Validation — 2026-11-04, Asia/Seoul

All commands use the assigned worktree. Cargo uses only
`CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend`, Rust **1.88.0**,
`--locked --offline -j 2`, `CARGO_PROFILE_DEV_DEBUG=0` and
`CARGO_PROFILE_TEST_DEBUG=0`. No duplicate Rust target. Cargo was explicitly
released to Post owner before native PostgreSQL ran.

| Command | Exit | Result |
| --- | --- | --- |
| `cargo ... test -p concert-domain -p concert-application --test feed` | 0 | Domain 2/2, application 18/18 |
| `cargo ... test -p concert-service --test feed_cursor --test feed_wire --test feed_owner` | 0 | Cursor 2/2, wire 3/3, owner adapter 2/2 |
| `cargo ... test -p concert-service --test feed_postgres --no-run --message-format=json` | 0 | Exact native executable compiled and copied/pinned |
| `python3 -B backend/scripts/mcr/feed_smoke.py --repository-test /Users/lhs/Desktop/mcr-build/feed/bin/feed-postgres-tests --report /tmp/mcr-feed-native-20261104-01.json` | 0 | Native transaction test 1/1, real PG16.14 restart persistence and cleanup PASS |
| Rustfmt 1.88 `--check` on owned Rust paths | 0 | Formatting |
| Python AST parse of `feed_smoke.py` | 0 | Syntax |
| `python3 scripts/mcr/check-plan.py` | 0 | Planning/provenance only; all 13 archives intact |
| `git diff --check` | 0 | Whitespace |

The first service build exited 101 because a shared cross-checkout build cache
lacked generated `post.concert.v1.rs`. Touching the local imported build script's
mtime forced correct regeneration without any content change; the final compile
and tests above pass. Earlier SQL-only reports `...feed-sql-20261104-01.json` and
`...-02.json` tested earlier migration bytes; the final native receipt below
supersedes them. The harness applies the three SQL files transactionally; it does
not qualify the application's SQLx migration CLI or deployed ledger.

Native report: `/tmp/mcr-feed-native-20261104-01.json`, SHA-256
`78c1b0274ae73cf7f59c1e4551b1594ec9b7e7ff5706f97d7b82e84ef4412ee6`.
Executed binary SHA-256:
`d668391c956b1f8c8f127ec1fc6d3bc56c378193292c5a327642f2f6d1ab3dd9`.
Final migration 0010 SHA-256:
`a0d3ac6672e37a1bff26129317ddaafdecfc18701c751b340df219c0859d7997`.
The receipt records all feed source/test hashes, contract pins and original
revision `bbf1b58` with tested changes not yet committed. Subsequent comparison
found **zero tested-source drift**. The final commit preserves those code bytes.

Native coverage includes default-deny authority; atomic Pending work; replay and
changed-input conflict; stored-target conflict; replacement-context denial;
original expiry; bound owner-DTO acknowledgement; one outbox effect; late-insert
rollback with budget restoration; and persisted receipt/work/outbox counts
`4|4|1` across a real postmaster restart. The Post commit response and policy in
this fixture are explicitly simulated; real SQL does not certify live authority
or a real cross-service Post mutation. The owned cluster was stopped and removed.

A targeted source review found the comment-pattern compile error, eager-CAS replay
problem, stored-target conflict error and incomplete original acknowledgement
binding. Fixes and regressions are included. Live HTTP/RPC forwarding, real
Post/Auth/Friend/media/link composition, two-account IN03, browser/device/load,
cleanup policy and release qualification remain **NOT_RUN/unqualified**.

The same reviewer verified all four original findings resolved after the fixes, with no remaining finding in that narrow recheck. This static review did not rerun tests or assess broader integration.
