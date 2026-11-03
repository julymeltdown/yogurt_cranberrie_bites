# BE31–BE32 feed/content adapter

Status: **IN_PROGRESS**. This is a bounded implementation handoff, not BE31/BE32
acceptance or IN03 qualification. Base: `9f72a71a99091fe89411b180af3404be85d963f1`.
Worktree: `/Users/lhs/Desktop/mcr-worktrees/be-feed-content`, branch
`codex/mcr-be-feed-content`. Only feed-owned modules/tests, this document and exact
`pub mod feed;` exports are changed. Delivery owns HTTP/RPC/root composition.

## Contract and source pins

C04 inputs are the base versions of `contracts/domain/v1/feed.schema.json`,
`operations.json`, `policy.json`, `common.schema.json` and generated
`contracts/generated/mcr-domain.openapi.json`. C03 PublicProfile and C06 avatar
references remain closed allowlists. No contract registry or source archive is
modified. New code is newly authored; no inherited implementation is copied.

Audited inherited source at the base:

- `backend/proto/post/v1/post.proto`: ordinary Post/Comment/Create/Get/Feed/
  Delete/Like contracts have no Concert event binding, revisions, edit operations,
  media or linked-resource IDs. Its feed is user-scoped, not an event timeline.
- `backend/crates/post-application/src/use_cases/mod.rs`: create/comment owner
  mutations already use owner receipts and transactional event ports; visibility
  and relationship filtering are useful foundations. These remain Post-owned.
- `backend/crates/post-domain/src/post.rs`, `comment.rs` and Post persistence:
  content remains the inherited owner's responsibility. Concert creates no second
  content store and never fabricates a revision for legacy rows.
- BE15 `concert-application/src/commands/` and Concert repository receipt/outbox
  APIs are inspected. Their registry and existing semantics are not modified.

Separate Post owner `01a1a207-f012-7343-a24b-12b9c544973f` owns the new owner
extension. Delivery owns shared interfaces. The live Auth/Friend owner explicitly
states that bounded remote observations cannot fence mutations through Concert
commit. No injected observation is production authority.

## First BE31 slice

`concert-domain::feed` defines bounded content, resource links, event-bound owner
snapshots, safe errors and strict `(created_at,id)` positions. Original creation
time controls order; revisions never reorder content. Text is not Debug-printed.

`concert-application::feed::Feed` depends on explicit `PostReader`, `FeedPolicy`
and `FeedResources` ports. Every request binds canonical actor, event, session
family, request UUID and original expiry. Current observations preserve separate
Auth actor, Auth subject, Friend, local relationship and event revisions. Final
fences must re-query authority, not echo an earlier stamp. All production defaults
return `DependencyUnavailable`.

The read adapter rejects cross-event, malformed or unordered owner batches;
removes hidden/deleted/private rows; and reauthorizes actual public linked
resources plus READY author-owned POST media. It never publishes private social,
recipient, membership or beacon activity automatically. Revoked links/media are
removed. A filtered page can become empty and still have a continuation. One
bounded scan reads at most 100 candidates; there is no hidden automatic loop.
Owner state is rechecked after profile/resource enrichment, then current policy
is fenced. Distributed point-in-time reads do not claim instantaneous revocation
across independent services; production freshness qualification remains required.

`concert_service::feed::cursor` uses AES-256-GCM with a fresh OS-random nonce and
a dedicated injected key. AAD binds actor, event, operation, collection, filter,
before and limit. Ciphertext contains version, immutable upper snapshot, last
strict tuple and 15-minute expiry. Tokens are canonical unpadded base64url;
tampering, context changes, malformed encoding, expired tokens and unavailable
keys fail closed. Cursors never authorize a request.

`concert_service::feed::wire` validates exact generated C04 DTOs and the nested
C03 public author schema. Unknown/private fields, mismatched author identities,
noncanonical UUID inputs, duplicate keys and invalid content are rejected. No
placeholder profile/avatar or fixture content is used in production composition.

## Owner/root integration request

Post owner implements current event-scoped `GetPost`, `ListPosts`, comments and
command/receipt operations through its new owner extension. Minimal snapshots
match the feed-owned Post types: immutable event/author/created_at; actual
revision; public/published/moderation/deletion state; text/media/link IDs;
viewer-specific reaction and safe counts. List order is descending tuple with
an inclusive upper timestamp and strict last tuple, maximum 100 candidates.

Commands require original operation/key, canonical actor/event/request binding,
original resource identity, CAS expected revision (or parent revision for comment
creation), current parent visibility, author/moderator authority and an actual
qualified commit fence. The owner records receipt/content/outbox atomically.
Replay must reauthorize current visibility. Deletion success is returned only
after the owner commits. Links/media need real current owner authorization;
caller-supplied booleans are not capabilities. Root integration must preserve
C04 same-origin/browser-intent guards and map unavailable dependencies safely.
No notices, operator facts, venue information or media uploads are invented.

## Validation evidence

Commands run from the assigned worktree on 2026-11-04 (Asia/Seoul):

| Command | Exit | Scope |
| --- | --- | --- |
| `rustfmt +1.88.0 --edition 2021` on all new feed Rust files | 0 | Parser/format check |
| `python3 scripts/mcr/check-plan.py` | 0 | Planning/provenance only; all 13 source archives intact |
| `git diff --check` | 0 | Whitespace |

Authored targeted Rust tests cover bounds, tie ordering, cross-event batches,
strict cursors, hidden/deleted/private parents, linked filtering, stale/block/
wrong-binding/expired observations, hide-during-enrichment, unavailable defaults,
AEAD binding/tampering/expiry and closed DTOs. Cargo tests are **NOT_RUN** pending
delivery's shared `/Users/lhs/Desktop/mcr-build/backend` target lease. No duplicate
Rust target is created. No database persistence is introduced in this first slice,
so native SQL, restart and cleanup checks are NOT_RUN/not applicable here. Real
Post/Auth/Friend/media/link owner composition, HTTP, two-account social flows,
browser/device/load/release checks are **NOT_RUN**.

## BE31 command / BE32 content slice

`feed::content` adds parent-protected comment pagination/thread reads and validated
`PreparedCommand` intents for all eight C04 create/edit/delete/comment/reaction
operations. Normal edit/delete is author-only; moderator operations remain the
separate BE39 owner contract. A direct comment-ID mutation resolves the actual
current parent, checks its event/public/published state and checks the comment's
owner/revision. Creating a comment checks `post_revision`; edits/deletions check
`expected_revision`. Reactions cannot supply another actor. Parent revision for
an owner reaction CAS is captured on the server and adds no browser parameter.

Prepared commands are private immutable intents, never grants. Canonical BE15
requests use exact registered paths, original query/body shapes, current actor,
event and original key. Create resources use a stable UUIDv8 derived from
SHA-256 of a version marker, actor, event, original key and operation; changed
input with the same key reaches fingerprint conflict, not a second target.
The exact original authority digest and expiry stay in Context; a digest is a
binding, not an authority capability. The owner independently reauthorizes.

`concert_service::feed::queue` implements BE15 `CommandProducer` with protected
owner work in reserved `0010_feed.sql`. A receipt and durable command intent
commit together as PENDING. No Post mutation happens within the Concert
transaction and no publication invalidation is emitted before actual owner
success. An authenticated owner-commit acknowledgement must bind exact original
scope/target/intent and actual revision. It completes the receipt and emits only
resource UUID/revision invalidation in one transaction; repeated acknowledgement
does not duplicate outbox effects. There is no automatic dispatcher or retry.
Uncertain outcomes stay PENDING until verified reconciliation.

`FencedFeedAuthority::check_in` must hold current policy, owner/CAS, membership,
phase, media and linked-target fences through commit. The only shipped production
implementation is `UnavailableFeedAuthority`. Bounded Auth/Friend observations
cannot implement this contract. There is no production mutation enablement,
trusted fake success or fixture authority. Work payloads are protected internal
content; receipt/outbox contain metadata only. Reviewed expired-job payload
retention/account-deletion cleanup remains an integration obligation; no arbitrary
retention interval or destructive automatic cleanup is introduced.

Post owner has accepted a caller-assigned stable create target and additive
`post.concert.v1.PostConcertFeedService` API. Its requested GetComment endpoint
must resolve current parent authority; comments must use the same C04 descending
order and upper timestamp ceiling. Internal receipts must echo actor/event/
operation/key/target; receipt metadata never substitutes for a current authorized
reread. The owner also reports that Concert actor forwarding is not yet qualified
by the inherited platform. All root RPC composition remains hard unavailable
until qualified forwarding and a real authority protocol are supplied.

Additional checks:

| Command | Exit | Result |
| --- | --- | --- |
| `python3 -B backend/scripts/mcr/feed_smoke.py --sql-only --report /tmp/mcr-feed-sql-20261104-01.json` | 0 | SQL_ONLY_PASS; PostgreSQL 16.14; migrations 0001/0002/0010; empty work/index; actual restart; owned cluster removed |
| Python AST parse of `feed_smoke.py` | 0 | Syntax only |

The SQL-only report hashes migration 0010 as
`df722616b0a673e7ba14e4ea93d7599558a8f1c71b5e7fd7c550f8d4b0c3ada7`.
It does **not** run the Rust native transaction test. The harness imports the
existing marker/inode fixture helper `backend/scripts/mcr/migration_smoke.py`
(base SHA-256 `a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8`)
without changing it. The dedicated Rust fixture guard follows the existing
Concert command test ownership checks at the base, with a distinct feed database
and environment namespace. No application implementation is copied.

Authored additional tests cover every comment route under hidden/deleted parent,
foreign writes, parent/event/revision binding, stale reaction authority, all eight
canonical operation shapes, stable create targets/fingerprint conflicts and
unready media/private links. The native `feed_postgres` test exercises real SQL
Pending/replay/conflict, default unavailable authority, current denial after
queueing, bound owner acknowledgement, outbox deduplication and late-insert
rollback. These Rust tests remain NOT_RUN until the shared Cargo lease.

Recovery refinement: durable work now retains the original authority digest, resolved parent and captured parent revision; a composite receipt foreign key binds all scope fields. Owner scan continuation is explicit (`scanned_through`, `has_more`) so a filtered empty batch cannot falsely end pagination. SQL-only run `/tmp/mcr-feed-sql-20261104-02.json` passed migration/restart/cleanup (exit 0), migration SHA-256 `2f59731ad04ae25ec99276ddb36a705debf0af014b4ba8928eff9a5bd2d11433`. Rust remains NOT_RUN pending lease.
