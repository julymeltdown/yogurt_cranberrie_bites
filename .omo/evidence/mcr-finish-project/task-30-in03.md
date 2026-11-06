# Task 30 — IN03 social and privacy gate

- Plan row: 30 (`Pass IN03 social and privacy gate`)
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w5-in03`
- Branch: `codex/mcr-exec-w5-in03`
- Base: `c35076243aa651db77c7bf60ca0df4e68ab8aa39`
- Merge: `3653cd462` merges master `3aa766178`, which carries the blocked-author
  feed filter `d020a46d1` (`codex/mcr-exec-w3-blockfeed`).
- Gate driver: `backend/scripts/mcr/in03_social_gate.py`
- Machine report: `task-30-in03-report.json`, generated at `3653cd462` from the
  working tree this commit delivers. Only the driver, the tests and the report
  differ from that revision; the served source is identical.
- Delivered commits: `f0d1f21ae` (retain before the effect witness), `4ec06d5ad`
  (gateway outer timeout to the two-second ceiling), `3653cd462` (master merge),
  and this evidence commit.
- Result: **PASS** on every executed check, `result_scope = PASS_WITH_RECORDED_NOT_RUN`,
  `cleanup = PASS`, exit code 0. 27 checks were evaluated and all 27 are bound to a
  recorded step.

Banner: **development stack, NOT_RELEASE_QUALIFICATION, QA08 NOT_RUN,
NO_CLIENT_ONLY_PRIVACY_CLAIM.** The run proves server-side friendship, block,
profile, concert-feed and wall privacy fences on this checkout's native services.
It does not claim provider verification, venue producer completion, client-cache
behavior, a world-socket frame, a physical device, capacity or a release decision.

## Defects in the previous report fixed here

### 1. The ledger order test was not load-bearing

The previous test took its own `FOR SHARE` on the scope row and then called
`ledger.register_child_intent`; it never executed the production hook, so reverting
`producer.rs` to witness-then-retain left it green. Re-measured on the previous
tree: with the order reversed, `product_scope_ledger` still reported
`6 passed; 0 failed`.

The write order now lives in `RuntimeHooks::succeeded` behind two private
collaborators (`SourceRetention`, `TransactionRegistration`), extracted without
changing behavior, so the hook itself can be executed with recording steps. The
native lib test
`social_live::producer::read_order_tests::projecting_writes_retain_the_child_before_the_effect_witness`
drives the real `succeeded` and asserts the emitted sequence. The reversal
demonstration, run against a full 61-migration owned cluster:

```
test social_live::producer::read_order_tests::projecting_writes_retain_the_child_before_the_effect_witness ... FAILED
assertion `left == right` failed: a projecting transaction must register its child intent
before the witness takes the scope share lock
  left: ["witness", "retain"]
 right: ["retain", "witness"]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 196 filtered out
```

In the same mutated tree the ledger integration suite still reported
`6 passed; 0 failed` — the weakness this change removes. Restored to the fixed
order, both are green again (`6 passed` and `1 passed`).

The test lives in the crate because `RuntimeHooks` is crate-private and
`tests/product_scope_ledger.rs` is a separate crate; this repository forbids
manufacturing an `OwnerGuardScope` (no fixture constructor exists and the social
support runner records that limit), so no integration test can construct the hook.
The integration test now pins the lock constraint itself, and its name says so:
`child_intent_registration_blocks_behind_a_held_scope_share_lock` asserts that
registration waits at least 150 ms on the ledger's 250 ms lock budget and then
succeeds once the share lock is released.

### 2. `comments_read` contradicted its own measurement

The delivered entry had `status = EXECUTED` next to a reason saying the route
"did not return data" and exceeded the outer timeout, while its recorded step was
`200` in 1591 ms with Bob as the author. The entry is now measurement-driven: the
reason quotes the recorded status, client-measured wall time and author presence,
and `provenance.comments_ceiling` records the margin against the 2000 ms gateway
budget.

### 3. The post-unblock feed read was claimed but never executed

The previous BLOCKED reason claimed carol's feed returned `200` with Bob's post
"after the unblock", but the driver never issued that read. The driver now records
`feed_carol_after_social_unblock` (200 in 623.2 ms, Bob's post present) and binds
check `feed_carol_after_social_unblock` to it.

### 4. The blocked-read case is now a real pass

The previously recorded BLOCKED observation (carol's feed returned 503 after she
blocked Bob through `/me/blocks`) is fixed on master. `public_profile::pair_visibility`
now classifies a blocked pair as `NotFound`, so `feed_error` maps it to the
skippable `ResourceUnavailable` while a real owner outage still fails closed.
This run records `feed_carol_blocked_author_state` as **200** in 289.9 ms with
`items = []`, Bob's post absent, and no Bob user id or email in the body, and binds
check `feed_carol_blocked_author_filtered` to it. The 503 blocker entry is gone.

### 5. Delivered binaries had no recorded hash; the committed runner cannot pass

See [Provenance](#provenance) and [Known runner defect](#known-runner-defect-not-fixed).

## Root causes fixed in this checkout

### 1. `/me/blocks` 503 was a lock-order defect, not a budget problem

Observed (before): `GET …/me/blocks` returned `CONCERT_503 DEPENDENCY_UNAVAILABLE`
in 356–514 ms with nine confirmed `concert_product_owner_transactions` rows. The
product RPC operation is classified (`product_rpc::operation` → `Operation::Social`,
`social_live::protocol::route` → `SocialOperation::Blocks`), the gateway maps the
route (`social_route`, `is_product`, `owns_receipt`), and the 2 s product budget was
not reached.

Instrumented evidence from the real stack:

```
SOCIALDBG track_inner complete
SOCIALDBG scope_ledger.sqlx: Database(PgDatabaseError { code: "55P03",
  message: "canceling statement due to lock timeout",
  where: Some("while locking tuple (7,5) in relation \"concert_product_owner_scopes\"") })
SOCIALDBG retain.register_child_intent: Unavailable Product scope lifecycle unavailable
SOCIALDBG list.retain: Unavailable Product scope lifecycle unavailable
```

The Social read projected its result, inserted its effect witness, and only then
tried to retain the source-exclusion child. The witness insert's trigger
(`concert_social_effect_witness_guard`) holds `FOR SHARE` on the root scope row for
the rest of the projecting transaction, and child-intent registration takes
`FOR UPDATE` on that same row (`PgScopeLedger::lock` and the
`concert_product_child_intent_transition` trigger). The second lock waits on the
first, hits the ledger's 250 ms `lock_timeout`, and the whole read fails 503.

Fix: perform the retention **before** the witness in every Social projection path —
`RuntimeHooks::succeeded` (covers `listOwnConcertHandles`/`reveal`) and the explicit
`retain`/`succeeded` pairs in `detail`, `handle_state`, `block_state` and `list`.
The ledger lock strength and its lifecycle serialization are unchanged; both writes
still commit inside the same projecting transaction, so nothing is exposed earlier.

### 2. Comments failed at ~1.0 s because the gateway's outer timeout was 1 s

The composed limits are 2 s (`product_rpc::BUDGET`, gateway concert clients), but the
gateway's outer `GATEWAY_REQUEST_TIMEOUT_MS` defaulted to 1000 ms and its deadline is
what bounds every product call (`call_deadline::scope`). Measurements on the real
stack:

| Request, same stack | 1000 ms default | 2000 ms default |
| --- | --- | --- |
| `GET …/posts/{post}/comments?limit=10` (one comment) | 504 in 1002–1006 ms | 200 in 1268–1744 ms |
| `GET …/feed?limit=5` | 503 in 1002 ms | 200 in 1146 ms |
| `GET …/feed?limit=1` | 200 in 290–892 ms | 200 in 254–569 ms |

Every failing request failed at the same 1001–1003 ms wall, including a
zero-comment thread and a five-post feed, so the cap was the gateway, not the
comments path. The comments path does add bounded per-comment work — one guarded
public-author read per comment (`feed/author_filter.rs` → `GuardedResources::author`,
plus the post owner's per-comment admission) — but with a single comment the fixed
cost already exceeds 1 s, so the route budget was the blocking cause.

Change: `GATEWAY_REQUEST_TIMEOUT_MS` default 1000 → 2000 ms (runbook updated). This
does **not** loosen the security budget: the product clamps its signed authority to
`min(auth_observed, profile_observed) + 2 s` and rejects any observation bound wider
than `product_rpc::BUDGET`, so the outer timeout now matches the documented
two-second ceiling instead of cutting it in half. `GATEWAY_JOURNAL_CLIENT_TIMEOUT_MS`
(900 ms) remains strictly lower.

## The write-order test, and how to run it

```sh
# 1. full-migration owned PG16 cluster (61 migrations; max_prepared_transactions=256)
python3 -B /private/tmp/in03_ledger_full_run.py   # this turn's one-off runner; not committed
# ledger integration suite:       6 passed; 0 failed
# read-order projection test:     1 passed; 0 failed
# read-order command test:        1 passed; 0 failed
# migrations: 61; prepared transactions left: 0; cleanup: PASS

# 2. directly, against any owned database:
CONCERT_TEST_DATABASE_URL=<owned url> \
  cargo test -p concert-service --lib -- --ignored --exact --test-threads=1 \
  social_live::producer::read_order_tests::projecting_writes_retain_the_child_before_the_effect_witness
```

The lib tests are `#[ignore]` because they need a real `PgConnection` for the hook
signature; they never read a table (the steps are recording doubles).

## Stack and inputs

- Seven native binaries built from this checkout: `api-gateway`, `auth-service`,
  `friend-service`, `profile-service`, `post-service`, `notification-service`,
  `concert-service`.
- Build: `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w5-in03 cargo build
  -p concert-service -p api-gateway -j 3` → exit 0 (0.63 s incremental relink).
- Owned disposable PG16 cluster with `max_prepared_transactions=256`, owned Redis,
  private local HTTP mail sink, generated mTLS PKI and a private temp root
  (removed by cleanup).
- Four independent principals (`alice`, `bob`, `carol`, `outsider`) completed real
  enrollment → mail verify → login → consent → profile initialization. Alice, Bob
  and Carol were admitted to the event; `outsider` was authenticated but
  deliberately not admitted.
- Social composition enabled: `CONCERT_SOCIAL_MODE=live`, notification inbox/target
  live, shared Intro/Participant live, World listener, and concert public feed reads
  (`CONCERT_PUBLIC_FEED_MODE=read`, `POST_CONCERT_READ_MODE=read`).
- Enrollment policy: revision `6e8d17f2-21f0-4239-b29e-87ccd56f1bfb`,
  `approvalReference = development-not-admitted`. The checkout's policy schema has
  no `productionApproval` field; no legal approval is claimed.

## Commands

```sh
cd /Users/lhs/Desktop/mcr-worktrees/exec-w5-in03/backend
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w5-in03 \
  cargo build -p concert-service -p api-gateway -j 3
# exit 0

cd /Users/lhs/Desktop/mcr-worktrees/exec-w5-in03
python3 -m py_compile backend/scripts/mcr/in03_social_gate.py
# exit 0

python3 -B backend/scripts/mcr/in03_social_gate.py \
  --bin-dir /Users/lhs/Desktop/mcr-build/exec-w5-in03-bins \
  --head-build /Users/lhs/Desktop/mcr-build/exec-w5-in03/debug \
  --ceiling-observation /private/tmp/in03-prefix-report.json \
  --report .omo/evidence/mcr-finish-project/task-30-in03-report.json
# exit 0; result PASS; result_scope PASS_WITH_RECORDED_NOT_RUN; cleanup PASS; 27/27 checks

python3 scripts/mcr/contracts/route-parity.py
# route parity ok: every registered command has an exact axum template (exit 0)

# ledger suite + read-order tests: full 61-migration owned cluster (see above)
# 6 passed; 0 failed + 1 passed + 1 passed; 0 failed

rustfmt --check --edition 2021 \
  services/concert-service/src/social_live/producer.rs \
  services/concert-service/tests/product_scope_ledger.rs \
  services/concert-service/tests/social_live_support/read_order.rs
# exit 0
```

## Case matrix (executed this run)

| Case | Actors | Request | Result |
| --- | --- | --- | --- |
| Follow (public target) | alice → bob | `POST /api/follows` | 200 `{"following":true,…}`; durable `follows` row |
| Private profile | bob | `PUT /api/profile/privacy` | 200; DB `profiles.account_visibility=PRIVATE` |
| Remove friend | alice | `DELETE /api/follows/bob` | 200; live `follows` rows = 0 |
| Follow request (private target) | alice → bob | `POST /api/follows` | 200 PENDING; DB `PENDING` |
| Incoming request | bob | `GET /api/follows/requests/incoming` | 200; request id present |
| Decline request | bob | `POST …/{id}/reject` | 200 `REJECTED` |
| Accept request | bob | `POST …/{id}/accept` | 200 ACTIVE; DB `ACCEPTED` |
| Private profile fence | carol → bob | `GET /api/profile/bob` | 404 exact `GATEWAY_404` |
| Private profile visible to friend | alice → bob | `GET /api/profile/bob` | 200 `PRIVATE` |
| Private view invalidated on unfollow | alice | unfollow, then `GET /api/profile/bob` | 404 exact `GATEWAY_404` |
| Block (friend surface) | alice → bob | `POST /api/relationships/blocks/bob` | 200; `user_blocks` row; invalidation journal 0→1 |
| Block observed by target | bob → alice | `GET /api/relationships/alice` | 200 `blockedViewer:true` |
| Blocked follow denied | alice → bob | `POST /api/follows` | 403 exact `GATEWAY_403` |
| Blocked profile denied (both directions) | alice ↔ bob | `GET /api/profile/{other}` | 404 exact `GATEWAY_404` in both directions |
| Concert feed baseline | carol | `GET …/feed?limit=1` | 200; Bob's post present; DELETED seed absent |
| Deleted post read | carol | `GET …/posts/{deleted}` | 404 exact `CONCERT_404 RESOURCE_UNAVAILABLE` |
| Blocked author disappears from feed (Friend block) | alice after friend block | `GET …/feed?limit=1` | 200; Bob's post absent |
| Blocked post read | alice | `GET …/posts/{bob_post}` | 404 exact `CONCERT_404` |
| Comments self-read | bob | `GET …/posts/{bob_post}/comments?limit=10` | 200 in **1373.9 ms** (client wall); seeded comment author present; DB `PUBLISHED` |
| Deleted post absent | carol | `GET …/posts/{alice_post}` | 404 exact `CONCERT_404` |
| Wall member read | alice | `GET …/mural/snapshot` | 200 revision 1 |
| Wall non-participant read | outsider | `GET …/mural/snapshot` | 404 exact `CONCERT_404` |
| Social block list (empty) | bob | `GET …/me/blocks` | 200 `{"items":[],"next_cursor":null}` |
| Social block | carol → bob | `PUT …/me/blocks/{bob}` | 200 `{"blocked_by_me":true,"revision":2,…}` |
| Block listing | carol | `GET …/me/blocks` | 200; target bob present |
| Feed after Concert social block | carol | `GET …/feed?limit=1` | **200 in 289.9 ms; `items=[]`, Bob's post absent, no Bob id/email in body** |
| Social unblock | carol → bob | `DELETE …/me/blocks/{bob}` | 200 `{"blocked_by_me":false,"revision":3,…}` |
| Feed after Concert unblock | carol | `GET …/feed?limit=1` | **200 in 623.2 ms; Bob's post present** |
| Unverified age social request denied | alice → bob | `POST …/social-requests` | 404 exact `CONCERT_404 RESOURCE_UNAVAILABLE` (SocialRuntime envelope) |
| Unblock (friend surface) | alice → bob | `DELETE /api/relationships/blocks/bob` | 200; `user_blocks.deleted_at` set |
| Profile after unblock | alice → bob | `GET /api/profile/bob` | 200 (public profile restored) |
| Denied payload audit | all denials | exact key set `{code,message,traceId,timestamp,details}` | 10 envelopes, 0 private metadata leaks across user ids, emails, handles, tokens and passwords |

Every successful follow/accept/reject/unfollow/block/unblock was also read back
from the owned PostgreSQL databases (`mcr_friend`, `mcr_profile`, `mcr_post`,
`mcr_concert`).

## Concert block feed (now filtered, not failed)

`feed_carol_blocked_author_state` — after carol blocks Bob through `/me/blocks`,
her own `GET …/feed?limit=1` returns **200** in 289.9 ms with `items=[]`. Bob's post
is absent, and neither his user id nor his email appears in the body. The identical
read returned 200 with Bob's post before the block (`feed_baseline`) and 200 with it
again after the unblock (`feed_carol_after_social_unblock`). Old behaviour was 503
in 294.5 ms (previous report). Consumer:
`backend/services/concert-service/src/adapters/out/public_profile::pair_visibility`
→ `feed_error` → skippable `ResourceUnavailable`; a genuine owner outage still maps
to `DependencyUnavailable`.

## NOT_RUN

| Case | Missing input or blocking consumer | Observed |
| --- | --- | --- |
| `venue_meeting_reads` | No composed `Operation::Meeting` ProductOwner; `venue_meeting_admission.rs` states the bilateral friend read does not exist, so the route fails closed at 503. Consumer: `backend/services/concert-service/src/product_rpc/venue_meeting_admission.rs`. | Not executed; no venue privacy claim |
| `venue_location_snapshot` | `PgVenueLocationAuthority::friend_in` returns `PolicyUnavailable` and no `VenueLocation` product producer is composed. Consumer: `backend/services/concert-service/src/venue_location/mod.rs`. | Not executed |
| `blocked_comment_author_filter` | Needs a parent author plus a comment by a blocked author in one read; this run seeds one comment by the parent author. Consumer: `backend/services/concert-service/src/feed/author_filter.rs`. | Not executed; no privacy pass claimed |
| `age_provider_positive` | Missing input: authorized age-verification provider with signed verified claims (`concert_age_owners`, `concert_age_claims`, sharing verifier). | NOT_RUN |
| `social_reveal_revoke` | Depends on that same provider/claim input; a real grant cannot be created without verified claims. | NOT_RUN |
| `external_oauth_provider` | Missing input: external OAuth/social provider credentials, callback registration and provider test account. | NOT_RUN |
| `external_mail_or_sms` | Missing input: external mail/SMS provider. Only the owned local HTTP mail sink was exercised. | NOT_RUN |
| `client_cache_and_ws_frames` | No browser client ran in this backend gate; no client-cache or world-socket frame claim is made. | NOT_RUN |
| `comments_read` | Executed, not claimed as a release pass: server-side read evidence only (see below). | 200 in 1373.9 ms |

## Provenance

### Binaries

Executed binaries (this run, `provenance.executed_binaries`); the delivery build in
`/Users/lhs/Desktop/mcr-build/exec-w5-in03/debug` is byte-identical for every entry
(`provenance.differing_binaries = []`):

| Binary | sha256 |
| --- | --- |
| concert-service | `36095a37ed55f0fb97de5ffbcfd7fdc6ee05808baa86f3f853e36ff827075c42` |
| api-gateway | `247cf28889054ba2f47ddfe12ff271f4c971fdf4ec8b8a3e990dfc957059d550` |
| auth-service | `3cd68a61c9cc4eaf51490b3266edf563089e5e1f30d2a0a9e2b1d3c1d06a44ed` |
| friend-service | `00d2c985bf6c8a76800e3d0bcdfcb5941df7f7550264dfba052acf7c16c52e5f` |
| notification-service | `821f48fb1c058664ed273e6b8c439b1d332f50635afdf1f203ca9710fe46fc21` |
| post-service | `9dff7470ef2919562db053db5c721dd5cc668268bb5ffd3f28a16c29613fe0b3` |
| profile-service | `88026521badd0a1e92d61490305b25f8dac0ae46e8e6155c1600893eb4eb5ad3` |

The previous delivered report recorded no binary hash. Measured this turn before
the rebuilds:

- The delivered copy `exec-w5-in03-bins/concert-service` (22:24:40) was
  `a6eee2a7e0ac8e1add6561b033a3c4f351ec1c60172b8ef8ca904599cb4d7f22`
  (85,473,472 bytes), identical to the then-current `cargo build` artifact
  `debug/deps/concert_service-a066790fcbc7e7a3`.
- `debug/concert-service` at 22:23:06 was
  `0106429b9fbb1fb4eb4b9d610d313836637e9008252a68abd939c779fd64724f`
  (85,433,472 bytes): a different artifact of the same source, so the two paths were
  not interchangeable and the previous run's "HEAD build" was not the executed file.

This turn also shows the mechanism: after `cargo test`, the top-level
`debug/concert-service` is a bin-target test harness (`7033fae0…`), while the
`cargo build` artifact stayed in `debug/deps/` (`36095a37…`). A delivery build must
be copied from a `cargo build`, and the executed hash is what certifies it. Both
paths were rebuilt afterwards; the historical values above are the ones measured
this turn.

### Timing headroom

The comments route is the closest call against the composed 2 s limits. Measured
comments reads (client-measured wall time around the HTTP request):

| Run | Comments | Margin vs 2000 ms | Outcome |
| --- | --- | --- | --- |
| This report | 1373.9 ms | 626.1 ms | 200, author present |
| Passing run earlier this turn (host load ~47) | 2047.9 ms | −47.9 ms | 200, author present |
| Prior run under concurrent compilation | 1958.6 ms | 41.4 ms | 200, author present (`/private/tmp/in03-prefix-report.json`, sha256 `49e7c168…`) |
| Failed attempt this turn (host load 14–34) | 2002.1 ms | −2.1 ms | 504 `CONCERT_504 DEADLINE_EXCEEDED`; the same attempt also saw 503 on the block command; report overwritten by the retry |

The same route can therefore complete just under, just over, or fail at the
2000 ms gateway deadline depending on host load from other lanes; the margin on a
quiet run (1373.9 ms) is not a capacity claim. QA08 remains `DEFERRED_BY_USER`.

## Known runner defect (not fixed)

`backend/scripts/mcr/product_scope_ledger_smoke.py` applies only
`0028_product_owner_scope_lifecycle.sql`, `0037_gift_private_child_intents.sql`
and `0042_social_read_child_intents.sql`. The current ledger's durable-capacity
query reads `concert_product_owner_scopes.cancellation_settled_at`
(`src/product_rpc/scope_ledger.rs:170`), which
`0048_root_response_cancellation.sql` adds. Against that partial schema every
`register` fails and the suite reports **0 passed; 6 failed** (measured this turn,
each `Unavailable` "Product scope lifecycle unavailable"). The runner needs the
full 61-migration set, as this turn's verification cluster used. Recorded here,
not fixed.

## Findings and limits

1. The write-order test is load-bearing: reversing the order in
   `RuntimeHooks::succeeded` fails it with `["witness", "retain"]`, while the old
   integration test stayed green at 6/6 in that same mutated tree.
2. Social `/me/blocks` list/block/list-after-block/unblock execute and pass;
   the prior "budget" explanation was a lock-order defect that is fixed.
3. A Concert social block now filters the author: carol's feed stays 200 with Bob
   absent, and Bob's post returns after the unblock.
4. Comments complete inside the two-second ceiling on a quiet run (1373.9 ms) with
   thin margin under load; the gateway outer timeout is aligned to that ceiling.
5. The venue meeting/location producers are still not composed; IN03 cannot claim a
   venue privacy pass until they exist.
6. Provider-dependent reveal/revoke and age-positive cases remain `NOT_RUN`.
7. The committed ledger runner remains defective (see above).
8. No release, QA08, capacity, device, visual or client-cache qualification is
   claimed.

## Cleanup receipt

- Driver `finally`: `stack.close()` → `cleanup = PASS`; owned PostgreSQL, Redis,
  mail sink, TLS material and temp root removed.
- No push or merge to a remote was performed; the merge of local `master` and the
  commits are local to `codex/mcr-exec-w5-in03`.
