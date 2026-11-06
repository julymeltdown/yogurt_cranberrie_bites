# BE37 consumer adoption status — 2026-11-06 execution checkpoint

Status: **IN_PROGRESS / partial**. No acceptance claim. Branch
`codex/mcr-exec-w1-relationship` at base `501710d293e5309e862997a353714e5753a76da0`.

## What was verified on this branch

All commands ran from the isolated worktree with
`CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w1-rel` and `CARGO_BUILD_JOBS=4`.
PostgreSQL work used a fresh marker/inode-owned socket-only PG16.14 cluster from
`backend/scripts/mcr/migration_smoke.Cluster`; no existing data directory was attached.

| Check | Command | Result |
| --- | --- | --- |
| World WS suite | `cargo +1.88.0 test -p concert-service --test world_ws` | exit 0, 20 passed, 0 failed, 4 ignored |
| Relationship producer suite (real PG) | `python3 backend/scripts/mcr/relationship_smoke.py --pg-bin /opt/homebrew/opt/postgresql@16/bin --repository-test <relationship_policy binary>` | exit 0; 22/22 named tests, runtime migrate/verify/reapply, restart persistence PASS, cleanup PASS |
| Attempted actor-only correction | same runner, now-unavailable commits `a4588ac1562c374ee2abef1c78f7fdb3f5b1570b` and `3f3db35dad8a1b6599f5b30e06ce953699d2b570` | NOT_RUN — commits absent from this repository; only the in-Frost UDS-only `authorize_actor_in` path exists at the selected Fastify base. No actual texture resolved. |
| Relationship authority wire | `cargo +1.88.0 test -p concert-service --test relationship_authority` | exit 0, 3 passed, 1 ignored (owned two-database cluster) |
| Social live transport | `cargo +1.88.0 test -p concert-service --test social_live` | exit 0, 8 passed, 22 ignored (owned native runner) |
| Social16e concert payload | `cargo +1.88.0 test -p concert-service --test social_remaining -- --ignored` on private PG16 | 8 passed, 1 failed: `authorization_social_matrix::actual_qa04_http_authorization_matrix` panics because `MCR_QA04_REAL_PLAN` is absent; the frozen `native.py` runner explicitly skips this case and requires seven real canonical principals plus a registered public runtime. |
| Social16e friend payload | `cargo +1.88.0 test -p friend-service --test concert_social_remaining -- --ignored` on private PG16 | exit 0, 5/5 passed |
| Social policy factory | `cargo +1.88.0 test -p concert-service --test social_policy_runtime -- --ignored` | 2 failed in the ad-hoc fixture at `foreign_membership` (`social_policy_runtime.rs:41`): the private schema lacked `concert_world_memberships`. This is a fixture precondition, not copied production evidence; the owned runner is still required for a qualification. |

Radical Social16e count expected by `tests/social_remaining_support/native.py`
(friend 5, concert 8) matches the executed payloads; the remaining concert
failure is the explicitly excluded real HTTP matrix.

## Production paths that already apply the canonical decision

Static read of the base branch found these consumers reading the canonical pair
or the fenced authority result rather than local booleans:

- Presence: `world/recipient_owners/local.rs::read_in` denies inactive subject
  admission and `state.blocked`; `world/recipient_owners.rs` requires
  `remote.actor_allowed()`, `remote.subject_allowed()` and
  `!remote.inherited_block()`.
- Party: `party_live/mod.rs` pair read under the event root; `party_live/product.rs`
  binds `PgRelationshipPolicy<GuardedAuthority>`.
- Gift: `gift_live/private/locks.rs` reads both directional bits; `gift_live/authority.rs`
  builds `PgRelationshipPolicy` over the guarded owner scope.
- Feed: `feed/policy.rs` requires the owner guard scope and local pair revision.
- Intro: `intro_live/authority.rs` requires `actor_allowed && subject_allowed` and
  excludes pair blocks in `visible_locally`.
- Notification/report-adjacent routing: `adapters/out/notification/routing.rs`
  suppresses `SUSPENDED` and both pair directions.
- Social: `social_live/producer.rs` reads `concert_relationship_pairs` and enumerates
  blocked subjects for invalidation.

No second policy source or client-side filter was added.

## Remaining acceptance work

1. One real PostgreSQL integration test that drives the actual presence, party,
   gift, feed, intro and report read entry points after a committed block and a
   committed suspension, asserting each removes the actor and emits/invalidates
   the relevant cache signal. The production functions are module-private today
   (`pub(super)`/`pub(crate)`), so this requires either in-module native tests or a
   narrow test-visible seam; it must not duplicate policy SQL in the test.
2. Route matrix for blocked actors returning 403/404 without private metadata on
   the registered public runtime (blocked by the same seven-principal plan as the
   QA04 matrix).
3. Two-client manual QA: A blocks B, B disappears from A's nearby/party/feed
   within one invalidation while the unblocked control remains visible.
4. `social_policy_runtime` owned fixture schema (world memberships) for qualification.

## Cleanup receipts

Every private cluster run removed its marker/inode-owned root; the reports record
`"cleanup": "PASS"`. `/tmp/be37-social-report.json` and
`/tmp/be37-relationship-1793928384.json` are the retained receipts. No shared
database, volume or build target was modified.
