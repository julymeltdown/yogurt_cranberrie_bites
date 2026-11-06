# Task 22 - prove the venue backend on real PostgreSQL and route parity

Date: 2026-11-06
Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w3-pg`
Branch: `codex/mcr-exec-w3-pg`
Base: `cc8ad1b5b` (`master`)
Status: delivered on the task branch; not pushed or merged.

## Scope

Row 22 of `.omo/plans/mcr-finish-project.md`: prove VENU-BE L01 / M01 / N01
(migrations `0073_venue_location.sql`, `0074_venue_invitations.sql`,
`0075_place_notes.sql`) on a private PostgreSQL 16 cluster, and close the venue
families in the public route-parity gate.

## Changed paths

| Path | Change |
| --- | --- |
| `backend/services/api-gateway/src/adapters/in/http/concert_product/feature_routes.rs` | Mount the seven registered location/notes templates, map them to closed `FeatureRoute` values, and extend receipt/integrity closure plus a route-family unit test. |
| `backend/services/api-gateway/tests/concert_product_http.rs` | Add the seven venue route shapes to the authenticated transport-boundary family test and separate read/DELETE header expectations. |
| `scripts/mcr/contracts/test_route_parity.py` | Add an exact venue-family case (location plus place notes) and a deliberately missing-note-template failure case. |
| `backend/services/concert-service/tests/place_notes.rs` -> `backend/services/concert-service/tests/venue_notes.rs` | Rename the N01 integration target so `cargo test --test venue_*` covers L/M/N. Test content is unchanged. |
| `backend/scripts/mcr/venue_postgres_smoke.py` | New owned-cluster driver: fresh PG16, venue ledger, full ledger, physical restart, failure oracle, venue tests, cleanup. |
| `.omo/evidence/mcr-finish-project/task-22-venue-report.json` | Machine-readable driver report (redacted). |
| `.omo/evidence/mcr-finish-project/task-22-venue-tests.md` | This evidence file. |

No register, lockfile, `docs/source/`, contract registry or shared manifest was
changed.

## Route parity

```
$ python3 scripts/mcr/contracts/route-parity.py
route parity: registry=69 contract_commands=63 templates=222 commands=70 missing=0
route parity ok: every registered command has an exact axum template
exit 0
```

Before the mounts the same command reported seven missing venue commands
(`missing=7`, exit 1): four location routes and three place-note routes. The
seven now mount through the existing authenticated `concert_product::dispatch`
proxy; no gateway domain logic was added.

```
$ python3 -m unittest scripts/mcr/contracts/test_route_parity.py
Ran 5 tests ... OK
exit 0
```

The venue unit case proves exact template matching for
`/venue/location`, `/venue/location/sharing`, `/venue/location/fix`,
`/venue/location/commands/{key}`, `/venue/landmarks/{landmarkId}/notes`,
`/venue/landmarks/{landmarkId}/notes/{noteId}/replies` and
`/venue/landmarks/{landmarkId}/notes/{noteId}`; deleting the final template
prints `MISSING DELETE ... (withdrawVenuePlaceNote)` and exits 1.

## Gateway transport proof

```
$ CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w3-pg CARGO_BUILD_JOBS=3 \
  cargo +1.88.0 test -p api-gateway --lib venue_
running 1 test
test ...venue_location_and_place_notes_families_are_exact_and_closed ... ok
test result: ok. 1 passed; 0 failed
exit 0

$ CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w3-pg CARGO_BUILD_JOBS=3 \
  cargo +1.88.0 test -p api-gateway --test concert_product_http
running 6 tests
test every_mounted_command_family_reaches_the_concert_boundary_behind_authentication ... ok
test unknown_command_suffix_under_a_mounted_family_is_a_closed_concert_404 ... ok
test result: ok. 6 passed; 0 failed
exit 0
```

The family test now includes the seven venue shapes. Unauthenticated requests
get the closed 401 envelope; authenticated requests reach the Concert product
boundary (`503 DEPENDENCY_UNAVAILABLE` in this harness, because no producer is
composed). Read and bodyless DELETE requests no longer carry JSON/idempotency
headers, while DELETE retains its required idempotency key.

## Private PostgreSQL 16 proof

Driver:

```
$ python3 backend/scripts/mcr/venue_postgres_smoke.py \
  --report .omo/evidence/mcr-finish-project/task-22-venue-report.json
exit 0
```

Environment: `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w3-pg`,
`CARGO_BUILD_JOBS=3`, toolchain `+1.88.0`, PostgreSQL 16.14 (Homebrew). The
cluster was initialized with `initdb -U mcr_owner`, `listen_addresses=''`, a
private 0700 Unix-socket directory, and a marker/inode-owned temp root.

### Migrations from empty and ledger

The driver builds a temporary migration directory containing exactly
`0001_concert_core.sql`, `0073_venue_location.sql`,
`0074_venue_invitations.sql`, `0075_place_notes.sql`, then applies it through
the service SQLx migrator so the ledger is real (not hand-written).

| Database | Applied set | Ledger | `--verify-migrations` |
| --- | --- | --- | --- |
| `mcr_concert_venue_location` | 0001, 0073, 0074, 0075 in order | `[1, 73, 74, 75]`, checksums matched | PASS |
| `mcr_concert_meeting` -> `mcr_meeting` | 0001, 0073, 0074, 0075 in order | `[1, 73, 74, 75]`, checksums matched | PASS before rename; ledger rechecked after restart |
| `mcr_concert_venue_full` | full committed set | 59 rows, checksums matched | PASS |
| `mcr_concert_venue_notes` | full committed set (the notes test runs the committed migrator itself) | 59 rows, checksums matched | PASS |

The service URL guard requires an `mcr_concert` database name, while the M01
test requires the exact name `mcr_meeting`. The driver applies the ledger to
`mcr_concert_meeting`, verifies it, then renames the database to `mcr_meeting`
before the test connects. This preserves the genuine SQLx ledger and satisfies
the test's closed name check; the reason is recorded as a limitation below.

### Venue tests on the owned cluster

```
$ cargo +1.88.0 test -p concert-service --test venue_* -- --test-threads=1
running 4 tests
test consent_viewer_scope_freshness_and_revocation ... ok
test missing_bilateral_friend_read_fails_closed ... ok
test public_operation_registration_is_exact ... ok
test stale_fix_is_last_known_and_invalid_input_fails ... ok
test result: ok. 4 passed; 0 failed

running 3 tests
test decisions_follow_producer_and_revision_and_stale_landmarks_fail_closed ... ok
test invitation_accept_message_round_trip_and_idempotent_replay ... ok
test reusing_a_command_key_with_different_request_body_conflicts ... ok
test result: ok. 3 passed; 0 failed

running 1 test
test place_notes_audience_expiry_reply_moderation_and_meeting ... ok
test result: ok. 1 passed; 0 failed
exit 0
```

This covers L01 consent/viewer scope/freshness/revocation/stale-fix failure,
M01 invitation round trip/idempotent replay/block/stale-landmark/producer
decisions, and N01 audience/expiry/reply/moderation/Meet-Here linkage on real
PostgreSQL. Fixtures supply synthetic Friend/Auth/landmark policy only; the
production bilateral friend read is still unavailable and fails closed.

### Physical restart

A sentinel event, participant and `ACTIVE` sharing row were written before the
tests, the cluster was stopped and started again, and the postmaster PID changed
`65063 -> 65168`. The sentinel read back `ACTIVE|1`, and every ledger above was rechecked after restart. The
service verifier was rerun after restart for the location, full and notes
databases; the meeting ledger was rechecked by version/checksum SQL because the
service URL guard rejects the test-mandated `mcr_meeting` name.

### Failure oracle

Two mutated migration directories were supplied to `--verify-migrations` for the
fully migrated database:

| Mutation | Verifier | Exit |
| --- | --- | --- |
| `0075_place_notes.sql` dropped | `receipt verification failed` | 1 |
| `0075_place_notes.sql` content tampered | `receipt verification failed` | 1 |

The dropped and mutated cases both fail non-zero; a `0075`-less tree cannot
silently verify a ledger that already applied version 75.

## Cleanup

The driver removed its marker-owned cluster and temp root after the run:

```
$ ls -d /private/tmp/mcr-pg-7q6rrjk4
No such file or directory
```

No postmaster remains for that root. Build artifacts remain only in
`/Users/lhs/Desktop/mcr-build/exec-w3-pg`. The report's `cleanup` field is
`PASS`.

## Manual QA / limitations

- Manual QA was the real command surface above: route parity, the gateway
  transport harness, and the eight venue integration tests against a live PG16
  cluster, including a real restart and a real verifier failure oracle.
- No browser/public-producer run is claimed. The Concert product RPC still does
  not compose a venue location/notes producer, so authenticated HTTP calls stop
  at the closed boundary (`503`), matching the L01/M01/N01 implementation
  limitation recorded in their evidence files. This task proves the templates,
  route families, domain adapters, migrations and restart behavior.
- The production bilateral friend read is absent; tests use synthetic authority
  ports and the production port remains fail-closed.
- The `mcr_meeting` rename is a test-harness accommodation, not a product
  database policy.
