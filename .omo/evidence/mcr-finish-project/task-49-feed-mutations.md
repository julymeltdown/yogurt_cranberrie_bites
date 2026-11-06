# Task 49 - qualified post mutation envelope and event-scoped routes

Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w2-feed`
Branch: `codex/mcr-exec-w2-feed`
Base: `6cb31ca709a0717d61d82e3308060049889ce3db`
Delivered commit: `d51f08d3b93167f763078ef2e824f4c91e882423`
Build target: `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w2-feed`, `--jobs 3`

## Delivered

The dead agent's uncommitted outline was completed and corrected. The commit adds:

- `backend/proto/post/v1/concert_feed.proto`: `CommandAuthority` (owner views plus
  the exact original HTTP method, target, body, content-type and idempotency key)
  and the dedicated `PostConcertFeedCommandService` with `ExecuteCommand`,
  `GetReceipt` and the read-only `PreviewCommand`.
- `backend/crates/platform/src/concert_feed_command.rs`: the closed eight-route
  parser, independent Concert-to-Post command signer/verifier, and key
  independence checks. The command service has an exact Post/Caller/method/
  actor-field policy reservation in `grpc_method_policy.rs`.
- `backend/services/concert-service/src/feed/command_product.rs` and
  `command_transport.rs`: the event-scoped producer, `PostTransport::execute`,
  `receipt` and `preview`, the shared Feed queue (load/replay/enqueue/
  acknowledge), and C04 response projection from the committed Post resource.
  `PreviewCommand` is used because the public qualified read service must reject
  a mutation binding; it is bound to the same whole envelope as the mutation and
  can read only that command's target and parent.
- `backend/services/post-service/src/concert_feed/command_admission.rs`,
  `command_runtime.rs`, `command_wire.rs` and
  `adapters/in/grpc/concert_feed/commands.rs`: whole-envelope admission, the
  typed command, idempotent receipt execution, receipt lookup and the bounded
  preview read. The command service is mounted only when both sides are enabled.
- `backend/services/concert-service/src/product_rpc/mod.rs`: every mounted
  event-scoped post/comment/reaction mutation now resolves to
  `Operation::FeedCommand`; no new path falls through to
  `Unsupported product operation`.
- Existing gateway templates for `POST /posts`, `PATCH|DELETE /posts/:id`,
  `POST /posts/:id/comments`, `PATCH|DELETE /comments/:id` and
  `PUT|DELETE /posts/:id/reaction` now reach that classified producer instead of
  the unmounted-route 403. `command_routes` already owned their receipts.
- Configuration: `CONCERT_FEED_COMMAND_MODE` / `POST_CONCERT_COMMAND_MODE` and
  independent command key material. Cross-keys are rejected on both sides.

## Behaviour receipts

| Required behaviour | Evidence |
| --- | --- |
| Duplicate key replay returns the stored result | `feed_postgres` now asserts `FeedQueue::load` returns the acknowledged command, target, parent and `SUCCEEDED/200` receipt after restart; a second enqueue returns the same receipt. Post `execute_effects_in` replays the stored outcome. |
| Conflicting body with the same key is rejected | The queue stores the exact typed intent; a changed intent returns `CommandError::Conflict` (asserted). The producer returns HTTP 409 `IDEMPOTENCY_CONFLICT` when the reloaded command or recomputed target differs. |
| Blocked/unauthorized gets 403/404 with no row | `Feed::prepare` policy/ownership reads run before any queue write; `GuardedFeedAuthority` and `first_execution_in` recheck phase, admission, deletion fence and parent inside the receipt transaction. The DB test flips the policy to denied, gets `Denied`, and the rollback leaves no receipt or work row. A missing event/resource returns 404 before enqueue. |
| Every mounted mutation path is classified | `feed_route_classification` covers all eight routes and four invalid shapes. |
| Route parity passes | `route-parity.py`: 69 registry rows, 63 contract commands, 229 templates, 70 command keys, 0 missing. |

## Commands and exits

All commands were run from the worktree with the target directory above.

| Command | Exit | Result |
| --- | --- | --- |
| `cargo check --workspace --all-targets --jobs 3` | 0 | full workspace and all test targets |
| `cargo test -p concert-service --test feed_route_classification` | 0 | 1 passed |
| `cargo test -p concert-service --test feed_product_scope` | 0 | 5 passed |
| `cargo test -p concert-service --test feed_wire` | 0 | 3 passed |
| `cargo test -p concert-service --test feed_owner` | 0 | 2 passed |
| `cargo test -p concert-service --test feed_runtime` (owned temporary PKI) | 0 | 4 passed |
| `cargo test -p concert-service --lib command_keys` | 0 | 1 passed (closed lowercase/nonzero key parsing) |
| `cargo test -p post-service --lib command_wire` | 0 | 2 passed (intent/duplicate-key/escaped-alias/rebinding) |
| `cargo test -p post-service --lib concert_feed::tests` | 0 | 2 passed, 1 ignored (owned DB test) |
| `cargo test -p post-service --test architecture` | 0 | 1 passed |
| `cargo test -p api-gateway --test architecture --test concert_product_http` | 0 | 4 + 6 passed |
| `python3 -m unittest discover -s scripts/mcr/tests -p test_boundaries.py` | 0 | 46 passed, 1 skipped |
| `python3 scripts/mcr/contracts/route-parity.py` | 0 | `missing=0` |
| `python3 -m unittest scripts/mcr/contracts/test_route_parity.py` | 0 | 5 passed |
| `backend/scripts/mcr/feed_smoke.py --repository-test .../feed_postgres-56aa05370ae74a87` | 0 | `PASS`, 1 Rust test, restart persistence `PASS`, cleanup `PASS`, revision `d51f08d3` |
| `backend/scripts/mcr/post_concert_feed_postgres.py --test-binary .../post_service-890b0ab4bd1b3773 --test-sha256 fa28823c...` | 0 | `postgres_owner_authorization_persistence_and_races` passed |
| `python3 scripts/mcr/backend_smoke.py --mode native --bin-dir ...` | 0 | seven-service stack `PASS`, cleanup `PASS`, revision `d51f08d3` |
| `docker info` | 1 | daemon unavailable; compose mode NOT_RUN |

The commit-pinned JSON receipts are outside the repository at
`/Users/lhs/Desktop/mcr-build/exec-w2-feed-feed-smoke-commit.json` and
`/Users/lhs/Desktop/mcr-build/exec-w2-feed-task49-native-commit.json`.
The feed smoke reports `row_counts=4|4|1` (receipts, work, outbox) with
`restart_persistence=PASS` after the runner compares the set across a
PostgreSQL restart; the native stack reports the public bootstrap route
`GET /api/concert/events/{event}/bootstrap` as a real 401
`AUTHENTICATION_REQUIRED` envelope.

## HTTP evidence and the startup gate

- The gateway HTTP harness issued all eight feed mutation routes twice: without a
  bearer they returned 401 `AUTHENTICATION_REQUIRED`; with a valid harness bearer
  they reached the Concert product boundary and returned 503
  `DEPENDENCY_UNAVAILABLE` because that harness intentionally has no producer.
  A sibling test confirms an unmounted suffix is a closed 404. This proves the
  routes and auth boundary without standing in a mock for the producer.
- The seven-service native stack started, migrated, restarted and cleaned up on
  the delivered commit. Its only HTTP product call is the bootstrap route above.
- No authenticated create/edit/delete/comment/reaction was issued on that stack.
  The stack runs Concert in health-only mode (`CONCERT_RUNTIME_MODE=local`, no
  product release bootstrap), so `FeedRuntimeSettings` is `None`, the feed
  producer and command runtime are not composed, and the command-mode variables
  are not set. The composed stack therefore cannot reach the new path; enabling
  it needs the product release bundle and the independent command keys, which is
  outside this worktree's startup gate. No mock HTTP response was substituted.
  Docker compose mode remains NOT_RUN because `docker info` exits 1.

## Limitations

- Live HTTP create/edit/delete/comment/reaction on the composed seven-service
  stack: NOT_RUN, due to the product-release startup gate above.
- The Post owner test and feed smoke use isolated owned PostgreSQL clusters and
  local test authority; both record `production_authority: NOT_QUALIFIED`.
  They do not qualify live Auth/Friend or the full external participant flow.
- Post create/edit with media ids or resource links is refused with
  `Unavailable` in `first_execution_in`; media/link integration is a separate
  obligation. Empty media/link arrays are accepted.
- The preview read is an internal command-service capability, not a new public
  read route. The public qualified read service still accepts only GET bindings.
- The first native attempt used `/usr/bin/openssl` (LibreSSL 3.3), which cannot
  read the generated Ed25519 delegation key and failed before any service
  started. The recorded run uses OpenSSL 3.6.3 and passed.
