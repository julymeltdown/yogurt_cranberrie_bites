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

## Call-site regression test

`blocked_pair_row_at_the_call_site_is_a_not_found_denial` drives
`local_pair_and_members_in` itself against an owned PG16 database through
`CONCERT_CONSUMER_TEST_URL` (concert migrations 0001-0004 and 0006, plus the
`concert_event_settings` table used by the existing consumer probe). It inserts
one blocked `concert_relationship_pairs` row for a fresh event with two active
admitted participants, then asserts the call returns `Code::NotFound` and
`feed_error(...) == ResourceUnavailable`; it unblocks the pair and asserts the
same call returns `Ok`.

Real run against the fixed source: 1 passed, 0 failed (`finished in 0.06s`).

Mutation proof: a throwaway detached worktree at `3aa766178` received the same
source and a revert of the call site to the pre-fix
`if low_blocks_high || high_blocks_low { return Err(unavailable()); }`. The
mutant test binary produced:

```
running 1 test
test adapters::out::public_profile::tests::blocked_pair_row_at_the_call_site_is_a_not_found_denial ... FAILED

thread 'adapters::out::public_profile::tests::blocked_pair_row_at_the_call_site_is_a_not_found_denial' panicked at services/concert-service/src/adapters/out/public_profile/mod.rs:481:9:
assertion `left == right` failed
  left: Unavailable
 right: NotFound

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 195 filtered out; finished in 0.06s
```

The consumer probe in the same mutant binary still passed, so the failure is
specific to the reverted call site. The throwaway worktree and its source were
removed after the run; the delivery worktree was never reverted.

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

Run against the fixed binaries with the gateway default of 1000 ms and two
warm-up reads (original `probe-after.json`):

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
| `cargo +1.88.0 test ... -p concert-service --lib` with `CONCERT_CONSUMER_TEST_URL` from an owned PG16 cluster | 101 | 133 passed, 3 failed, 60 ignored; both blocked-pair tests and the consumer probe passed |
| `cargo +1.88.0 test ... -p concert-service --test feed_live_read` | 0 | 9 passed |
| `cargo +1.88.0 test ... -p concert-application --test feed` | 0 | 18 passed |
| `python3 -B scripts/mcr/contracts/route-parity.py --root <worktree>` | 0 | registry=69, contract_commands=63, templates=231, commands=70, missing=0 |

The lib failures are environment-gated tests, not assertions about this
change. With `CONCERT_CONSUMER_TEST_URL` supplied, 133 passed and 3 failed: the
three `product_rpc::scope_ledger::notification_binding` tests panic in
`notification_root_binding_support::pool` with `owned root required:
NotPresent` (`NB01_OWNER_ROOT`). Without that env, the consumer probe test also
panics with `owned consumer probe database required: NotPresent`; that is the
fourth failure in the unfurnished run. They fail before reaching product code.

## Build provenance and gateway configuration

The original receipts `/Users/lhs/Desktop/mcr-build/exec-w3-blockfeed/probe-before.json`
and `probe-after.json` record `candidate.revision` but no binary digest, and
they ran with the gateway default `GATEWAY_REQUEST_TIMEOUT_MS=1000` after two
warm-up reads. That default flaked under load: a two-author `limit=3` feed read
exceeded 1 s and returned 504/503 before the product answered, so those
receipts are not a default-configuration proof of a cold multi-author read.

The A/B verification run (fixed and mutant peers, same pair row) used
`GATEWAY_REQUEST_TIMEOUT_MS=5000` because of that flake; its fixed peer produced
the PASS sequence above and its mutant peer failed `feed-blocked-503` on the
same pair row.

The probe now records `candidate.script_sha256`, the effective
`candidate.gateway_request_timeout_ms`, and SHA-256 plus byte size for all seven
binaries under `candidate.binaries`, and it sets the gateway override
explicitly. The fresh 5 s receipt is
`/Users/lhs/Desktop/mcr-build/exec-w3-blockfeed/probe-after-5s.json`:

| Field | Value |
| --- | --- |
| result / cleanup | PASS / PASS |
| candidate revision | `3aa7661789f821a6c925d2c338177a837f41bc26` |
| probe script SHA-256 | `64422c6706952c3ff477225c00481e5f5e7fda4a9f28198f5df5869281702c3f` |
| gateway request timeout | 5000 ms |
| carol feed before block | 200, bob's post `f66827f8-80fc-49cb-b40d-a08fdaca9cfb` (802.0 ms) |
| pair after block | low_user=carol `69bb99b3-...`, high_user=bob `794c8fed-...`, `low_blocks_high=true`, revision 2 |
| carol feed after block | 200, `items=[]`, cursor returned (232.3 ms) |
| pair after unblock | both block flags false, revision 3 |
| carol feed after unblock | 200, `f66827f8-80fc-49cb-b40d-a08fdaca9cfb` restored (643.9 ms) |
| profile-service stopped | pid 98346, exit 0 |
| carol feed owner down | 503, `DEPENDENCY_UNAVAILABLE` (4.6 ms) |
| api-gateway | `ec250de8cc4ee8bfe3c8f2cf2ec546d8cd77cfe3427c132a18f2206088defbdb`, 59,163,440 bytes |
| auth-service | `3f4d3b601a62e339eadf2d7efb53eb19b9626cf52e1d801cd3c56703b10d403e`, 48,336,192 bytes |
| concert-service | `45c7a5dfaab296eb305160af00ce51ed634a156c8770ad27de0622a01f855975`, 76,419,392 bytes |
| friend-service | `5a36186f96b65419aa00b8205ddf3262705c0dd4ebcbd9270f884a2081579245`, 38,350,832 bytes |
| notification-service | `45d216ffb9ae53f0be83549d4e978d3ad136ae18ea8aca3a7b9ac171b2c6a392`, 34,930,720 bytes |
| post-service | `7e4a7800b4ce777bb15cbdb314ffd487d6355b747b4b69376f2297a4c4b60c2a`, 50,074,528 bytes |
| profile-service | `dde7b374a682d50a8567b5131a2084fb4e0fee123e844ca83d745af6fb0bf4eb`, 40,253,472 bytes |

The receipt was produced at `3aa766178` with the follow-up test and probe edits
present in the worktree. The follow-up commit changes only `#[cfg(test)]` code,
the probe and this evidence, not the service binary, so the digests above remain
the binary under test.

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
