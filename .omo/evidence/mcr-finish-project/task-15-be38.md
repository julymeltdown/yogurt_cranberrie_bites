# Task 15 — BE38 event settings, notices and flags (2026-11-06)

Status: **PASS for task-15 acceptance on branch `codex/mcr-exec-w2-ops`**, commit
`7bed8f78b feat(operations): complete settings, notices and flags` (base
`f75ec4238`). The central task register is untouched (task 47 owns it).

Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w2-ops`.

## Gap traced

Two production gaps made every operator route unreachable through the public
gateway even though the transactional producers were complete:

1. `product_rpc::operation` classified only report routes for the Operations
   family (`ReportOperation::route`: POST `reports`, GET `me/reports`). Admin
   routes (`admin/settings`, `admin/phase`, `admin/notices`, `admin/places`,
   `admin/reports`, public `notices`) therefore failed as "Unsupported product
   operation" before any producer or role check ran.
2. `ReportStartupSettings::compose_shared` built the `OperationsRuntime` with
   `OperationsOwners::default()`, whose `roles: None` and
   `review: Arc<UnavailableReview>` disabled settings/phase/notice writes for
   everyone and never consulted a durable operator role or reviewed notice
   source.

## Change

- `backend/services/concert-service/src/product_rpc/operations_admission.rs`:
  closed `OperationsOperation` route family mirroring the registered admin/read
  routes, with canonical-uuid path ids, mutation/read payload rules, and the
  registered operation id for each route.
- `product_rpc/mod.rs`, `query.rs`, `owner_media.rs`: new
  `Operation::Operations(..)` variant classified before generic routing, checked
  in admission, and refused by the media owner.
- `operations_live/report_product.rs` + `config/report_runtime.rs`: the scoped
  producer routes `Operation::Operations` through the retained
  `OperationsRuntime`, and production composition now installs
  `PgOperationsRoles` and `PgOperationsReview` (no positive default remains).
- `operations_live/roles.rs`:
  - `PgOperationsRoles::role_in` locks the caller transaction's current
    `concert_event_operator_roles` row (`role='OPERATOR'`, `revoked_at IS NULL`)
    and returns a revisioned `RoleLease`; a missing or revoked grant is
    `CommandError::Denied`, surfaced as HTTP 403 `PERMISSION_DENIED` by
    `runtime.rs:311`.
  - `PgOperationsReview::review_in` requires an exact durable
    `concert_operation_notice_reviews` row for this event, reviewed actor,
    source URI and `notice_review_digest(input)` (title/text/source/confirmed_at/
    state), and returns the revisioned review lease consumed by the existing
    notice producer. An operator role alone is not a review.
- `migrations/0073_operations_authority.sql`: event-scoped operator roles
  (revisioned, revocable, immutable identity) and digest-bound notice reviews
  (immutable identity, revisioned provenance). No role, reviewer or notice source
  is seeded; provisioning is an out-of-band operator concern.
- Tests: `event_operations.rs` production-provider scenario, `place_operations.rs`
  unreviewed-place/closure-provenance scenario, and product route classification
  tests.

Place publication was already bound to the immutable reviewed release
(`place/operations.rs`: closure-only mutation, `proposed != old` refuses new
provenance), so no additional place owner was introduced; the new test proves an
unreviewed place cannot claim `VERIFIED`/`REVIEWED` in place and that a closure
keeps provenance byte-identical.

## Acceptance evidence

| Requirement | Evidence |
| --- | --- |
| Operator sets flags/phase/notice -> persisted | real PG test `production_operator_role_and_reviewed_notice_are_required_and_persisted`: settings row (`parties/social` true), event `LIVE`, notice row `PUBLISHED`, durable role/review rows |
| Non-operator -> 403 | same test: no grant -> `CommandError::Denied`; second user -> `Denied`; revoked operator -> `Denied`; runtime maps Denied to 403 (`operations_live/runtime.rs:311`) |
| Place without current review cannot be published | `unreviewed_place_cannot_be_published_and_closure_preserves_provenance`: `MapVersionMismatch`, current-state revision stays 1 |
| Closure preserves provenance | same test: closure succeeds at revision 2 and `place.provenance` equals the released provenance (`UNREVIEWED` preserved) |
| Phase visible in bootstrap | the updated `concert_events.phase` (LIVE) is the row Bootstrap reads (`bootstrap/mod.rs:244`) and rebinds as `event.phase` (`bootstrap/release.rs:587`) |
| Routes reach the operations owner | unit test `every_registered_admin_route_reaches_the_operations_owner` for GET/PUT settings, POST phase, GET/PUT places, GET/POST/PUT notices, GET admin/public notices, GET admin reports/report detail |

## Commands and exit codes

All with `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w2-ops`, jobs 3.

| Command | Exit | Result |
| --- | --- | --- |
| `cargo +1.88.0 check -p concert-service --all-targets` | 0 | no errors |
| `cargo +1.88.0 test -p concert-service --test social_policy_runtime` | 0 | 1 passed, 2 ignored |
| `cargo +1.88.0 test -p concert-service --test operations_live_contract` | 0 | 5 passed |
| `cargo +1.88.0 test -p concert-service --lib product_rpc::operations_admission` | 0 | 3 passed |
| `event_operations` binary on owned PG16.14 | 0 | 8 passed, 1 ignored |
| `place_operations` binary on owned PG16.14 | 0 | 4 passed |
| `cargo +1.88.0 test -p api-gateway --test concert_product_http` | 0 | 6 passed |

Real PostgreSQL: private owned cluster (`initdb` + socket-only, port 5432,
`max_prepared_transactions = 256`), database `mcr_concert_be_operations`, all 53
migration files applied in order including `0073_operations_authority.sql` at
that time (renumbered to 0076 in the fix round below).
Harness: `/tmp/mcr_task15_pg.py` (outside the repository) using
`backend/scripts/mcr/migration_smoke.Cluster`; cluster stopped and removed.

## Verification-fix round (2026-11-06)

Follow-up commit `ccfc9fc0e fix(operations): qualify BE38 migration and read
routes` on `codex/mcr-exec-w2-ops` (parent `7bed8f78b`) closes the two gaps found
by row-15 verification.

### Migration collision fixed

- Renamed `migrations/0073_operations_authority.sql` to
  `migrations/0076_operations_authority.sql`: master owns
  `0073_venue_location.sql` and `0075_place_notes.sql`, and master now also
  carries `0074_venue_invitations.sql`.
- Composed-ledger proof on a fresh owned PG16.14 cluster: master's full
  migration tree plus this branch's 0076 = 60 files, versions 1-76, zero
  duplicate versions (0073 = venue location, 0074 = venue invitations, 0075 =
  place notes, 0076 = operations authority). `sqlx migrate run` exited 0 and
  applied 1-76; a second `sqlx migrate run` exited 0 as a no-op.
  `_sqlx_migrations` held 60 rows, 60 distinct versions, min 1, max 76, 0 failed.
  Both
  `concert_event_operator_roles` and `concert_operation_notice_reviews` existed.
  Cluster stopped and removed (`cleanup: PASS`).
- The branch-only transactional suite also re-applied the renamed file: 53
  migration files, 0 errors.

### Operations read routes mounted

- New `concert_product/read_routes.rs` registers the seven missing GET
  templates through the existing authenticated `concert_product::dispatch`
  (thin proxy, no domain logic): `admin/settings`, `admin/places`,
  `admin/notices`, `admin/reports`, `admin/reports/{reportId}`, `notices`,
  `notices/{noticeId}`. The module is merged into the product router, and
  `protects`/`is_product`/the unmounted-suffix fallback now cover these paths.
- New `tests/concert_operations_read_routes.rs`: all seven routes reject missing
  bearer auth with 401 `AUTHENTICATION_REQUIRED`; with bearer auth they reach the
  product dispatch boundary and return its fail-closed 503
  `DEPENDENCY_UNAVAILABLE` (no producer is configured in `RouteHarness`) with
  `no-store`/`nosniff` protection, instead of the previous 400 method-mismatch or
  404 fallback. This is transport admission evidence; the 200/403 producer
  representation still requires the configured mTLS producer and identity
  services, as before.

### Commands and exit codes (fix round)

All with `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w2-ops`, jobs 3.

| Command | Exit | Result |
| --- | --- | --- |
| `cargo +1.88.0 check -p concert-service -p api-gateway --all-targets` | 0 | no errors |
| composed `sqlx migrate run` on owned PG16.14 | 0 | 60 files, versions 1-76, 0 duplicates, second run no-op |
| `event_operations` binary on owned PG16 (`--test-threads=1`) | 0 | 8 passed, 1 ignored |
| `place_operations` binary on owned PG16 | 0 | 4 passed |
| `cargo +1.88.0 test -p concert-service --test operations_live_contract` | 0 | 5 passed |
| `cargo +1.88.0 test -p concert-service --lib product_rpc::operations_admission` | 0 | 3 passed |
| `cargo +1.88.0 test -p concert-service --test social_policy_runtime` | 0 | 1 passed, 2 ignored |
| `cargo +1.88.0 test -p api-gateway --test concert_product_http` | 0 | 6 passed |
| `cargo +1.88.0 test -p api-gateway --test concert_operations_read_routes` | 0 | 3 passed |

## Limitations / risks

- The frozen Bootstrap C03 schema (`backend/docs/api/mcr-core.openapi.json`,
  `additionalProperties: false`, integrator-owned) has no `notices` or
  `settings` member. Notice text and feature flags are therefore visible through
  `listConcertNotices`/`getConcertNotice` and `getConcertSettings`, not inside
  the bootstrap document; the operator phase is visible in bootstrap. Adding
  flags/notices to bootstrap requires a contract change by the integrator.
- Route-level proof is product classification + the real transactional producer
  on owned PG. The full mTLS `OwnerGuardScope` dispatch path
  (`operations_live_native`) was not rerun; the native harness requires the
  three-database Auth/Friend/Concert PKI fixture and is heavy for this timebox.
- `migration 0073` was renumbered to 0076; the composed-ledger proof above now
  includes master's 0073 venue location, 0074 venue invitations and 0075 place
  notes.
- `PgOperationsRoles` treats only `role='OPERATOR'` as authoritative for these
  admin actions; `MODERATOR` rows are ignored here and remain BE39's concern.

## Cleanup receipts

- Owned PG cluster: stopped and removed by `Cluster.cleanup()`; report
  `cleanup: PASS`, owned root absent.
- No shared database, volume, or checkout touched; no push/merge performed.
- Temporary runner and logs live under `/tmp`, outside the repository.

## Provenance

- Delivered commit: `7bed8f78b feat(operations): complete settings, notices and flags`
  (11 files, +697/-11).
- Request path retained: `ProductRequest` -> `product_rpc::operation` ->
  `ScopedProductOwner` -> `OperationsRuntime::dispatch` (verified parent frame +
  canonical transaction) -> `PgEventOperations` / `PgReports`.
