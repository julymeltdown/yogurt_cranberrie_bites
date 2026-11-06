# Task 49 - qualified post mutation envelope and event-scoped routes

Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w2-feed`
Branch: `codex/mcr-exec-w2-feed`
Base: `6cb31ca709a0717d61d82e3308060049889ce3db`
Merged master: `13d2ee25fcfc60ac02ae91efe8c21649d920091f` (`eb3491dcc`)
Envelope commit: `d51f08d3b93167f763078ef2e824f4c91e882423`
Replay/preview fix: `7b378f8fa1688368f7922e86228f118c8bf5df07`
Build target: `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w2-feed`, `--jobs 3`

## Delivered

The merge resolved the single `product_rpc/mod.rs` conflict by keeping master's
module ordering and the branch's `pub(crate) mod query;`; the full workspace
check and feed suites then passed on the merged tree.

The qualified mutation envelope and the event-scoped routes are delivered:

- `backend/proto/post/v1/concert_feed.proto`: `CommandAuthority` (owner views
  plus the exact original HTTP method, target, body, content-type and
  idempotency key) and the dedicated `PostConcertFeedCommandService` with
  `ExecuteCommand`, `GetReceipt` and the read-only `PreviewCommand`.
- `backend/crates/platform/src/concert_feed_command.rs`: the closed eight-route
  parser, independent Concert-to-Post command signer/verifier, and key
  independence checks. The command service has an exact Post/Caller/method/
  actor-field reservation in `grpc_method_policy.rs`.
- `backend/services/concert-service/src/feed/command_product.rs` and
  `command_transport.rs`: the event-scoped producer, `PostTransport::execute`,
  `receipt` and `preview`, the shared Feed queue (load/replay/enqueue/
  acknowledge), and C04 response projection from the committed Post resource.
  `PreviewCommand` is used because the public qualified read service must reject
  a mutation binding; it is bound to the same whole envelope and can read only
  that command's target and parent.
- `backend/services/post-service/src/concert_feed/command_admission.rs`,
  `command_runtime.rs`, `command_wire.rs` and
  `adapters/in/grpc/concert_feed/commands.rs`: whole-envelope admission, typed
  command, idempotent receipt execution, receipt lookup and the bounded preview
  read. The command service is mounted only when both sides are enabled.
- `backend/services/concert-service/src/product_rpc/mod.rs`: every mounted
  event-scoped post/comment/reaction mutation resolves to
  `Operation::FeedCommand`; no new path falls through to `Unsupported product
  operation`.
- Gateway templates for `POST /posts`, `PATCH|DELETE /posts/:id`,
  `POST /posts/:id/comments`, `PATCH|DELETE /comments/:id` and
  `PUT|DELETE /posts/:id/reaction` now reach the classified producer.
- Configuration: `CONCERT_FEED_COMMAND_MODE` / `POST_CONCERT_COMMAND_MODE` and
  independent command key material. Cross-keys are rejected on both sides.

Two functional gaps found by the live proof were fixed in `7b378f8fa`:

- A duplicate key whose stored receipt is already `SUCCEEDED` now returns the
  stored result without re-running `acknowledge` under a new HTTP request id.
  Re-acknowledging a replay is not required for the write and was rejected by
  the original-request binding.
- `CreateComment` preview no longer tries to resolve the not-yet-created comment
  as its own parent; it reads the parent post carried by the signed route.
- The command write path no longer reuses the product read-tracking
  registration. That registration tracks product reads; the command envelope
  plus the current subject fences are the write authority. The preview read
  remains command-bound.

## Behaviour receipts

| Required behaviour | Evidence |
| --- | --- |
| Duplicate key replay returns the stored result | `feed_postgres` asserts `FeedQueue::load` returns the acknowledged command, target, parent and `SUCCEEDED/200` receipt; a second enqueue returns the same receipt. The composed stack replay returns 200 with the same resource/revision and no new rows. |
| Conflicting body with the same key is rejected | The queue stores the exact typed intent; a changed intent returns `CommandError::Conflict`; the composed stack returns 409 `IDEMPOTENCY_CONFLICT` with unchanged rows. |
| Blocked/unauthorized gets 403/404 with no row | `Feed::prepare` policy/ownership reads run before any queue write; `GuardedFeedAuthority` rechecks phase, admission, deletion fence and parent inside the receipt transaction. The composed stack returns 403 for a non-owner, 404 for a missing resource, and 404 for the blocked pair, with unchanged receipt/work/outbox/Post rows. |
| Every mounted mutation path is classified | `feed_route_classification` covers all eight routes and four invalid shapes. |
| Route parity passes | `route-parity.py`: 69 registry rows, 63 contract commands, 230 templates, 70 command keys, 0 missing. |

## Live composed HTTP proof

`backend/scripts/mcr/closure_vertical_stack.py` was extended with the feed
composition (independent post-read and command keys, `CONCERT_PUBLIC_FEED_MODE`,
command mode, Post read/command mode, event admissions/participants/avatar
settings for the two development principals) and a feed proof section. It uses
the existing real Auth enrollment/signup/verify/login flow for both principals
and the public Gateway routes. The run uses its own ephemeral PG16 cluster,
ephemeral loopback ports and an owned temporary root, and cleans up with
`cleanup=PASS`.

Report: `/Users/lhs/Desktop/mcr-build/exec-w2-feed-task49-feed-final.json`
(`result=PARTIAL`; every feed check below is true; the only `None` checks are
the archive ones).

| Step | HTTP | Body / durable state |
| --- | --- | --- |
| create | 200 | post created; `concert_command_receipts/work/outbox=1/1/1`, `post_posts/receipts/outbox=1/1/1` |
| duplicate create (same key/body) | 200 | same resource/revision; rows unchanged at `1/1/1` and `1/1/1` |
| conflicting create (same key, changed text) | 409 | `IDEMPOTENCY_CONFLICT`; rows unchanged |
| edit | 200 | revision 2; receipts/work/outbox `2/2/2` |
| delete | 200 | `state=DELETED`, revision 3; receipt/work/outbox `3/3/3` |
| second create | 200 | second post revision 1; receipt/work/outbox `4/4/4`, posts `2` |
| comment | 504 `DEADLINE_EXCEEDED`, then retry 200 | the first attempt committed receipt/work/outbox `5/5/5` and the comment; the same-key retry returns the stored result and adds no rows |
| reaction | 200 | `reacted_by_me=true`, `reaction_count=1`, revision 3; receipt/work/outbox `6/6/6` |
| unauthorized edit by operator | 403 | `PERMISSION_DENIED`; rows unchanged at `6/6/6` and Posts `2` |
| missing post edit | 404 | `RESOURCE_UNAVAILABLE`; rows unchanged |
| blocked comment (member blocks operator in the Friend DB) | 404 | accepted blocked/unauthorized 403/404; rows unchanged; block cleared afterwards (`0` active rows) |
| write after closure | 409 | `EVENT_READ_ONLY`; `rows_before == rows_after`, no feed work/outbox row |
| event closure | 200 ACK | event `READ_ONLY`, closure receipt `SUCCEEDED|200`; member closure 403; repeat closure 409 `INVALID_TRANSITION`; forged/expired tokens 401; archive 404 |

The create/replay/conflict row counts prove the replay and conflict paths leave
the durable journal untouched. The comment 504/retry pair is a real
authority-budget observation: the first attempt committed the effect and the
same idempotency key returned it after the projection deadline. No retry was
used for the create or conflict evidence, and the retry did not add a row.

Log excerpts (redacted child logs in the report): the Gateway reports its
existing `signing key fetch failed; keeping the installed keys` warning at
startup; the Post service reports
`POST_EXTERNAL_PUBLISHER is disabled; events retain their registered destination requirements`;
the Concert service reports its health/world listeners. The composed stack
started, served the HTTP steps, restarted nothing, and cleaned up.

## Commands and exits

All commands were run from the worktree with the target directory above.

| Command | Exit | Result |
| --- | --- | --- |
| `git merge 13d2ee25f` (conflict resolution in `product_rpc/mod.rs`) | 0 | merge commit `eb3491dcc` |
| `cargo check --workspace --all-targets --jobs 3` | 0 | full workspace and all test targets on the merged tree |
| `cargo test -p concert-service --test feed_route_classification` | 0 | 1 passed |
| `cargo test -p concert-service --test feed_product_scope` | 0 | 5 passed |
| `cargo test -p concert-service --test feed_wire` | 0 | 3 passed |
| `cargo test -p concert-service --test feed_owner` | 0 | 2 passed |
| `cargo test -p concert-service --test feed_runtime` (owned temporary PKI) | 0 | 4 passed |
| `cargo test -p concert-service --lib command_keys` | 0 | 1 passed |
| `cargo test -p post-service --lib command_wire` | 0 | 2 passed |
| `cargo test -p post-service --lib concert_feed::tests` | 0 | 2 passed, 1 ignored (owned DB test) |
| `cargo test -p post-service --test architecture` | 0 | 1 passed |
| `cargo test -p api-gateway --test architecture --test concert_product_http` | 0 | 4 + 7 passed |
| `python3 -m unittest discover -s scripts/mcr/tests -p test_boundaries.py` | 0 | 46 passed, 1 skipped |
| `python3 scripts/mcr/contracts/route-parity.py` | 0 | `registry=69 contract_commands=63 templates=230 commands=70 missing=0` |
| `python3 -m unittest scripts/mcr/contracts/test_route_parity.py` | 0 | 5 passed |
| `feed_smoke.py --repository-test .../feed_postgres-2e2c2dc4475d9925` | 0 | `PASS`, 1 Rust test, restart persistence `PASS`, cleanup `PASS`, `row_counts=4|4|1`, revision `eb3491dcc` |
| `post_concert_feed_postgres.py --test-binary .../post_service-890b0ab4bd1b3773 --test-sha256 fa28823c...` | 0 | `postgres_owner_authorization_persistence_and_races` passed |
| `python3 backend/scripts/mcr/closure_vertical_stack.py --bin-dir ...` | report `PARTIAL` | all 12 feed checks true; archive checks `None`; cleanup `PASS` |
| `docker info` | 1 | daemon unavailable; compose mode NOT_RUN |

The closure-stack helper maps `PARTIAL` to process exit 2. Two later
exit-capture reruns hit host flakes rather than feed assertion failures
(`SmokeError: sql command timed out`, and a truncated HTTP error body); the
report above is the clean run with every feed check true. The reported result is
`PARTIAL` solely because the archive checks are `None`: the event is
`READ_ONLY`, so `PgPublicArchive::page` returns 404 and the later closure
lifecycle steps still have no composed `LifecycleOwners`
(`ClosureProducer` uses `UnavailableLater`). The archive is not part of task 49.

## Limitations

- First attempts can exceed the two-second product authority budget on this
  composed development stack (504 `DEADLINE_EXCEEDED` / 503
  `DEPENDENCY_UNAVAILABLE`). The same idempotency key returns the stored result;
  the row counts show the first attempt committed and the retry added nothing.
  This is a measured latency limit, not a mock and not a missing effect.
- The composed stack uses the `photo2-world-display-v1` development release and
  is labelled `NOT_ADMITTED`; it makes no QA08, IN03 or release-qualification
  claim. Docker compose mode is NOT_RUN because the daemon is unavailable.
- The stack inserts `concert_event_admissions`, `concert_participants` and
  `concert_avatar_settings` for the two development principals because the
  canonical Feed policy reads those event/participant rows. This is an owned
  development fixture, not production seeding.
- Post create/edit with media ids or resource links is refused with
  `Unavailable`; media/link integration is separate. Empty arrays are accepted.
- The preview read is an internal command-service capability, not a public read
  route. The public qualified read service still accepts only GET bindings.
- The feed smoke and Post owner DB test use isolated owned PostgreSQL clusters
  and local test authority; both record `production_authority: NOT_QUALIFIED`.
- The first native-stack attempt used `/usr/bin/openssl` (LibreSSL 3.3), which
  cannot read the generated Ed25519 delegation key. The live-stack runs use
  OpenSSL 3.6.3. A temporary `feed_runtime` PKI must also use OpenSSL 3 and the
  `URI:` SAN form; LibreSSL rejects the URI entry.
