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
