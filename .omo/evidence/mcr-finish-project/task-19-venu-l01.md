# Task 19 - VENU-BE-L01 location consent and friend freshness

Date: 2026-11-06
Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w3-loc`
Branch: `codex/mcr-exec-w3-loc`
Base: `5b2b4329d`
Commit: `1650a6fc210efeaa435f97a07d27b2df54c719d6` `feat(venue/location): implement consent and freshness`

## Deliverables

- `backend/services/concert-service/migrations/0073_venue_location.sql`: one
  sharing row per `(event, owner)`, explicit selected viewers, one replaceable
  phone fix, no movement history. 0053 is already `0053_world_account_sessions`
  at the base commit and 0054..0072 are occupied; the next free version is used
  and this deviation from the plan text is recorded here.
- `backend/crates/concert-domain/src/venue_location.rs`: pure consent state
  (`OFF`/`ACTIVE`/`PAUSED`/`REVOKED`/derived `EXPIRED`), viewer-scope command
  validation, policy invariants, fix validation, server-time freshness projection
  (`PHONE` vs `LAST_KNOWN`), receipt and Seoul day boundary.
- `backend/crates/concert-application/src/venue_location.rs`: use-case port
  `VenueLocation`, command envelope, policy-provider port and receipt builder.
- `backend/services/concert-service/src/venue_location/mod.rs`: PostgreSQL
  adapter. `VENUE_FRIEND_V1` is an explicit authority port whose production
  implementation returns `PolicyUnavailable` (503) until the bilateral Friend
  read exists; it never reconstructs friendship from the directional follow flag.
- `backend/services/concert-service/src/venue_location/routes.rs`: closed public
  route parser plus the four operation tuples.
- `backend/crates/concert-application/src/commands/registry.rs`: four additive
  operation registrations (`getVenueLocationSnapshot`,
  `setVenueLocationSharing`, `publishVenueLocationFix`,
  `getVenueLocationCommand`).
- `backend/services/concert-service/tests/venue_location.rs`: real PostgreSQL
  tests with synthetic Friend/Auth owner evidence.

## Behavior

- A location request does not create a lease; only an explicit ACTIVE publication
  selects viewers. ACTIVE requires 1..`max_viewers` unique non-self viewers, all
  satisfying the friend port at the effect; PAUSED/REVOKED require an empty list.
- Lease `expires_at = min(now + policy.sharing_lease_ms, policy.valid_until)`.
  Expiry is derived on read, never mutated by a read.
- `fresh_until = captured_at + policy.fresh_ms`; within `last_known_ms` the same
  fix is returned as `LAST_KNOWN` with `fresh_until = null`; older or outside
  retention it is omitted. `physical_arrival_verified` is always false.
- PAUSED/REVOKED delete all viewer rows and the precise fix in the same
  transaction, increment the sharing revision and leave no current marker.
- Reads and effects use `clock_timestamp()` inside one transaction; snapshot and
  viewer reads lock the sharing roots they disclose, so a concurrent pause or
  revoke cannot interleave between eligibility and disclosure.
- A blocked or suspended viewer is denied; an unavailable friend authority is
  503 (fail closed), never a permissive default.
- Fix input is rejected when the sample is older than `fix_max_age_ms`, later
  than `server now + future_skew_ms`, outside finite WGS84 bounds, above
  `max_accuracy_m`, or bound to a stale sharing revision.

## Pinned compatibility facts

- The retained Friend proto exposes only a directional following flag plus
  block/mute metadata; `VENUE_FRIEND_V1` therefore cannot be derived from it and
  the production port stays 503.
- `concert_events.timezone` remains CHECK-constrained to `Asia/Seoul`; the
  domain clock uses that fixed offset and server time only.
- The location OpenAPI/proto field names and enums are unchanged; this task adds
  no wire field and no migration to a shared service.
- The command registry is additive: the existing 62 operations keep their
  order and tuples; four venue operations are appended (count 66).

## Checks

| Command | Exit | Result |
| --- | --- | --- |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w3-loc cargo +1.88.0 check -p concert-service --all-targets` | 0 | no errors; pre-existing warnings only |
| `cargo +1.88.0 test -p concert-application --test commands` | 0 | 8 passed, 0 failed (registry count now 66) |
| `cargo +1.88.0 test -p concert-service --test venue_location -- --test-threads=1` | 0 | 4 passed, 0 failed in 0.19s |
| service `--migrate` against the private cluster | 0 | 0073 applied; ledger verified by SQLx |
| `rustfmt +1.88.0 --edition 2021` on the six new/modified Rust files | 0 | formatted; `git diff --check` clean |
| `git status --short` after commit | 0 | empty |

The tests ran against a private PostgreSQL 16.14 cluster with
`max_prepared_transactions=256`, database `mcr_concert_venue_location`, and the
migration applied through the service `--migrate` entry point. The cluster
(`/Users/lhs/Desktop/mcr-build/exec-w3-loc-pg`) was created by the run's
marker/inode-owned helper and removed after the run
(`cleanup: owned cluster stopped and removed`). Tests exercised:
grant and PHONE freshness/TTL; non-selected friend denial; pause deleting the
fix and returning no location; revoke; expiry derived from server time; blocked
and suspended viewer denial; unavailable bilateral friend authority (503);
stale fix as LAST_KNOWN; over-age, future-skew, accuracy, NaN and stale-revision
input rejection; exact route/registry tuples.

## Limitations

- Production `VENUE_FRIEND_V1` remains unavailable (503) until the Friend owner
  supplies the bilateral read. Blocked/suspended behavior is proven through the
  synthetic owner port, not through a live Friend/Auth stack.
- Product RPC/native settlement composition does not yet dispatch these
  operations; the closed route parser and registry entries are delivered, while
  the central composition remains integrator-owned and is untouched.
- Idempotency-key receipt persistence, ASK LOCATION requests, check-in and
  reviewed landmarks are outside this L01 slice and remain unimplemented.
- No frontend, contract, task-progress JSON or shared manifest was changed.

## Attempt 2 - self-provisioning friend fixtures (2026-11-06)

Commit: `47478ed51` `test(venue/location): self-provision bilateral friend fixtures`
(on `codex/mcr-exec-w3-loc`, parent `1650a6fc2`). Only
`backend/services/concert-service/tests/venue_location.rs` changed (+50 lines).

### Root cause

Attempt 1 reported 4/4 green, but on a fresh database with only the committed
migrations, three DB tests failed with `relation "venue_test_follows" does not
exist`. The synthetic Friend/Auth authority reads test-only tables
`venue_test_follows` / `venue_test_restrictions`, and no committed setup
created them: they had been created by undocumented manual DDL on the attempt-1
cluster. A repository grep for either name outside the test file was empty, and
`public.venue_test_follows` was `NULL` after `--migrate`, confirming the gap.

### Fix

- `FIXTURE_DDL` plus `provision(&PgPool)` create both tables with
  `create table if not exists` under one transaction guarded by
  `pg_advisory_xact_lock(7613023461094200116)`, so concurrent test binaries
  cannot race the catalog. `provision` runs from `fixture()`, before any
  fixture row is written.
- `Fixture::cleanup()` deletes this test's rows in dependency order
  (fixture tables, viewers, fixes, sharing, participants, event); the three DB
  tests call it at the end. The empty shared fixture tables remain as
  idempotent test scaffolding.
- Test-only DDL stays in the test file: `grep -rln "venue_test_"` over
  `backend/services/concert-service/migrations`, `backend/crates` and
  `backend/services/concert-service/src` returns nothing.
- The product side is untouched: `0073_venue_location.sql` is unchanged, and
  `PgVenueLocationAuthority::friend_in` still returns
  `VenueLocationError::PolicyUnavailable` (the 503 class). Friendship is still
  never derived from the directional follow flag.

### Verification (fresh private PostgreSQL 16.14)

Cluster: `/Users/lhs/Desktop/mcr-build/exec-w3-loc-pg`, `initdb -U mcr_owner`,
unix-socket only, `max_prepared_transactions=256`, database
`mcr_concert_venue_location`. Build: `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w3-loc`,
`CARGO_BUILD_JOBS=3`, toolchain `+1.88.0`.

| Command | Exit | Result |
| --- | --- | --- |
| `CONCERT_DATABASE_URL=<private> concert-service --migrate` | 0 | 53 migrations applied, ledger success; 0073 present |
| `CONCERT_DATABASE_URL=<private> concert-service --verify-migrations` | 0 | receipts verified |
| `CONCERT_TEST_DATABASE_URL=<private> CONCERT_TEST_OWNER_ROOT=<cluster> cargo +1.88.0 test -p concert-service --test venue_location -- --test-threads=1` | 0 | 4 passed, 0 failed |
| `git diff --check` before commit | 0 | clean |
| `rustfmt +1.88.0 --edition 2021 --check tests/venue_location.rs` | 0 | formatted |
| `git status --short` after commit | 0 | empty |

Self-provisioning proof: after `--migrate`, `select to_regclass('public.venue_test_follows')`
returned `NULL`; both fixture tables were then dropped from the migrated
database and the compiled test binary was rerun unchanged:

```
running 4 tests
test consent_viewer_scope_freshness_and_revocation ... ok
test missing_bilateral_friend_read_fails_closed ... ok
test public_operation_registration_is_exact ... ok
test stale_fix_is_last_known_and_invalid_input_fails ... ok
test result: ok. 4 passed; 0 failed
```

The same result was reproduced through the required cargo command with the
committed setup only. After the run, a single-row count over
`venue_test_follows`, `venue_test_restrictions` and `concert_events` returned
`0|0|0`, proving cleanup.
(`public.venue_test_follows` / `venue_test_restrictions` still exist because
empty idempotent scaffolding is kept; they contain no rows.)

### Fail-closed mutation check (bilateral VENUE_FRIEND_V1)

Temporarily mutating `PgVenueLocationAuthority::friend_in` from
`Err(PolicyUnavailable)` to `Ok(true)` made
`missing_bilateral_friend_read_fails_closed` fail (exit 101, assertion
`left: Ok(LocationSnapshot ...)` vs `right: Err(PolicyUnavailable)`). The file
was restored byte-for-byte (`git status` shows only the intended test edit) and
the full suite reran 4/4 green. Therefore the 503 rule is retained and pinned by
a test that detects a permissive port; it is not reconstructed from the
directional flag.

### Cleanup receipts

- Private cluster stopped (`pg_ctl -m fast stop`, exit 0) and its directory
  removed; `ls -d <root>` reports absent.
- Ad-hoc logs were copied to `task-19-attempt2/` and the originals under
  `mcr-build` were removed; only the shared CARGO_TARGET_DIR remains.
- Attempt-1 content above is preserved unchanged; no register, task-progress
  JSON, manifest, contract or migration was edited in this attempt.

### Artifacts

- `task-19-attempt2/migrate-verify.log`
- `task-19-attempt2/venue-location-final.log` (required cargo command, 4 passed)
- `task-19-attempt2/venue-location-mutation.log` (permissive-port mutation fails the 503 test)
