# Task 20 — VENU-BE-M01 landmark invitations and conversation

- Base: `5b2b4329d` (`codex/mcr-exec-w3-inv`, worktree `/Users/lhs/Desktop/mcr-worktrees/exec-w3-inv`)
- Status: `INTEGRATED` on the task branch; not merged or pushed.
- Scope: invitation create/detail/list/decision (accept, decline, cancel) bound to a
  reviewed landmark tuple and revision, plus event/pair-scoped conversation
  messages. No bare-user DM route exists.

## Changed implementation

| Area | Path |
| --- | --- |
| Migration | `backend/services/concert-service/migrations/0074_venue_invitations.sql` |
| Domain values/transitions | `backend/crates/concert-domain/src/venue_meeting.rs` |
| Application guards/authority ports | `backend/crates/concert-application/src/venue_meeting/mod.rs` |
| PostgreSQL adapter | `backend/services/concert-service/src/venue_meeting/mod.rs` |
| Route decode/registration | `backend/services/concert-service/src/product_rpc/venue_meeting_admission.rs` |
| Operation registry hook | `backend/services/concert-service/src/product_rpc/mod.rs`, `owner_media.rs` |
| Gateway routes | `backend/services/api-gateway/src/adapters/in/http/concert_product/feature_routes.rs` |
| Real-PG test | `backend/services/concert-service/tests/venue_meeting.rs` |

## Enforced behavior

- `INVITE` create/detail/decision require a reviewed landmark selection. The
  authority must return the exact `catalog_revision` + `landmark_revision`, a
  current `valid_until`, and `can_meet`; otherwise the operation fails closed
  (404/503, never a silent upgrade).
- Decisions use revision CAS and the frozen rights: recipient ACCEPT/DECLINE on
  PENDING; sender CANCEL on PENDING or ACCEPTED; expiry is checked against the
  database clock. Same key/exact body replays the stored result; a different body
  on the same key conflicts.
- Conversation is independently authorized by the friend authority on every
  read/list/send; invitation linkage is checked only as same-event/same-pair
  context and never grants access. ACCEPT does not create conversation rights.
- The venue friend port is bilateral-only. No production bilateral read exists
  yet, so the composed default (`UnavailableVenueMeetingAuthority`, and the
  absent live producer) fails closed with 503. Friendship is never reconstructed
  from the directional following flag.
- W04 has no route, producer or capability here.

## Migration number deviation

The task text names `0054_venue_invitations.sql`; `0054_social_account_deletion.sql`
is already published, and SQLx versions are parsed from the numeric prefix, so a
second `0054` file would fail migration loading. `docs/mcr/contracts/paradise-city-v1/meeting.md`
provisionally names `0102_venue_meeting.sql`, which cannot precede migrations
`0055`–`0072` in an increasing ledger. This branch uses the next free ledger
version, `0074`; the L01/N01 lanes retain `0073`/`0075` assumptions in the plan.
The rename is recorded here rather than changing frozen contracts.

## Verification (this session)

| Command | Result |
| --- | --- |
| `cargo +1.88.0 check -p concert-service --all-targets` (`CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w3-inv`, `CARGO_BUILD_JOBS=4`) | exit 0 |
| `cargo +1.88.0 check -p api-gateway --all-targets` | exit 0 |
| `cargo +1.88.0 test -p concert-service --lib product_rpc::venue_meeting_admission` | exit 0, 2 passed |
| `cargo +1.88.0 test -p concert-domain venue_meeting` | exit 0, 3 passed |
| Private PostgreSQL 16.14 (`mcr_meeting`, user `mcr_owner`, unix-socket-only): migrations `0001_concert_core.sql` + `0074_venue_invitations.sql`, then `venue_meeting --test-threads=1` | exit 0, 2 passed |

Private-PG test output:

```
running 2 tests
test decisions_follow_producer_and_revision_and_stale_landmarks_fail_closed ... ok
test invitation_accept_message_round_trip_and_idempotent_replay ... ok
test result: ok. 2 passed; 0 failed
```

The real-PG test proves:

- A (sender) creates an invitation against reviewed landmark `gate-a` rev `3/4`.
- The same idempotency key/body replays the stored APPLIED result.
- B accepts with expected revision 1; the accept key replays identically.
- A sends one message; B lists it through a friend-only context.
- A block in either direction makes a new invite and a send fail
  `ResourceUnavailable`.
- A non-friend outsider is denied on send.
- Sender-only CANCEL succeeds; recipient ACCEPT after cancel is denied; a stale
  landmark revision is denied; DECLINE is covered.

## Manual QA / artifacts

- None via browser. Public transport remains unavailable by design until the
  bilateral `VENUE_FRIEND_V1` read exists; the gateway route table and concert
  product decoder register the exact M routes so that future composition returns
  503 rather than a default allow.
- Frozen contracts (`backend/docs/api/paradise-city-v1/meeting.openapi.json`,
  `backend/proto/venue/v1/meeting.proto`) are unchanged.
- `docs/mcr/task-progress-2026-11-04.json` is untouched.

## Limitations / risks

- No production `VenueMeetingProductOwner` is composed: the required bilateral
  friend owner read does not exist, so the public slice correctly refuses (503).
  This task delivers the domain, transaction adapter, route registration and
  tests; live composition is blocked on the Friend owner's bilateral read.
- The root closed operation registry (`contracts/domain/v1/operations.json`) and
  generated client remain integrator-owned per the contract; this lane does not
  edit that registry. Route parity for M is a follow-up task 22 obligation.
- The same-native witness table described in the contract handoff is not added:
  no canonical confirmed Auth/Friend registration exists to witness. The
  migration's deferred effect-deadline triggers are present.
- Receipt races for the exact same command key across concurrent transactions
  rely on the primary key; the second transaction returns storage-unavailable
  rather than converging on the stored receipt. Same-key replay after commit is
  covered.

## Cleanup

- Private PostgreSQL cluster is created and removed by the ad-hoc runner under a
  marker-owned temporary directory; its socket/deadline state is not persisted
  in the worktree.
- Build artifacts remain only in `/Users/lhs/Desktop/mcr-build/exec-w3-inv`.


## Attempt 2 — verification defect fixes

- Base head: `1285633ca`; delivered commit: `181e13da5`
  (`fix(venue/meeting): decode context route and bind receipts to the body`) on
  `codex/mcr-exec-w3-inv`.
- Changed files: `backend/services/concert-service/src/product_rpc/venue_meeting_admission.rs`,
  `backend/services/concert-service/src/venue_meeting/mod.rs`,
  `backend/crates/concert-application/src/venue_meeting/mod.rs`,
  `backend/services/concert-service/tests/venue_meeting.rs`,
  `backend/services/concert-service/tests/product_feature_admission.rs`.

### Defects and fixes

1. Context route decode matched only the bare `/venue-meeting` prefix while the
   gateway and OpenAPI contract forward `GET .../venue-meeting/context` verbatim,
   so the forward fell through to the generic 403 instead of the meeting
   admission branch. The decoder now accepts the exact `context` suffix; the
   bare prefix is rejected. Unit test asserts the full path maps to
   `MeetingOperation::Context`; the integration test
   `venue_meeting_context_reaches_the_fail_closed_admission_instead_of_unsupported_403`
   drives the signed gRPC admission service and observes `UNAVAILABLE`/REFUSAL
   with the Context operation recorded by the refusing owner.
2. The receipt digest covered operation/event/target/selection only, so a reused
   idempotency key with different bytes replayed the stored result instead of
   conflicting. The digest now also hashes the canonical command body
   (`expectedRevision` + `decision` for decide; `body` + `invitationId` for
   send; `selection` for create). Mismatch returns
   `MeetingError::RevisionConflict`, which the meeting command mapping renders
   as 409 `REVISION_OR_KEY_CONFLICT`.
3. `require_invitation_landmark` revision, `can_meet` and `valid_until` checks
   were untested, so deleting them kept the suite green. New application tests
   assert denial for catalog-revision mismatch, landmark-revision mismatch,
   `can_meet = false`, `valid_until == now` and expired reviews, plus propagation
   of an unavailable review.

### Verification (this session)

Environment: macOS arm64, `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w3-inv`,
`CARGO_BUILD_JOBS=3`, `cargo +1.88.0`. Private PostgreSQL 16.14 cluster in a
`cd 700` temporary root, unix-socket only (`-c listen_addresses=''`), database
`mcr_meeting`, user `mcr_owner`, migrations `0001_concert_core.sql` +
`0074_venue_invitations.sql` applied fresh for this run.

| Command | Result |
| --- | --- |
| `cargo +1.88.0 check -p concert-service -p api-gateway --tests` | exit 0 |
| `cargo +1.88.0 test -p concert-service --test venue_meeting -- --test-threads=1` (fresh PG16) | exit 0, 3 passed |
| `cargo +1.88.0 test -p api-gateway --test concert_feature_routes` | exit 0, 7 passed |
| `cargo +1.88.0 test -p concert-service --test product_feature_admission venue_meeting_context` | exit 0, 1 passed |
| `cargo +1.88.0 test -p concert-application --lib venue_meeting` | exit 0, 2 passed |

Real-PG output:

```
running 3 tests
test decisions_follow_producer_and_revision_and_stale_landmarks_fail_closed ... ok
test invitation_accept_message_round_trip_and_idempotent_replay ... ok
test reusing_a_command_key_with_different_request_body_conflicts ... ok
test result: ok. 3 passed; 0 failed
```

### Mutation sensitivity

- Removing the `require_invitation_landmark` revision/`can_meet`/`valid_until`
  condition makes `landmark_guard_denies_unreviewed_or_closed_or_stale_reviews`
  fail (confirmed, source restored and re-run green).
- Dropping the body from `digest()` makes
  `reusing_a_command_key_with_different_request_body_conflicts` fail with the
  replayed first result (confirmed, source restored and re-run green).

### Notes

- The first real-PG invocation failed with `error with configuration: empty
  host` because the DSN used `postgresql://mcr_owner@/db?host=/sock`. The
  correction is the encoded socket host form
  (`postgresql://mcr_owner@%2Ftmp%2F…%2Fsocket:5432/mcr_meeting`); it is a
  harness detail, not a product defect.
- The public HTTP surface still refuses with 503 until the bilateral
  `VENUE_FRIEND_V1` read is composed; the 409 assertion is made at the service
  seam (`MeetingError::RevisionConflict`), whose existing mapping is
  `SafeFailure::VersionConflict` → 409.
- `rustfmt --check` reports diffs on these files but not only on new lines; the
  repository carries no `rustfmt.toml` and the base file already differs, so
  formatting was left unchanged to keep the fix diff minimal.
