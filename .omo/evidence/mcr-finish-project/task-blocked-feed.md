# Task - blocked author must not fail the reader's own feed

Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w3-blockfeed`
Branch: `codex/mcr-exec-w3-blockfeed`
Base: `93e63def4e4008fc456a904b2a55401ebcd6b395` (master)
Build target: `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w3-blockfeed`, `--jobs 3`
Owner-down binary: `profile-service` stopped after the unblock read.

## Defect

After carol blocks bob through `PUT /api/concert/events/{event}/me/blocks/{bob}`,
carol's own `GET /api/concert/events/{event}/feed?limit=1` returned
`CONCERT_503 DEPENDENCY_UNAVAILABLE`. The same read returned 200 before the
block and after the unblock. The durable `concert_relationship_pairs` row was
blocked, so the reader's feed failed closed instead of excluding bob's content.

## Layer that was wrong

`backend/services/concert-service/src/adapters/out/public_profile/mod.rs`,
`local_pair_and_members_in`.

- The post-service admission is not the failing layer.
  `RequestReadAuthority::acquire` -> `Access::ListPosts` fences only
  `context.actor_id` (`backend/services/post-service/src/concert_feed/read_authority.rs`).
  `ReadClients::subject` checks the actor's own Auth/Friend view; the Auth owner
  always reports the two block flags as false
  (`backend/services/auth-service/src/concert_authority/owner.rs:51-52`), and
  carol's pair with herself is unblocked. Bob is never inspected there.
- The concert feed author/policy path is the failing layer. Both
  `GuardedPolicy::observe` and `GuardedResources::author`
  (`backend/services/concert-service/src/feed/policy.rs`,
  `resources.rs`) resolve every row author through
  `PublicProfileReader::acquire_local`, which calls `local_pair_and_members_in`.
- That function returned `Status::unavailable("Current public Profile
  unavailable")` when `low_blocks_high || high_blocks_low` was set. A block is
  an authorization result, not an owner outage.
- `feed_error` (`public_profile/mod.rs`) intentionally maps only `NotFound` to
  `FeedError::ResourceUnavailable` (skippable author) and maps `Unavailable`,
  `DeadlineExceeded`, `PermissionDenied` and other codes to
  `DependencyUnavailable`. `Feed::project` skips only
  `ResourceUnavailable`/`PermissionDenied`
  (`backend/crates/concert-application/src/feed/mod.rs`), so the block became a
  503 instead of a filtered row. `Feed::page` propagates it unchanged.

## Fix

`local_pair_and_members_in` now classifies the pair through `pair_visibility`.
Either block direction returns `Status::not_found("Public Profile unavailable")`,
which `feed_error` maps to the same skippable `ResourceUnavailable` used when
the guard reports a blocked subject. Genuine dependency failures continue to
return `Status::unavailable`, which still maps to `DependencyUnavailable` and a
503. The change is confined to the concert-service public-profile adapter and
applies to the feed author path and the comment author filter, both of which
already skip `ResourceUnavailable`.

New unit test:
`blocked_pair_is_skippable_while_an_owner_outage_stays_closed` in
`public_profile/mod.rs`. It asserts both block directions map to
`FeedError::ResourceUnavailable` and that `unavailable()` still maps to
`FeedError::DependencyUnavailable`.

## Real-stack reproduction and proof

Driver: `backend/scripts/mcr/feed_blocked_author_probe.py` (new). It reuses the
IN03 gate composition pattern: seven native services, an owned private PG16
cluster with `max_prepared_transactions=256`, real mTLS, real enrollment through
the Gateway and a private local mail capture, three principals (alice, bob,
carol), event admissions/participants/avatars and one published post per author.
Posts and event rows are seeded directly because the probe isolates the read
path; post creation HTTP is not exercised.

Run against the unfixed binaries (base build):

```
python3 -B backend/scripts/mcr/feed_blocked_author_probe.py \
  --bin-dir /Users/lhs/Desktop/mcr-build/exec-w3-blockfeed/debug \
  --report /Users/lhs/Desktop/mcr-build/exec-w3-blockfeed/probe-before.json
```

Exit 1. The probe stops at `feed-blocked-503`, which is the expected failure:

| Step | HTTP | Time | Observed |
| --- | --- | --- | --- |
| carol feed `limit=1` before block | 200 | 687.7 ms | item `a105ca0d-b254-4f8a-8ba1-c0ce98b14051`, author bob |
| pair row after `/me/blocks` | - | - | low_user=bob `1176c058-...`, high_user=carol `8bd46b2e-...`, `high_blocks_low=true`, `revision=2` |
| carol feed `limit=1` after block | 503 | 307.9 ms | `CONCERT_503`, `details.reason=DEPENDENCY_UNAVAILABLE`, `items=[]` |

The 307.9 ms failure matches the reported 294.5 ms symptom.

Run against the fixed binaries:

```
python3 -B backend/scripts/mcr/feed_blocked_author_probe.py \
  --bin-dir /Users/lhs/Desktop/mcr-build/exec-w3-blockfeed/debug \
  --report /Users/lhs/Desktop/mcr-build/exec-w3-blockfeed/probe-after.json
```

Exit 0, `result=PASS`, `cleanup=PASS`.

| Step | HTTP | Time | Observed |
| --- | --- | --- | --- |
| carol feed `limit=1` before block | 200 | 722.1 ms | item `ed73a0c3-dd31-457a-9960-c432b79f3833`, author bob `ad89bcd4-...` |
| carol blocks bob (`/me/blocks`) | 503 projection | - | pair row low_user=carol `13a23ed6-...`, high_user=bob `ad89bcd4-...`, `low_blocks_high=true`, `revision=2` |
| carol feed `limit=1` after block | 200 | 348.4 ms | `items=[]`, bob's post absent, cursor returned |
| carol unblocks bob | 503 projection | - | pair row `low_blocks_high=false`, `high_blocks_low=false`, `revision=3` |
| carol feed `limit=1` after unblock | 200 | 740.8 ms | item `ed73a0c3-dd31-457a-9960-c432b79f3833` present again |
| profile-service stopped, carol feed `limit=1` | 503 | 4.2 ms | `CONCERT_503`, `details.reason=DEPENDENCY_UNAVAILABLE` |

The owner-down case stops the Profile owner (pid 17344, exit code 0) and keeps
the blocked/unblocked pair unchanged; the feed still fails closed with 503.

Raw receipts:
`/Users/lhs/Desktop/mcr-build/exec-w3-blockfeed/probe-before.json` and
`/Users/lhs/Desktop/mcr-build/exec-w3-blockfeed/probe-after.json`.

## Checks

| Command | Exit | Result |
| --- | --- | --- |
| `cargo +1.88.0 build --manifest-path backend/Cargo.toml --locked --offline -j 3 -p api-gateway -p auth-service -p friend-service -p profile-service -p post-service -p concert-service -p notification-service --message-format=json` | 0 | all seven binaries built |
| `cargo +1.88.0 build ... -p concert-service --message-format=json` (after fix) | 0 | rebuilt concert-service |
| `cargo +1.88.0 test ... -p concert-service --lib` | 101 | 131 passed, 4 failed, 60 ignored; `blocked_pair_is_skippable_while_an_owner_outage_stays_closed` passed |
| `cargo +1.88.0 test ... -p concert-service --test feed_live_read` | 0 | 9 passed |
| `cargo +1.88.0 test ... -p concert-application --test feed` | 0 | 18 passed |
| `python3 -B scripts/mcr/contracts/route-parity.py --root <worktree>` | 0 | registry=69, contract_commands=63, templates=231, commands=70, missing=0 |

The four lib failures are environment-gated tests, not assertions about this
change:
`notification_root_binding_support::pool` panics with
`owned root required: NotPresent` (`NB01_OWNER_ROOT`) in three
`product_rpc::scope_ledger::notification_binding` tests, and
`relationship::consumer_probes::pool` panics with
`owned consumer probe database required: NotPresent`
(`CONCERT_CONSUMER_TEST_URL`). They fail before reaching product code.

## Limits

- On this master base the `/me/blocks` and unblock HTTP projections return
  `CONCERT_503 DEPENDENCY_UNAVAILABLE` while still committing the pair row.
  That is a separate, already diagnosed social retain-order defect, fixed on
  `codex/mcr-exec-w5-in03` by `f0d1f21ae` ("fix(social): retain read children
  before the effect witness"). It is not part of this commit. The pair changes
  are proven by the durable pair rows above; the feed reads use the row, not
  the block response.
- The gateway's outer request timeout on this base is 1 s. A two-author
  `limit=3` feed read exceeded it in a cold run, so the probe uses the reported
  `limit=1` shape after two warm-up reads. The IN03 branch raises the outer
  timeout to the product's two-second ceiling (`4ec06d5ad`); that is not part
  of this commit.
- The owner-down case stops `profile-service` only. `post-service` and the
  other owners were not stopped individually.
- No browser or frontend cache was exercised; this is a backend HTTP proof.
- The probe seeds posts and admissions directly instead of using the Post
  mutation HTTP routes.
