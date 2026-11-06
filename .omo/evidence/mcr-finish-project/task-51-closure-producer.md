# Task 51 — closure producer and `/archive` endpoint

Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w2-closure-producer`
Branch: `codex/mcr-exec-w2-closure-producer` (base `b2676f68c`, delivered `1bc74e3b8`)
Date: 2026-11-06

## Ruling consumed

1. Operator authority = the Ops lane's durable `concert_event_operator_roles` schema
   (renumbered to `0076_operations_authority.sql` by BE38). No second role migration,
   config allowlist or JWT role claim is created. `RootOperatorAuthority::check_in`
   reads the event-scoped `OPERATOR` row `for share` on the caller's transaction,
   requires `revoked_at is null`, uses the row `revision` as both `auth_revision` and
   `role_revision`, and fences authority until `now + 10s`. `retention_in` returns the
   reviewed retention (`CONCERT_CLOSURE_RETENTION_HOURS`, `CONCERT_CLOSURE_POLICY_REVISION`)
   anchored at the caller's `closed_at`.
   Merge ordering: this branch merges after BE38's `0076_operations_authority.sql`;
   the fixture test creates the table with test-only DDL where the base lacks it.
2. Archive authorization follows the frozen contract: `getConcertArchive` is `PUBLIC`.
   The gateway route requires no session and forwards no actor delegation. The concert
   owner refuses with 404 while the event is not `ARCHIVED` and 409 on a stale cursor.

## Change

- `backend/proto/concert/v1/product.proto`: `ProductArchiveService.GetArchive`
  (`ArchiveRequest`/`ArchiveResponse`, JSON bytes only, no identity fields).
- `product_rpc/mod.rs`: `Operation::Closure` (`closeConcertEvent`), exact
  `POST admin/closure` admission, mutation (JSON + idempotency key) classification and
  an 8192-byte body bound before any owner work.
- `product_rpc/owner_media.rs`: exhaustive match includes `Closure` as unavailable.
- `config/closure_runtime.rs` (new):
  - `ClosureSettings` (explicit `CONCERT_CLOSURE_MODE=operator`, retention/policy
    revision, receipt key, archive cursor key, page limit; disabled mode retains nothing).
  - `RootOperatorAuthority` over `concert_event_operator_roles`; `UnavailableLater`
    keeps later closure steps fail-closed until their real owning adapters exist.
  - `BridgeHandoff` receives the single serving `AdmissionBridge` via a watch handoff
    created in `main` and published by the World runtime; `ReadOnlyCommandDriver` is
    constructed per request from that exact bridge, never a second one.
  - `ClosureProductOwner` + `ScopedProductOwner`/delivery reuse. Operator-auth 404 is
    projected as 403 `PERMISSION_DENIED` for the closure command only.
  - `PostPublicArchive` over `post_concert_posts`: `sum(revision)` (monotonic across
    publish/edit/soft-delete) is `source_revision`; only `state='PUBLISHED'` text is
    projected; `media_ids` are always empty; deleted rows are never returned.
  - `ArchiveService` implements the public gRPC read through the retained
    `event_lifecycle::http::archive_response` (validated `Domain_admin_Archive`).
- `config/world_runtime.rs`: `serve_with_ready_and_bridge` /
  `serve_with_ready_and_social_and_bridge` publish `bridge.clone()` to the handoff.
- `main.rs`: loads closure settings, creates the handoff, composes closure after the
  shared canonical owner composition, passes the bridge sender to World, and mounts
  `ProductArchiveService` with `guard_without_delegation`.
- `event_lifecycle/read_only.rs` + `http.rs`: driver-level `close_response` reusing the
  retained closed HTTP projection; `denied_response` for a foreign event.
- Gateway: `GrpcConcertProductClient::archive` (service identity only, no actor
  delegation), public `GET /api/concert/events/:event_id/archive` route with a closed
  `limit`/`cursor` parser, and `is_product` protection.

## Verification

| Command | Exit | Result |
| --- | --- | --- |
| cargo +1.88.0 check -p concert-service -p api-gateway --all-targets (CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w2-closure, jobs 3) | 0 | all targets typecheck |
| cargo +1.88.0 test -p api-gateway --test concert_product_http | 0 | 7 passed, including the new unauthenticated archive boundary test (limit=0 -> 400, no producer -> 503) |
| python3 -B backend/scripts/mcr/event_lifecycle_pg.py --native-binary event_lifecycle --native-binary event_lifecycle_archive --native-binary event_lifecycle_read_only (PATH=postgresql@16) | 0 | result PASS; 3 archive tests (incl. new operator-role fence and PostPublicArchive), 7+8 lifecycle/read-only tests, restart seeds/resumes |
| Owned source pin | - | 729537e9d7b52d2a770d6c2eaca0dd74102e6a67e0ea6f2c794eb4e6fb67b841 matched by all three binaries |

Real-PostgreSQL coverage added in tests/event_lifecycle_archive.rs:

- operator_role_fence_is_event_scoped_revisioned_and_revocation_aware: no role -> denied; granted OPERATOR revision 3 -> fence role/auth revision 3, policy 7, cleanup at +30 days; member -> denied; revoked -> denied.
- public_post_archive_projects_only_current_permitted_content: only PUBLISHED text, deleted row absent, media_ids empty, private handle absent, cursor invalidated with Conflict after an edit, archive revision increments, and the projection validates against Domain_admin_Archive.

Not run (explicit): the live public stack (real Auth + Friend + Concert + Gateway listeners, TLS/bootstrap release, operator token) and therefore the operator closure ACK / member 403 / post-closure 409 through the public HTTP route. The gateway test above proves the public route transport and closed errors without a Concert client.

## Limitations

- Full public-stack proof (real Auth + Friend + Concert + Gateway listeners, TLS keys,
  bootstrap release) is not run in this worktree; the gateway runtime test has no
  Concert product client configured and proves only the transport/closed boundary.
- Later closure steps (`FREEZE_MURAL`…`CLEANUP`) remain unavailable until their real
  owning adapters are composed, so the live producer reaches `READ_ONLY` and the job
  stays pending; `ARCHIVED` (and therefore a live `/archive` 200) requires those owners.
- `backend/proto/` is integrator-owned shared input; this branch carries the archive
  service addition for integration review.

## Live vertical flow attempt (post-merge)

- `0076_operations_authority.sql` landed on origin/master at `127c7406f`; merged into
  the branch as `1def9496a` (no rebase, no force). Five conflicts resolved by unioning
  the archive route with `read_routes`, and by adding bridge-aware World wrappers
  alongside the deletion wrappers.
- `cargo +1.88.0 check -p concert-service -p api-gateway --all-targets` on the merged
  tree: exit 0 (`/tmp/checkmerged.log`, finished 42.95 s).
- Live flow NOT RUN. Blocker: the repo dev driver `backend/scripts/mcr/dev_stack.py`
  starts Auth/Friend/Profile/Post/Notification/Gateway but supplies no
  `CONCERT_*`/World configuration (no `CONCERT_*` in `runtime_smoke.environments`),
  so Concert runs health-only and the closure producer (which requires the single
  World `AdmissionBridge`) is not mounted. The World-capable driver
  `backend/scripts/mcr/in03_stack_runtime.py` is an acceptance harness gated by
  `admitted_cohort` binary pins, `QualificationGuard` allocation deadlines and prepared
  release inputs; the task-51 candidate binaries cannot pass that admission.
  Therefore operator ACK / member 403 / post-closure 409 / live archive statuses are
  not captured. Status: BLOCKED (driver composition), not a product failure.

## World release gate diagnosis (2026-11-06)

Instrumented the exact validation chain (diagnostic markers removed afterwards; no product
code changed): `from_product_files` passes all stages; `spatial::validate_product` fails
only at the avatar texture resource loop, because this sparse checkout omitted
`frontend/public/world/avatars` and `frontend/public/world/reference-scene-v2`.
Every one of the 36 pins in `photo2-world-display-v1/source-binding.json` matches disk;
map id/revision/content/geometry hashes, all three scene scenery bytes, PARADE interface
pins, theme textures and the tiled map all validate.

Side chosen: (a) is false for the release content; the binding is complete and current,
and the validator is correct. The offender was the local checkout view (sparse checkout),
not the release or the validator. Fix applied as local view materialization only:
`git sparse-checkout add docs/mcr/art/evidence/photo2-world-display-v1`,
`frontend/public/world/reference-scene-v2`, `frontend/public/world/avatars`.
No geometry invented, no manifest edited, `ReleasePolicy` unchanged, no synthetic path.

After materialization the release validates (`SPATIAL_SOURCE_DONE`) and Concert proceeds
to its next refusal: "operator closure requires the shared canonical and World
composition" (`ClosureSettings`). The dev driver does not yet configure the canonical
Participant/Intro composition, so the live closure statuses remain NOT_RUN. Driver remains
uncommitted until Concert starts.

## Canonical composition attempt (development only, NOT_ADMITTED)

Exact canonical settings, taken from in03_stack_runtime.py: CONCERT_PARTICIPANT_MODE=live,
CONCERT_PRODUCER_INSTANCE_ID, CONCERT_PARTICIPANT_{AUTH,FRIEND,PROFILE}_GRPC_ENDPOINT,
CONCERT_PARTICIPANT_RECOVERY_KEYS_JSON, CONCERT_PARTICIPANT_RELEASE_DIGEST (from
--inspect-product-release), CONCERT_RELATIONSHIP_SIGNING_SEED/VERIFYING_KEYS,
CONCERT_PROFILE_SELF_SIGNING_SEED, PROFILE_PARTICIPANT_DECISION_VERIFYING_KEYS,
CERT_OWNER_AUTHORITY_ENABLED + AUTHORITY_CONCERT/PROFILE_SETTLEMENT_ENDPOINT,
CONCERT_INTRO_MODE=live + cursor/receipt/pickup keys, real PKI via world_in01_live.pki/tls.

Product fix committed: the closure gate used intro_settings as "canonical"; canonical is
participant + guarded (31ded59e7).

Next blocker (not in this lane): Auth/Friend start before Concert and their
CONCERT_OWNER_AUTHORITY_ENABLED path calls verify_ready against the Concert settlement
endpoint inside a 2 s startup budget, so they exit with "Owner authority unavailable".
Minimal diff needed in the shared dev tooling: start Concert before Auth/Friend, or give
runtime_smoke's Child.wait_listeners/verify a bounded readiness retry for the settlement
endpoint. Driver remains uncommitted; live statuses NOT_RUN.

## Settlement ordering fix (committed 4cbf52bbd)

runtime_smoke.start_order() moves concert-service before auth/friend when it is part of
the composed stack; dev_stack reserves and starts in that order. Regression tests in
test_dev_stack.py fail on the old order. With the fix, all six non-Concert services start
and Concert binds its health listener and begins World startup; it now stops at
"World startup readiness failed" because the dev driver seeds the event manually instead
of using world_in01_prepare.seed_concert, so the reviewed world_activation inputs are
absent. Next minimal step: replace manual seeding with seed_concert (which returns
world_activation) and copy in03's managed_features wiring. Driver remains uncommitted;
live statuses NOT_RUN; PG suite not re-run this round.

## World replay finding + merged-tree PG suite

- world_activation is not an input: world_in01_prepare.seed_concert writes
  "CLOSED_PENDING_NATIVE_QUALIFICATION" and in03 pops it, so seed_concert cannot fix the
  World refusal. The real failure after the settlement ordering fix is
  "World replay bootstrap unavailable" from RemoteInvalidationRuntime::bootstrap
  (config/world_runtime.rs), i.e. the Auth/Friend concert invalidation sources are not
  serving. Consumer: World replay; required input: Auth/Friend invalidation service
  enablement/keys (not present in the dev stack env), not a closure input.
- Merged-tree PostgreSQL suite re-run after rebuild: PASS
  (event_lifecycle_pg.py with the three binaries, exit 0).
- Live closure statuses remain NOT_RUN; driver uncommitted.

## Invalidation replay enablement investigation

- Auth/Friend concert-invalidation services are NOT flag-gated: main mounts them only
  when \`config.tls_material\` is present ("main omits this service on plaintext
  listeners"). Our driver already supplies world.tls material, so both services are
  composed.
- The immediate failure is therefore not enablement but a startup cycle:
  Concert \`RemoteInvalidationRuntime::bootstrap\` (world_runtime.rs) needs Auth/Friend
  invalidation RPCs, while Auth/Friend \`configured_store\` calls
  \`PgOwnerAuthority::verify_ready\` against the Concert settlement endpoint BEFORE
  binding their listeners. Serial and concurrent start orders cannot satisfy both.
- Owners/files for the minimal fix (outside the dev-stack lane):
  auth-service/src/concert_authority/runtime.rs and
  friend-service/src/concert_authority/runtime.rs (bounded verify_ready retry or defer to
  first request) plus concert-service/src/config/world_runtime.rs (bounded
  remote.bootstrap retry) so the two startup directions can overlap.
- No stub, no bypass: services remain real, TLS/credential material real.

## Bounded startup retries (committed 76c5be63f)

- auth/friend: verify_ready now retries retryable statuses (Unavailable, DeadlineExceeded,
  Aborted, ResourceExhausted) at 250 ms intervals inside a 10 s window, each attempt
  clipped to the remaining window and 2 s; permission/auth/invalid requests fail fast.
- concert: remote.bootstrap retries only ApplyError::Unavailable in the same bounded
  pattern; the terminal error remains "World replay bootstrap unavailable".
- dev_stack launches all children before waiting for listeners so the two retrying
  sides can overlap. 9 unit tests (3 per service) cover late-arrival convergence,
  exhaustion with the original error, and fail-fast refusal.
- Runtime timeline: Concert still exhausts the 10 s replay window
  ("World replay bootstrap unavailable"), so the invalidation replay RPC is not merely
  late — it is unreachable/refused for auth/friend in this composed environment. Next
  diagnostic: capture auth/friend stderr and the exact tonic status of the
  AuthConcertInvalidationService call.

## Replay status unmasked (committed 517b1ffbd)

bootstrap now logs "World replay bootstrap unavailable: <ReadError> from source <i> as
consumer <id>" and classifies Unauthenticated/PermissionDenied/InvalidConfiguration as
non-retryable (fail fast) while transport Unavailable keeps retrying.

Observed real status on every attempt (10 s window, source 0 = Auth):
ReadError::Unavailable from source 0 as consumer d59e4eba-...
No Unauthenticated/PermissionDenied/InvalidConfiguration was ever returned, so the guarded
server is not rejecting on policy; the Concert client cannot establish the Auth
invalidation transport. Next diagnostic: probe the Auth invalidation listener over its
configured mTLS endpoint (auth-service listener/TLS composition), not the guard.
Driver remains uncommitted; live statuses NOT_RUN.


## Final delivery: World readiness gate (2026-11-06)

This section supersedes the earlier "Live flow NOT RUN" and "Driver remains uncommitted"
notes in this file. Those earlier notes record the diagnosis path; they are not the
final state.
Product commit: `40f85ad5f` `fix(startup): serve settlement before World readiness with
gated capabilities`. The driver and this evidence are the second commit on
`codex/mcr-exec-w2-closure-producer`.

### Mechanism

The gRPC listener used to wait for the supervised World startup barrier before serving.
The World barrier needs the Auth/Friend concert-invalidation sources, and each of those
services verifies owner-authority settlement against this exact Concert listener before
it binds. Neither side could finish. The gate breaks the cycle without weakening TLS,
workload policy or the settlement authority:

- The tonic server is built and binds before World readiness. A deny-by-default whole
  router layer (added last, therefore outermost) answers `UNAVAILABLE` with the message
  `World not ready` (grpc-status 14) for every request whose path is not exactly
  `/authority.lease.v1.ProducerSettlementService/<method>`.
- A separate `World readiness` component awaits the same oneshot barrier the World
  runtime publishes after replay bootstrap, room claim and `startup_ready`. Success
  flips an `AtomicBool` (Acquire/Release); a dropped sender returns
  `World startup readiness failed`, requests shared shutdown through the existing
  component supervisor and propagates that original error out of `main`.
- `serve_with_shutdown` and every transport/policy layer stay in place. Account deletion
  can only be composed from the World's serving binding, so its service is mounted
  immediately as a slot that answers `UNAVAILABLE` (`Account deletion is starting`)
  until the exact binding composes; the composition failure requests shutdown.
- When no World is configured there is no barrier, so the gate starts open.

The closure driver also projects receipts through its own prepared command
(`ReadOnlyCommandDriver::close_response` -> `self.command` -> shared
`event_lifecycle::http::receipt_response`) instead of the inner lifecycle service.
The inner call bypassed the ephemeral fence permission that `START` installs, which the
live run surfaced as `UNAVAILABLE` on an otherwise valid operator closure. The shared
projection keeps the exact closed receipt statuses for the public route.

### Per-service gate table (exact mounted gRPC names)

The gate is deny-by-default; this table documents each mounted capability and its
observable state. None of these calls reaches an owner while the gate is closed.

| gRPC service | Mount condition in `main` | Gate closed | Gate open |
| --- | --- | --- | --- |
| `authority.lease.v1.ProducerSettlementService` | `participant_owners` (shared canonical composition) | served (the only early answer) | served |
| `concert.v1.ConcertService` (`Bootstrap`) | runtime + release | `UNAVAILABLE` `World not ready` | served |
| `concert.world.v1.WorldService` | runtime + release | `UNAVAILABLE` | served |
| `concert.participant.v1.ConcertParticipantService` | `participant_owners` | `UNAVAILABLE` | served |
| `concert.participant.v1.ConcertParticipantDecisionService` | `participant_owners` | `UNAVAILABLE` | served |
| `concert.product.v1.ProductService` | `product_owner` | `UNAVAILABLE` | served |
| `concert.product.v1.ProductDeliveryService` | participant delivery or product owner | `UNAVAILABLE` | served |
| `concert.product.v1.ProductArchiveService` | closure settings | `UNAVAILABLE` | served |
| `authority.lease.v1.ConcertRootReadService` (feed callback) | feed settings | `UNAVAILABLE` | served |
| `notification.concert.v1.ConcertNotificationTargetAuthority` | notification-target settings | `UNAVAILABLE` | served |
| `concert.account_deletion.v1.AccountDeletionService` | managed deletion + World | `UNAVAILABLE`; after the gate opens, `UNAVAILABLE` `Account deletion is starting` until the World binding composes | served once composed |

### Cargo

`tower.workspace = true` was added to `[dependencies]` (it was already in
`[dev-dependencies]` and the workspace already declares `tower 0.5.2`, resolved to
0.5.3 in `Cargo.lock`). The gate implements `tower::Layer`/`tower::Service`, which
requires naming the real trait crate; tonic's `Router::layer` takes that exact trait, and
tonic does not re-export it. No new crate, no lockfile change.

### Gate tests (`cargo test -p concert-service --bin concert-service`)

| Test | Assertion |
| --- | --- |
| `closed_gate_serves_settlement_and_refuses_every_gated_service` | settlement reaches the inner service once; every gated name returns grpc-status 14 `World not ready` and the inner call count stays 1, so removing the gate fails the test |
| `opening_the_gate_after_world_readiness_serves_gated_services` | `ConcertService` is refused first, then served after the false -> true flip |
| `a_configured_world_starts_closed_and_an_absent_world_serves` | closed only when a World barrier exists |
| `world_failure_requests_shutdown_with_the_original_error` | dropped barrier requests shutdown, preserves `World startup readiness failed` and never opens the gate |
| `early_allowlist_is_exactly_owner_authority_settlement` | allowlist equals `["authority.lease.v1.ProducerSettlementService"]`; no gated name is in it |
| `serving_binary_builds_the_listener_behind_the_world_gate` | the serving binary layers the gate, awaits readiness beside the listener and does not await `world_ready_rx` inside the gRPC component |
| existing component/account-deletion tests | unchanged behavior; 9/9 pass |

## Live vertical flow on the owned development stack (2026-11-06)

Driver: `backend/scripts/mcr/closure_vertical_stack.py`
(sha256 `7e78eb45f6533ee6aee73205a653e5657f054c1e1f94d57f6fc82f8315fc19c0`).
It starts this checkout's native binaries on an owned private PG16 cluster with real
mTLS/PKI, real Auth/Friend/Profile/Post/Notification/Gateway listeners and the supervised
World, drives only public HTTP routes, and records a step only after the durable row is
read back. Banner: development release, `NOT_ADMITTED`; QA08 `NOT_RUN`; IN03 cohort and
release qualification `NOT_CLAIMED`.

```
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w2-closure cargo build \
  -p concert-service -p auth-service -p friend-service -p profile-service \
  -p post-service -p notification-service -p api-gateway -j 3        # exit 0
python3 -B backend/scripts/mcr/closure_vertical_stack.py \
  --bin-dir /Users/lhs/Desktop/mcr-build/closure-flow-bins \
  --report /tmp/closure-dev-report.json                              # exit 2 = PARTIAL
```

Report sha256 `b12ed295d8aa9b84c536c6bf98d52276da9f2ccc26f4314e7fc609c0759773f3`.
Cleanup `PASS`; no owned cluster remains.

### Before the gate (same driver, previous binary)

`/Users/lhs/Desktop/mcr-build/closure-flow-run3.json`:

- Concert: `World supervised failure before cleanup: World replay readiness lost`
- Auth and Friend: `SETTLEMENT_VERIFY_FAILED=Unavailable: Owner authority unavailable`

After the gate, both services start with no failure line and the flow below runs.

### Steps (exact status, redacted body, durable row)

| Step | Request | Status | Redacted body / row | One log excerpt |
| --- | --- | --- | --- | --- |
| 1. Operator row | `SELECT role,revision,revoked_at FROM concert_event_operator_roles` after the `0076_operations_authority.sql` table | n/a | `OPERATOR\|1\|` (revoked_at empty) | no new child log bytes (row read directly) |
| 2. Operator closure ACK | `POST /api/concert/events/{event}/admin/closure` with operator token | `200` | `{"event_id":"f1fd7b85-...","job_id":"da2f44dd-...","revision":1,"state":"PENDING","steps":[READ_ONLY SUCCEEDED, FREEZE_MURAL..CLEANUP PENDING]}` | Concert: `CONCERT_HEALTH_LISTENER=127.0.0.1:57935` and `CONCERT_WORLD_LISTENER=127.0.0.1:57936`; Gateway: `WARN signing key fetch failed; keeping the installed keys` (scheduled, unrelated) |
| 3. Member closure | same route with member token | `403` | `{"code":"CONCERT_403","message":"Concert request unavailable","details":{"reason":"PERMISSION_DENIED"}}` | no new child log bytes |
| 4. Feed-post write (observation) | `POST /api/concert/events/{event}/posts` | `403` | `{"code":"CONCERT_403","details":{"reason":"PERMISSION_DENIED"}}`; consumer: `product_rpc::operation` has no `POST posts` case and refuses it as `Unsupported product operation` (the route was mounted by the gateway lane; feed writes are not classified by this producer) | no new child log bytes |
| 5. Post-closure supported write | `POST .../admin/closure` again, fresh idempotency key | `409` | `{"code":"GATEWAY_409","message":"The requested action is unavailable.","details":{"reason":"INVALID_TRANSITION"}}`; consumer `CloseProducer::apply` -> `Completion::Failed(InvalidTransition)` | no new child log bytes |
| 6. Archive with session | `GET /api/concert/events/{event}/archive` with member token | `404` | `{"code":"GATEWAY_404","details":{"reason":"RESOURCE_UNAVAILABLE"}}`; consumer `PgPublicArchive::page`: phase is `READ_ONLY`, not `ARCHIVED` | no new child log bytes |
| 7. Archive anonymous | same route without a token | `404` | same closed body | no new child log bytes |
| 8. Forged token | closure route with `forged.development.token` | `401` | `{"code":"CONCERT_401","details":{"reason":"AUTHENTICATION_REQUIRED"}}` | no new child log bytes |
| 9. Expired token | closure route with a genuinely signed RS256 token (real key, `exp` 60 s in the past) | `401` | `{"code":"CONCERT_401","details":{"reason":"AUTHENTICATION_REQUIRED"}}` | no new child log bytes |

"no new child log bytes" is the driver's truthful per-step record: these steps emit no
line in the owned children. The status, redacted body and durable row are the evidence.

### Durable rows after the flow

| Query | Result |
| --- | --- |
| `concert_events` phase,revision | `READ_ONLY\|2` |
| `concert_closure_jobs` job_id,actor_id,retention_revision,progress->>'state' | `da2f44dd-888e-57f8-be5e-98a909ab0134\|d64a3e15-f4bf-4ceb-837a-8351cf85c173\|1\|PENDING` |
| `concert_command_receipts` closeConcertEvent state,result_status | `SUCCEEDED\|200` and `FAILED\|` (the fresh-key retry is recorded as the refused transition) |
| `concert_closure_effects` step,owner_revision | `READ_ONLY\|1` |
| `concert_closure_archives` count | `0` |
| `post_concert_posts` count | `0` |

### Archive PUBLIC projection: exact remaining boundary

The authenticated and anonymous archive route both answer the closed `404
RESOURCE_UNAVAILABLE` while the event is `READ_ONLY`. The public projection cannot be
200 in this composition: `PgPublicArchive::page` requires phase `ARCHIVED` and a
`concert_closure_archives` row, which the later closure steps create. Those steps
(`FREEZE_MURAL`..`CLEANUP`) require owners that are not composed in the serving binary
(`ClosureProducer` uses `UnavailableLater`; only the Mural freeze and Social grant owners
exist, and `CleanupRuntime` is not mounted). Implementing those owners is separate
producer work, not this row.

The PUBLIC projection with no private fields is proven on the same merged tree against
real PostgreSQL by the `event_lifecycle_archive` binary (see next section):
`public_post_archive_projects_only_current_permitted_content` asserts only current
`PUBLISHED` text is returned, deleted rows never reappear, `media_ids` are empty, the
private handle is absent, and an edit invalidates the cursor with a conflict;
`event_lifecycle_archive_deletion_invalidates_cursor_and_never_reintroduces_content`
covers deletion. Result remains `PARTIAL`, never `PASS`: the two live archive checks are
recorded as `null`, not fabricated.

## Merged-tree PostgreSQL lifecycle suite (re-run)

```
PATH=/opt/homebrew/opt/postgresql@16/bin:$PATH python3 -B backend/scripts/mcr/event_lifecycle_pg.py \
  --native-binary .../event_lifecycle-d0063be54bb8c15b \
  --native-binary .../event_lifecycle_archive-b474867851f3d9d2 \
  --native-binary .../event_lifecycle_read_only-3ad2f8f5b283afa6 \
  --output /tmp/pg-lifecycle-report.json                              # exit 0
```

- Result `PASS`; 23 SQL checks and the three native binaries pass with
  `BE41_SOURCE_PIN=8e90be2576c36d08b74b5dc1e1b8b73978897f5ad57ffca8b0ce3bdcd14f2383`.
- Binary sha256: `a75730b8...34de` (event_lifecycle), `6bdda438...6388` (archive),
  `978f4f9b...dbc` (read-only); 7 + 3 + 8 native tests including the operator-role fence
  and the public archive projection above.
- Report sha256 `b858406da185cf77c8f2d18b1d06d04cafeea4c0a7d37c1ae9e5d6e75e42272d`.

## Remaining limitations (explicit)

- Live `/archive` 200 and its populated PUBLIC projection remain unreachable until the
  later closure owners are composed; the route currently returns the truthful
  `404 RESOURCE_UNAVAILABLE` at `READ_ONLY`, and the projection is proven by the PG suite.
- `POST /api/concert/events/:event_id/posts` is mounted but unclassified by the product
  producer (`403 PERMISSION_DENIED`, `Unsupported product operation`); it is reported as
  an observation, and the post-closure 409 is proven on the supported closure command.
- This is a development vertical flow, not IN03 cohort admission, QA08 capacity or
  release qualification.
