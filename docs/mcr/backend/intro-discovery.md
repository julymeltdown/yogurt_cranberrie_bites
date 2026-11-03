# BE27 introduction authoring and discovery

## Delivery boundary

Task **BE27** implements owner draft/set/publish/close, authenticated
introduction discovery and direct lookup, canonical-profile projection, and
stable pagination. The implementation base is
`00ecd45999dacf29addf8ede9532060112eb9a03` on
`codex/mcr-be-BE27-intros`.

This is a bounded source and native-persistence delivery. It does not accept
BE27, IN03 or release. Production Auth, Profile, Friend, BE26 claim, invalidation
consumer and HTTP composition remain explicit integration gates. All native
authority and profile rows are synthetic fixtures.

Owned paths:

- `backend/crates/concert-application/src/intro/{mod,cursor,wire}.rs`
- `backend/crates/concert-application/tests/intro.rs`
- `backend/services/concert-service/src/adapters/out/repository/intro.rs`
- `backend/services/concert-service/src/adapters/out/repository/intro_schema_v1.sql`
- `backend/services/concert-service/tests/intro.rs`
- `backend/services/concert-service/tests/intro_support/{owned,native_smoke.py}`
- This document

The production module exports are integration changes for Main:

```rust
// backend/crates/concert-application/src/lib.rs
pub mod intro;

// backend/services/concert-service/src/adapters/out/repository.rs
pub mod intro;
```

The temporary `#[path]` exports used to qualify this isolated checkout are not
part of the BE27 feature commit. Root routers, generated contracts, workflows,
service configuration and the central migration ledger remain owned by the
integrator.

## Source and contract provenance

BE25 remains the authoritative age and introduction policy. This work reuses
its validated contact and age semantics through explicit adapter ports. The
read-only upstream source is `julymeltdown/bigyeting` at
`5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`.

| Upstream path below `backend/services/bigyeting-api/` | SHA-256 | Use |
| --- | --- | --- |
| `src/application/use_cases/social_intro_policy.rs` | `9a28d746c7b646a6039c2a645a76caa163775d6a811f6926d73c0fc1cfb47770` | Publication, draft and contact-validation semantics |
| `src/application/use_cases/social_intro_contact_policy.rs` | `e9cdc345e1da02c6eca667cfbc6d56d9977267332c654cdb754852bfa692b766` | Existing BE25 contact detector, reused without duplication |
| `src/adapters/out/repository/current_age.rs` | `74df0f4f60a9ce262aeec2af6a33deb897279889634cb14ed6cfe74579e11a50` | Existing BE25 canonical claim and age semantics |

No Bigyeting rendering, provider token, request/reveal flow, identity store or
SQL schema was imported. The newly authored persistence, projection and cursor
surfaces use only existing retained dependencies. Their source pin is the base
above; no manifest or lockfile resolution changed.

| Current C04 input | SHA-256 |
| --- | --- |
| `contracts/domain/v1/common.schema.json` | `303f3019611c636679282d6c29106deab83d7c0c9717bd8b9ba526d2ff865a35` |
| `contracts/domain/v1/social.schema.json` | `b275b36f2ad8eee90922f7ad27cfcbece943773a94831720b6dfca588d0201d2` |
| `contracts/domain/v1/policy.json` | `01c6b9e9a44b81bcf1273ebd7ba8361d3cc035a969399b4dad1f9dedec05839a` |
| `contracts/domain/v1/operations.json` | `411fcee8222db2f5a7938866ac37791033e85682f11d024ceb895c245c708500` |

## Commands and transaction behavior

`IntroCommand` produces the canonical actor/event/idempotency-key/operation
binding. The introduction target is derived from event and actor, so repeated
clients cannot select a foreign introduction ID. Transport assertions are not
authority.

The repository consumes BE15's transactional receipt producer:

1. Authorize the complete command scope inside the database transaction.
2. Lock global receipt budget and look up an existing receipt by exact scope.
3. Reauthorize any retained receipt and compare the signed canonical request.
4. For a new key, lock the event and current owner row, validate event phase,
   expected revision, draft, expiry and the configured event cutoff.
5. Write the introduction and atomic invalidation outbox row with the successful
   receipt. A policy or finalization error rolls all three back.

`SET` accepts absent revision zero only when the owner row is absent. It creates
or updates the actor's single event-scoped introduction and preserves its stable
ID and creation time. `PUBLISH` accepts only a current nonempty `DRAFT`;
`CLOSE` accepts any current non-closed revision. All successful commands advance
exactly one revision. Response loss is recovered by retrying the exact key and
canonical body; a changed body with the same key is a conflict.

Optional expiry must be later than the transaction database clock and no later
than `concert_meeting_settings.event_cutoff`. The setting row is locked through
commit. Absence or a past cutoff denies set/publish; owner close remains
available. Database time is read after the policy/event lock to avoid a
cross-clock comparison with a canonical microsecond timestamp.

The formal application export `pub mod intro;` and production repository export
`pub mod intro;` remain Main integration insertions. The final isolate build
used temporary path exports solely to compile the owned test binaries; those
exports are removed before this feature commit.

## Read authorization and projection

`discover` and `direct` open one transaction, take a database clock, validate
the request context, and call the same `IntroProjectionExecutor` collection
authority even for an empty page. The transaction then:

- selects only the requested event's current `PUBLISHED` rows;
- excludes expired rows and accepts only rows at or before the page scan fence;
- calls `project_public` for current relationship/eligibility/claim checks;
- resolves the exact C03 `PublicProfile` through a separate port;
- structurally validates its allowlist, matching canonical owner and reviewed
  avatar config;
- applies the optional case-insensitive text/display-name filter;
- acknowledges the complete page before commit.

`IntroProjectionExecutor` is deliberately the BE27 lifecycle port:

```rust
check_collection(connection, context, now)
check_owner(connection, context, now)
project_public(connection, context, row, now)
acknowledge(connection, context, now)
```

Any implementation must resolve current authority and keep collection, intro,
claim, profile, relationship and owner locks through final acknowledgement and
`COMMIT`. A timestamp, detached BE25 `ClaimLease`, profile response or receipt
is not a substitute. A projection deny/not-found removes one row; dependency
failure aborts the page. Direct lookup maps a denied row to not-found.

`IntroPolicy::check` must lock current event/owner authority and pin
`mcr.intro_authority_until` before mutation. `IntroProfileProjection::author`
must return exact current C03 public profile bytes and hold its source through
projection. `IntroPolicy::edit_consent` is the BE28/BE29 seam for closing
pending requests and revoking grants after edits.

## Cursor and page stability

The cursor is a request-bound authenticated ASCII framing, not JSON:

```text
HMAC-SHA256 | upper-micros | padded-position-micros:UUID | actor | event |
listConcertIntros | intros | query-hash | deadline-micros
```

The actual octet string has no spaces or line breaks. Its encoded body is
`upper|position|actor|event|operation|collection|query|deadline`; the HMAC prefix
makes nine pipe-delimited fields. Microseconds are exactly 20 decimal digits.
The key is injected by `CursorCodec`; this task provides the authenticated codec
and an unavailable production default.

A page fixes its scan upper bound at the first database clock. Ordering is
`created_at DESC, intro_id DESC`; continuation uses the strict tuple
`(created_at,intro_id) < position`. Rows inserted after the fence do not disturb
the page sequence. The initial page and every scanned continuation stay under
the same actor/event/query binding. The token expires at 15 minutes. Peer profile
or relationship changes may still filter later rows, so the cursor is a stable
scan position, not a claim that all later rows remain visible.

Filtered pages paginate on scanned position, not the last returned match, so
sparse results cannot loop. A 1,000-row scan cap fails closed without a
continuation rather than returning an incomplete sequence as complete.

An early native failure was not HTTP or projection: the authenticated position
timestamp had ordinary 16-digit formatting while the decoder required 20 digits.
The encoder now uses `{:020}`; the pure codec fixture round-trips a non-null
position and rejects tampering, wrong binding, expiry and malformed width.

## DTO and persistence constraints

Public DTOs contain only `intro_id`, `event_id`, exact C03 `author`, text,
creation time, `PUBLISHED` and revision, plus the cursor at page level. Owner
DTOs contain private age bounds and expiry only on the owner path. Neither path
exposes claims, exact age, DOB, verifier data, provider tokens or internal
position/filter bytes.

`intro_schema_v1.sql` is an owned prerequisite schema, not a central migration
number. It requires event/owner scope uniqueness, revision bounds, state/content
consistency, paired inclusive/exclusive age bounds, nondecreasing timestamps,
immutable event/intro/owner/creation scope, one-step revisions, terminal close
and no published-to-draft transition. The discovery index is event/time/ID over
published rows.

Registration in the migration ledger, grants, retention/deletion, backup and
live-data migration remain integrator/operations work. The SQL fixture applies
the real C03/C04/C06 prerequisite schema through `0008_meeting_beacons.sql`.
Its authority, profile and deadline tables are created only by the native test.

## Composition status

The production integration work is not hidden behind a permissive fallback:

| Seam | Current state |
| --- | --- |
| Canonical Auth session/request authority | `NOT_INTEGRATED`; synthetic authority only |
| Current event membership/feature authority | `NOT_INTEGRATED`; explicit policy port |
| Current Profile C03/avatar owner read | `NOT_INTEGRATED`; synthetic exact-profile bytes |
| BE37 relationship/block policy | `NOT_INTEGRATED`; projection test hook only |
| BE26 current claim projection | `NOT_INTEGRATED`; no fake synchronous lease |
| BE28/BE29 request close/reveal revocation | `NOT_INTEGRATED`; edit-consent seam only |
| Realtime invalidation consumer | `NOT_INTEGRATED`; outbox row emitted |
| Public HTTP/OpenAPI/routes | `NOT_INTEGRATED`; contracts frozen upstream |
| Central migration, grants and runtime | `NOT_INTEGRATED` |

BE26's proposed generic `project<T,F>` can host the complete BE27 direct/page
projection once the joint collection and resource authority protocol is frozen.
That composition must include Profile and relationship locks; calling BE25's
old synchronous `ClaimReader` with a timestamp-only lease would be unsafe.

## Qualification evidence

The runner uses Rust and Cargo `1.88.0`, locked/offline dependencies, jobs 1,
incremental 0, dev/test debug info 0, an 8 GiB free-space floor, and
the released shared `media-combined-fa4b4b3` target. No duplicate target or
dependency installation is used. Every native run starts from a new marker- and
inode-validated private Unix-socket PostgreSQL cluster, checks server 16.14 and
the private data directory, uses no TCP, stops and restarts the same cluster for
persistence, checks secret absence, and removes only its owned root.

Application command, closure, DTO and cursor tests: **PASS 4/4** on candidate
sources before this document's final qualification. Native receipt 79 preserves
an intermediate state: **2 passed, 2 failed**, with pagination passing. Native
receipt 80 preserved the same command failures in the later no-diagnostic build.
The traced application reached `apply`, sampled `db_now`, then filled an absent
row's `created_at` from `Utc::now()`; the observed values were 331 and 256
microseconds beyond `db_now`. Owner validation correctly rejected those future
creation times. The correction uses the already sampled transaction database
clock for an absent row's creation time and publish-expiry comparison. It
preserves creation time on updates and all receipt, revision and cutoff guards.
Native receipt 82 recorded **3 passed, 1 failed**, fixing the receipt case; that
run exposed a test-only event-state collision. Later fixtures added a separate,
main-event-authorized cutoff actor. Its accepted `SET(0)` establishes revision 1;
the beyond-cutoff set returns `Invalid` and the post-cutoff set returns `Denied`
without advancing it, so owner close at revision 1 produces `CLOSED` revision 2.
Earlier failing receipts remain diagnostic evidence and are not relabeled as
passes.

Final no-trace candidate 87 compiled from test SHA-256
`3ab8e527da14bb0ef7aec383c0b722e54e1cc993e41a254b5efe7ed60052efe2` and
production repository SHA-256
`02c039f2bc3a18f52580e887f59e1d67da05da01f2363f4d2345741d6bec454b`.
The exact executable is
`/private/tmp/mcr-be27-intro-00ecd45-20261104/intro-tests-final-87`, SHA-256
`aa6dfb77b11b3b6ed375e0ebabe7153b7fa55fb239b731f7678befb60a0aa58d`.
Its owned PostgreSQL 16.14 receipt is
`/private/tmp/mcr-be27-native-87.json`, SHA-256
`3b5b3d8b419837d0d0c9c6c20a6b050f84704ec03f5b053b6d609b25aa6e195d`.
All **4** test cases passed with exit 0; the combined output SHA-256 is
`a914f347e673573788dad983d20a6062448699eb5ca85bda80e7f24813f6e9f1`.
The server stopped and restarted to a different PID, the persisted row before
and after restart was `PUBLISHED|1`, the no-TCP/secret checks passed, and the
owned cluster cleanup passed. Candidate 79 remains preserved at
`/private/tmp/mcr-be27-intro-00ecd45-20261104/intro-tests-candidate-79`
with SHA-256
`1646549d2f23e0fb521b54bf876658b38bd92508f10a8848c3d50f7ed60634d4`.
Receipt
`/private/tmp/mcr-be27-native-79.json` has SHA-256
`6bbf2f5095d3ef1761a2ae2716f25043d69867e998a75f62a31f014e95cdd09e`.
Intermediate receipts 80, 83, 84, 85 and 86 are also retained as failed
diagnostic evidence rather than silently overwritten or relabeled.

This report does not convert its synthetic native rows into real multiuser,
provider, HTTP, load, browser, device or IN03 evidence.
