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
