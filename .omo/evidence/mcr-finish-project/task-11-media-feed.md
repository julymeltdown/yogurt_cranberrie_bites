# Task 11 - BE24/BE31/BE32 media, feed and post composition

Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w1-media-feed`
Branch: `codex/mcr-exec-w1-media-feed`
Base: `501710d293e5309e862997a353714e5753a76da0`
Executed: 2026-11-06
Toolchain: `cargo +1.88.0`, `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w1-mf`, `CARGO_BUILD_JOBS=4`

## Environment

Two isolated private PostgreSQL 16 clusters, loopback only, `max_prepared_transactions=256`:

- `/tmp/mcr-mf-owned` - unix socket cluster (`port=5432`, `listen_addresses=''`), owner
  `mcr_owner`, databases `mcr_concert_be_media`, `mcr_concert_owner_media_live`,
  `mcr_concert_feed`, plus the marker/inode/token layout the owned fixtures require.
- `/tmp/mcr-mf-postpg` - `127.0.0.1:55443`, owner `mcr_owner`, parent database
  `port_rust_post_test_exec` (`system_identifier` 7704865615524555901).
- Per-fixture Redis (`POST_TEST_REDIS_FIXTURE_ROOT=/tmp/mcr-mf-redis-fixture`).

No Docker is available on this host; all native fixture modes above were used.

## Finding and fix

The BE24 producer's conflict path denied a competing same-key `finalize` instead of
converging on the committed receipt. `prelocked_stored_in` compared an unlocked
hint snapshot against the locked row including `revision`; a concurrent same-key
finalize that committed between the two reads changed `revision` and produced
`CommandError::Denied` before the receipt replay path ran. Revision is not linkage:
the locked comparison now covers owner/purpose/target only, and `apply` still
enforces the expected revision/state. `competing_finalize_same_key_converges_on_one_durable_receipt`
now passes.

A stale post-service unit assertion inspected `user_lifecycle_repository.rs`, which
now delegates cleanup to `deletion_receipt::apply`. The assertion was pointed at the
actual cleanup owner (`src/deletion_receipt/mod.rs`); the table list is unchanged.

BE24 production composition is now wired: `config/owner_media_runtime.rs` composes
the existing `OwnerMediaRuntime` as a `ScopedProductProducer` behind the shared
`CanonicalOwnerFence`/delivery publisher, binds each request frame with
`CanonicalOwnerMediaFence::from_request`, and multiplexes media operations over the
base product owner. `CONCERT_OWNER_MEDIA_MODE=uploader` requires guarded gRPC, the
shared canonical composition, bounded `CONCERT_MEDIA_SANITIZER_WORKERS`/
`CONCERT_MEDIA_SANITIZER_QUEUE`, and explicit `CONCERT_MEDIA_RECEIPT_KEYS_JSON`;
`off` reads nothing else. `main.rs` composes it beside Feed; the standalone
`production_owners` constructor no longer carries the media mode.

## Commands and exit codes

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cargo +1.88.0 test -p concert-service --test media -- --test-threads=1` | 0 | `21 passed; 0 failed; 1 ignored` (ignored = `durable_restart_verify`, harness-only after restart) |
| 1b | `cargo +1.88.0 test -p concert-service --test owner_media_composition` | 0 | `4 passed` (disabled mode reads nothing; uploader requires shared canonical scope, bounded workers/queue, explicit nonzero distinct keys) |
| 2 | `cargo +1.88.0 test -p concert-service --test media competing_finalize_same_key` | 0 | `1 passed` after the fix (failed with `Current authorization denied` before) |
| 3 | `cargo +1.88.0 test -p concert-service --test owner_media_live` | 0 | `11 passed; 0 failed` incl. `native_owner_upload_metadata_content_recovery_and_revocation` |
| 4 | `cargo +1.88.0 test -p concert-service --test feed_postgres` | 0 | `1 passed` (pending work replay/conflict/outbox/rollback) |
| 5 | `python3 backend/scripts/mcr/post_concert_feed_postgres.py --test-binary ... --test-sha256 ...` | 0 | `1 passed` owner authorization/persistence/races; report `production_authority: NOT_QUALIFIED` |
| 6 | `cargo +1.88.0 test -p post-service --test unit` | 0 | `141 passed; 0 failed` |
| 7 | `cargo +1.88.0 test -p post-service --lib` | 1 | 44 passed, 1 failed: `config::tests::external_migration_mode_rejects_schema_damage_without_owner_privileges` - requires a container runtime (Docker absent) |
| 8 | `cargo +1.88.0 test -p post-service --test integration -- --test-threads=1` | 1 | `52 passed; 4 failed` - 2 Kafka-broker tests need a broker; `post_user_deletion_fence` x2 return `Unavailable` where `PermissionDenied` is asserted (BE42 lane, files untouched here) |
| 9 | `cargo +1.88.0 check -p concert-service --all-targets` | 0 | `Finished dev profile` |

## Acceptance mapping

- Authenticated upload -> finalize -> authorized read, and read after revocation:
  exercised by `media`/`owner_media_live` native producer suites with a test-only
  fenced authority (`real_authority: NOT_COMPOSED` at HTTP level).
- Feed create/edit/delete, comment and reaction persistence with receipts:
  `concert_feed::tests::postgres_owner_authorization_persistence_and_races` via the
  documented owned-cluster harness (test authority, not production).
- Feed reads over the qualified protocol: `feed_postgres` and `feed_runtime`.
- End-to-end Gateway HTTP upload/read with a real canonical Auth/Friend stack:
  NOT RUN here (no running Auth/Friend/Post/Gateway stack; Docker absent).

## Remaining gaps (not delivered by this task)

1. BE24 production composition is delivered here: `owner_media` media operations are
   composed through the shared canonical fence and Gateway-mounted media routes. HTTP
   qualification with Auth/Friend/Post/Gateway is still NOT RUN on this host.
2. BE31/BE32 mutation transport is still closed: `PostConcertFeedGrpcServer::execute_command`
   and `get_receipt` return `UNAVAILABLE`; the Post owner `Owner::execute` is complete
   and covered by unit/harness tests, but a qualified mutation authority/envelope
   (a `backend/proto` contract change) is required to expose it through Concert/Gateway.
3. Public Gateway media routes are mounted and now have a real composition behind
   them; event feed/posts/comments reads are composed through the Feed producer.
   Feed/post/comment/reaction mutations still depend on (2).

Consequently BE24/BE31/BE32 remain `IN_PROGRESS`; this run fixes the media conflict
regression and records reproducible validation, but does not register them as accepted.

## Commits (branch `codex/mcr-exec-w1-media-feed`)

- `39512c69a` fix(media): converge competing same-key finalize on its receipt
- `53d547059` test(post): inspect the actual deletion cleanup owner
- `d962c7d2f` feat(media): compose uploader media over the canonical owner scope

## Cleanup

Failures above are outside this task's changed files: the Kafka tests require a broker
that this host does not run, and the two `post_user_deletion_fence` assertions belong to
the BE42 account-deletion lane. They are reported, not skipped or weakened.

The two private clusters and the per-fixture Redis processes are stopped after the
run; `/tmp/mcr-mf-*` directories are retained only as local scratch and are not
referenced by committed artifacts.
