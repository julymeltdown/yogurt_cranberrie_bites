# Post-owned concert feed capability — BE31/BE32 dependency

Status: CONTRACT_READY for the additive protocol and bounded owner-source
capability. Owner persistence/regression validation is executed. This is not
BE31/BE32 acceptance or a working production feed HTTP flow.

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

## Validation record — 2026-11-04, Asia/Seoul

Compiled implementation pin: `fd38c9379ac9025d599e3f9897044f9c37a2528c`.
The subsequent fixture/evidence commit changes no compiled Rust source.
Commands ran from this assigned worktree (`backend/` for Cargo), with:

```text
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend
CARGO_BUILD_JOBS=2
CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0
Rust toolchain 1.88.0; Cargo --offline --locked
```

The shared build lease transferred feed → Post-owner → BE07. No other Rust target
was created. The exact Post library unit executable was copied before releasing
the lease. Native PostgreSQL validation then ran without Cargo or the shared
build lease. Binary SHA-256:
`dfe92e51e924b4499311394ccf3e0a8f850fd9c84e029e06711a5de997cc4bc1`.

| Command / check | Exit | Actual result |
|---|---:|---|
| `python3 scripts/mcr/check-plan.py` | 0 | Planning/provenance PASS; no feature acceptance inferred |
| Focused `rustfmt +1.88.0 --edition 2021 --check` on new Rust modules/tests/build scripts | 0 | PASS |
| `protoc --proto_path=backend/proto --descriptor_set_out=<temporary path> backend/proto/post/v1/concert_feed.proto` | 0 | New additive descriptor compiles |
| `python3 -m py_compile backend/scripts/mcr/post_concert_feed_postgres.py` | 0 | PASS |
| `git diff --check` and exact changed-path/source-boundary inspection | 0 | 21 assigned/explicitly delegated paths; no frontend import, source archive change, manifest or lock change |
| `cargo +1.88.0 test --offline --locked -p post-domain --test concert_feed_domain -p post-application --test concert_feed_authority` | 0 | Domain 3/3; application 2/2 |
| `cargo +1.88.0 test --offline --locked -p post-service --lib --no-run --message-format=json` | 0 | Exact native executable compiled, including ignored owner SQL fixture |
| `cargo +1.88.0 check --offline --locked -p post-service -p concert-service --bins` | 0 | Post registration and both proto build inputs compile |
| Exact copied Post library executable, unfiltered | 101 | 41 passed; one inherited container-dependent schema-damage test failed because `/var/run/docker.sock` is unavailable; owner native fixture ignored by default |
| Same executable with `--skip config::tests::external_migration_mode_rejects_schema_damage_without_owner_privileges` | 0 | 41 passed, 1 ignored, 1 filtered; filtered test remains unqualified |
| `python3 backend/scripts/mcr/post_concert_feed_postgres.py --sql-only` | 0 | All 31 migrations applied to an empty isolated database; five new owner tables present |
| Same runner `--test-binary <exact copied executable> --test-sha256 <digest above>` | 0 | Actual owner PostgreSQL regression: 1 passed, 0 failed, 42 other tests filtered; PostgreSQL **16.14 (Homebrew)** |

The initial fixture compilation found that the retained SQLx features omit the
`migrate!` macro. The fixture now uses the existing runtime `Migrator` API; the
final compile and PostgreSQL runs above include that committed fix. No dependency
features or lock resolutions were changed. The unfiltered inherited Docker test
is an explicit validation limitation; the filtered run does not claim it passed.

### Actual owner SQL regression coverage

The one native test contains concrete assertions for:

- Default production construction denies writes without leaving content/receipts.
- Immutable event/author/time and parent relation; foreign event reads and owner
  edits denied; SQL triggers reject direct event transplantation.
- Two concurrent same-revision owner edits produce exactly one winner; concurrent
  identical create retries yield one durable outcome/outbox. Changed payload or
  operation identity under a retained key conflicts.
- Comment create parent CAS, edit/delete owner CAS and counters, safe comment-ID
  parent lookup, descending comment pages with upper bound; removed comments are
  unavailable. Reopening an owner over the same pool observes persisted state.
- Either-direction blocks deny replies, reactions, reads and receipt replays.
  Hidden/deleted parents cannot be recovered through comment IDs/lists/edits;
  hidden parent replay and receipt lookup are denied. Exact owner-delete retries
  return their own metadata-only tombstone.
- Reaction set/no-op/unset and duplicate retry do not double count. An authority
  fence failure after SQL changes rolls back reaction rows, parent revision,
  receipt and outbox together.
- Expired deadline, foreign actor/context, altered original authority/request
  binding and mismatched guard binding deny. A lock-holding test authority keeps
  its policy revision stable through actual commit; it is `cfg(test)` only.
- Media needs the exact event/uploader/READY POST qualification; missing, wrong
  uploader/event and non-READY media deny. Links need current matching event and
  public qualification. Read-time revocation conservatively hides the post.
- Actual PostgreSQL feed ordering includes equal-timestamp UUID ties, strict
  keyset continuation and link filters. Receipt lookup never executes a missing
  command. Receipt/outbox coverage is one-to-one and contains no raw key or text.

The fixture runner creates an empty loopback-only PostgreSQL cluster under a
unique private temporary directory, verifies database prefix and initdb lineage,
runs all Post migrations, and invokes the hash-pinned actual owner executable.
It stops and removes its owned cluster even on failure. It never queries existing
application databases. The fixture uses test authority, not real session,
relationship, related-object or media-service authority.

### Unrun acceptance and remaining integration

Real Concert-to-Post actor authority, remote mutation fencing, admitted RPC
mapping, actual media/link authorities, outbox publishing/consumers, moderation
and user-deletion integration, archive/retention and real multiuser HTTP/browser
acceptance are NOT_RUN. Production RPC stays hard UNAVAILABLE. These are explicit
future integration/qualification dependencies, not approval gates for the
bounded owner-source handoff. BE31/BE32 and IN03 are not marked ACCEPTED.

## Committed handoff and ancestry

Series from source base `9f72a71a99091fe89411b180af3404be85d963f1`:

1. `e975500` — contract, domain/application authority seam and regressions; portable
   Post/Concert proto inputs (shared prerequisite already supplied to feed peer).
2. `bd92708` — PostgreSQL owner, migration 0031, isolated gRPC denial and fixture.
3. `49147af` — replay state reauthorization, local key hashing, pinned fixture and
   native regression additions.
4. `fd38c93` — retained runtime migration loader correction; compiled source pin.
5. The containing final fixture/evidence commit records executed checks above.

Delivery integrates this series serially, resolving only its allocated root
registration overlaps. The feed worker's cherry-picked prerequisite `e975500`
must not be duplicated. Only the delivery integrator publishes master. Remote
master integration/ancestry is pending that integrator; this worker did not push.

Exact changed paths against the source base:

```text
backend/crates/post-application/src/concert_feed/mod.rs
backend/crates/post-application/src/lib.rs
backend/crates/post-application/tests/concert_feed_authority.rs
backend/crates/post-domain/src/concert_feed/mod.rs
backend/crates/post-domain/src/lib.rs
backend/crates/post-domain/tests/concert_feed_domain.rs
backend/proto/post/v1/concert_feed.proto
backend/scripts/mcr/post_concert_feed_postgres.py
backend/services/concert-service/build.rs
backend/services/post-service/build.rs
backend/services/post-service/migrations/0031_concert_feed_owner.sql
backend/services/post-service/src/adapters/in/grpc/concert_feed.rs
backend/services/post-service/src/adapters/in/grpc/mod.rs
backend/services/post-service/src/concert_feed/mod.rs
backend/services/post-service/src/concert_feed/persistence.rs
backend/services/post-service/src/concert_feed/query.rs
backend/services/post-service/src/concert_feed/tests.rs
backend/services/post-service/src/config/mod.rs
backend/services/post-service/src/lib.rs
backend/services/post-service/src/main.rs
docs/mcr/backend/post-concert-feed.md
```
