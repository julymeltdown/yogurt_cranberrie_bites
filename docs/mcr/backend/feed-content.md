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
