# Task 14 — BE42 sealed account-deletion apply (2026-11-06)

Status: **PASS on branch `codex/mcr-exec-w2-deletion`**, commit `3f3c6331f` (base `5a5da504f`).
Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w2-deletion`.
Central register `docs/mcr/task-progress-2026-11-04.json` is untouched: task 47 owns it.

## Ownership gap and proposal seal

The serving account-deletion runtime composed `private_data: None`, so the canonical
Auth->Ticket/Sessions/Sharing/PrivateData saga could never settle the fourth target,
and the aggregate coordinator had no `DeletionOwner` adapter. The recovered
production-composition source (`codex/mcr-be-BE42-production-composition`,
head `e682b5034`, merge-base `80de53f32`) was source-only: it did not compile
(owner imports pointed at non-existent re-exports) and its migration 0068 never
parsed (`IF ... AND CASE ... END THEN`, SQLSTATE 42601), so no proposal seal could
be issued. This change re-qualifies that source on the current base:

- `AccountDeletionRuntime::compose` now registers `PrivateDataDeletionAdapter`
  over `config::private_data_runtime::production_coordinator`.
- `PrivateDataRuntimeOwners::production` registers all seven closed categories
  (Core, Age, Media, Meeting/Rally, Party/Gift, Mural, Intro), each from an actual
  owner factory; missing owners still fail closed as durable INCOMPLETE.
- Aggregate selection is one category per bounded round (`stored_cursor`), so a
  bounded provider page cannot monopolize later categories or fake completion.
- Migration 0068 CASE expression parenthesised; migrations 0067/0068/0070/0071
  apply cleanly in order on a fresh database.

The sealed apply is the whole PrivateData ACK and it exists only when all seven
category receipts are durable COMPLETE; a replay reauthenticates the exact Auth
source and never rewrites the terminal receipt.

## Commands and exit codes

| Command | Exit | Result |
| --- | --- | --- |
| `cargo check -p concert-service --tests` (CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w2-del, JOBS=4) | 0 | all targets typecheck |
| `cargo test -p concert-service --test private_data_production_native -- --ignored --nocapture` | 0 | 1 passed; convergence + idempotent replay + fence |
| `cargo test -p concert-service --test account_private_data_aggregate` | 0 | 11 passed |
| `cargo test -p concert-service --test account_private_data` | 0 | 3 passed |
| `cargo test -p concert-service --test account_private_data_writer_fence` | 0 | 2 passed |
| `cargo test -p concert-service --test account_private_data_aggregate_native -- --ignored --nocapture` | 0 | 1 passed |
| `cargo test -p api-gateway --test unit a_logged_out_session_cannot_delete` | 0 | 1 passed (revoked session -> 401, delete not forwarded) |

Private PostgreSQL 16.14 (`/opt/homebrew/opt/postgresql@16/bin`), socket-only,
`max_prepared_transactions=256`, data dir `/tmp/mcr-be42-pg/data` (mode 0700),
databases `mcr_private_data_production` and `mcr_private_data_aggregate`.

## Acceptance evidence

Deletion request -> sealed apply -> all canonical references removed. The
production fixture seeds Core participant/avatar, Age owner/verification/claim,
Media row, Meeting beacon, Party aggregate, Mural limit; the canonical command
runs seven bounded rounds through `production_coordinator`; the seventh round
returns the whole ACK (`completed_categories=7`).

Before (seeded private rows): participant theme DANGER + avatar row, 1 age
verification + 1 claim + 1 age owner, 1 media row with staged bytes, 1 owned
party aggregate, 1 explicit-audience beacon, 1 mural limit/actor.

Erase receipts after apply (psql, `mcr_private_data_production`):

```
core neutralized=1 avatars=1
media erased=1
age verifs=1 claims=1
mural rows=1
categories complete=7
whole ack=true
```

Post-deletion orphan check for the deleted user:

```
participants_personal=0
avatars=0
media=0
party_input=false
meeting_input=false
mural_limits=0
fences=1
other_participant_theme=DANGER
```

The control user's participant theme is unchanged (DANGER), so the erase is
identity-scoped. `fences=1` is the one immutable common fence.

Second deletion request: the test re-applies the same command, requires a
verifying ACK with the same `fence_revision`, and asserts `completed_at` and the
category-row count are unchanged (no new effects).

Revoked session -> 401: `api-gateway` unit
`high_risk_sessions::a_logged_out_session_cannot_delete_the_account_or_link_a_provider`
drives the ended/revoked session answer and asserts HTTP 401 with the Auth
deletion not forwarded. Concert-side session withdrawal sets terminal
`SESSION_REVOKED` (4401) in `world_ws::SocketTransport::withdraw_account`.

Permanent recreation guards remain active: the test asserts a post-deletion
`concert_mural_limits` insert for the deleted actor is refused; the writer-fence
suite exercises the same permanent guards.

## Limitations

- The Intro category converges on an empty owner in this fixture. Real Intro row
  erasure requires the canonical native-witness/product-scope fixture already
  owned by `tests/intro_deletion.rs` (ignored; canonical mTLS + marker-owned PG),
  which was not rerun here. No introspection guard was disabled or bypassed.
- Sharing terminal evidence is seeded through the real 0054 insert/guard path
  after the aggregate establishes the common fence; the Sharing target itself is
  owned and qualified by its own suite.
- Broker/serving composition (mTLS, App wiring, restart of the full service) is
  outside this test; `AccountDeletionRuntime::compose` is the production entry
  point and now carries the PrivateData owner.

## Cleanup receipts

Private cluster connection was closed by each test (`p.close()`); databases are
isolated and the cluster is stopped and removed after evidence capture
(`pg_ctl stop -m fast`, then the private directory is deleted). No shared
database, volume or checkout was touched.

## Provenance

- Delivered commit: `3f3c6331f feat(account): complete sealed account deletion apply` (23 files, +1891/-14).

- Recovered source: `codex/mcr-be-BE42-production-composition` @ `e682b5034`
  (merge-base `80de53f32`): migrations 0067/0068/0070/0071, media/meeting/party/
  gift/mural owners, `config/private_data_runtime.rs`, aggregate cursor hunk,
  runtime adapter hunk, `giveaway::withdraw_private_input_in`,
  `party_live::wire::load_for_update`/`save_party` visibility, mural write fence.
- Adaptations on this base: owner import paths; migration 0068 CASE parentheses;
  aggregate native fixture bounded rounds; production fixture precision
  (`.123456000Z`, core timestamp precision) and Intro seed removal.
