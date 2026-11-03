# Post-owned concert feed capability — BE31/BE32 dependency

Status: IN_PROGRESS. This is a bounded Post-owner extension, not BE31/BE32
acceptance or a claim of working production feed HTTP flows.

Source base: `9f72a71a99091fe89411b180af3404be85d963f1` on isolated
`codex/mcr-be-post-owner` at `/Users/lhs/Desktop/mcr-worktrees/be-post-owner`.
Contract source: C04 candidate `contracts/domain/v1/feed.schema.json`,
`common.schema.json`, and `operations.json` at that base. Existing inherited
Post ownership/history remain intact. All extension code is newly authored;
no source rendering or other service's database is copied. `docs/source/` is
unchanged. Dependencies use retained chrono/serde/uuid/SQLx/Tokio/sha2/tonic
workspace resolutions; no manifest or lockfile change.

Delivery explicitly allocated migration
`backend/services/post-service/migrations/0031_concert_feed_owner.sql`, domain,
application and service `pub mod concert_feed` exports, new
`backend/proto/post/v1/concert_feed.proto`, additive Post/Concert build inputs,
new isolated Post gRPC adapter/export/policy, Post main service registration,
and explicit optional Concert caller acceptance in Post config. Other writes
stay within the assigned new concert_feed modules/tests, fixture runner and
this evidence document. No Gateway, Concert feature implementation, existing
Post RPC or generic module is modified.

## Owner semantics

Post is the only durable owner of event post/comment/reaction content. Concert
owns composition, public profiles, opaque cursor signing, latest relationship
policy and producer commands; it must not keep a competing content store.
There is no event/revision backfill for legacy Post records. New canonical rows
are event-public and bind immutable event, author, object/parent and creation
time, independently guarded by PostgreSQL triggers and composite foreign keys.

Commands bind canonical actor/event, original operation ID, stable caller-assigned
post/comment IDs, idempotency key and exact mutation including expected revision.
Create starts at revision 1. Owner edits/deletes use object CAS; comment creation
uses parent CAS, comment edits/deletes use comment CAS. All accepted comment
changes and reaction commands advance the parent revision. A repeated active
reaction does not increase the count; a distinct accepted no-op command still
gets a revision/receipt/outbox. Revisions are positive JavaScript-safe integers.

An actor/event/key hash and global operation ID identify each durable receipt.
Replays require the actual parent/comment to remain visible. Exact owner-delete
replays can return their own metadata-only tombstones; hidden targets fail closed.
The canonical command fingerprint detects payload/target/revision/key/operation
changes. Parent advisory locks serialize owner mutations; receipt, content,
counters, reactions and outbox commit in one PostgreSQL transaction. Original
authority digest and deadline are retained in the receipt; raw keys, content and
credentials are absent from receipt/outbox metadata. Retry never creates a new
resource ID. Receipt lookup never executes a missing command. Receipts are
historical commit metadata, not proof of current visibility or current revision.
The internal protocol echoes the key only from a matched caller command; it
must never expose it as a browser receipt field.

Read paths constrain event and current PUBLISHED state. Comment ID lookup first
resolves the actual immutable parent and rechecks its state and authority;
comments cannot recover a hidden/deleted parent. Feed and comments use strict
DESC `(created_at,id)` keysets with an immutable inclusive upper timestamp.
Internal scan boundaries must be signed/scoped by Concert before browser use.
Denied links/media conservatively hide the whole post. No cached projection or
client-supplied URL constitutes resource authority.

## Authority and activation limit

`post-application::concert_feed::Authority` is an explicit admission/fence seam.
It must verify current actor/session/event eligibility, relationship policy for
all actual parent/comment authors, exact READY POST media (event/uploader), and
current linked target type/ID/event/visibility. Its guard must hold those
revisions stable until the Post transaction commits; an observe-only policy
snapshot, supplied digest, boolean or expiry is insufficient. The owner checks
request bindings and expiry at admission and immediately before commit while
guards remain alive. No test-authority constructor exists in production.

Production `Owner::new` installs `DenyAuthority`. The isolated RPC service is
Concert-only at workload authentication and always returns UNAVAILABLE before
owner database access. Context fields are untrusted bindings. The existing
platform child-actor protocol supports only a Post presenter, so it cannot
validate Concert-forwarded actor authority. There is no feature flag, test
credential, user-supplied digest or wire boolean that activates the RPC. Explicit
Concert credentials are optional in Post config and do not grant access to old
Post methods or new content effects.

Required follow-up: qualify Concert-to-Post actor delegation, actual session and
relationship fences, related-owner/media authority and atomic remote command
admission; then implement admitted protocol mapping and prove real HTTP/RPC
flows. The separate metadata outbox is durable staging only: no production
publisher, notification consumer, moderation/deletion integration, retention
policy or archival gate is claimed in this slice. HIDDEN state is enforced on
reads; moderation entrypoints remain future assigned work.

## Validation record

- `python3 scripts/mcr/check-plan.py`: exit 0, planning/provenance PASS.
- Focused `rustfmt +1.88.0 --edition 2021 --check`: exit 0.
- `protoc --proto_path=backend/proto --descriptor_set_out=<temporary path>
  backend/proto/post/v1/concert_feed.proto`: exit 0; descriptor compiles.
- `python3 -m py_compile backend/scripts/mcr/post_concert_feed_postgres.py`: exit 0.
- `git diff --check`: exit 0.
- Rust build/unit tests: NOT_RUN pending assigned shared Cargo lease.
- `python3 backend/scripts/mcr/post_concert_feed_postgres.py --sql-only`: exit 0,
  all 31 Post migrations applied to an empty isolated database; five owner tables
  present; owned cluster stopped and removed.
- PostgreSQL owner Rust regression: NOT_RUN pending exact built unit executable.
- Real authority, real multiuser HTTP flow, production RPC, load/device and
  BE31/BE32 acceptance: NOT_RUN.

The fixture runner creates an empty loopback-only PostgreSQL cluster under a
unique private temporary directory, checks database prefix and initdb lineage,
runs all Post migrations, executes the actual owner persistence regression with
a cfg(test)-only lock-holding authority, then stops/removes its owned cluster.
It does not build a second Rust target or inspect existing application data.

Initial contract/domain milestone is committed separately from the persistence
and transport candidate, so the Concert peer can consume the typed additive
protocol while the owner SQL checks await the serialized build slot. Final
handoff records exact commits, changed paths and executed evidence below.
