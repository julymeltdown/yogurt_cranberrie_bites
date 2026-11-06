# Task 21 — VENU-BE-N01 place notes and replies

Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w3-notes`
Branch: `codex/mcr-exec-w3-notes`
Base: `5b2b4329d`
Commits:
- `ea63964b8 feat(venue/notes): implement place notes and replies`
- `3489b6934 fix(venue/notes): renumber place notes migration to 0075`

## Delivered

- Migration `backend/services/concert-service/migrations/0075_place_notes.sql`.
  Renamed from `0073_place_notes.sql` on 2026-11-06 after the venue location
  lane took `0073` and the invitation lane took `0074`; the notes schema now
  takes the next free SQLx version. SQL content is byte-identical to the
  previously reviewed `0073_place_notes.sql` (100% rename, no content change).
  The SQLx ledger parses the numeric prefix, so the file must not collide with
  `0073_venue_location.sql` or `0074_venue_invitations.sql`.
  Root notes and one-level replies, immutable landmark tuple, immutable
  absolute expiry, server-computed one-hour/venue-end-day expiry, body purge on
  withdrawal and moderation, reply inheritance trigger, revision guard.
- Domain values and validation in
  `backend/crates/concert-domain/src/venue_notes/` (`NoteAudience`,
  `NoteExpiry`, `LandmarkSelection`, `NoteText`, `NoteAuthor`,
  `NoteMeetingEntry`, `PlaceNote`, `NotePage`, cursor canonicalization).
- Commands and fail-closed owner seams in
  `backend/crates/concert-application/src/venue_notes/`
  (`CreatePlaceNote`, `CreatePlaceReply`, `NoteListQuery`, `PlaceNoteUseCases`).
- PostgreSQL adapter in
  `backend/services/concert-service/src/venue_notes/mod.rs`
  (`PgPlaceNotes`). Reviewed-place, bilateral-friend, profile, meeting and
  moderation owners are explicit ports; defaults return `503`/no entry.
  Friends reads/writes return `DependencyUnavailable` (503) until a bilateral
  `VENUE_FRIEND_V1` owner is supplied; friendship is never inferred from a
  directional flag.
- Public operation registration: `createVenuePlaceNote`,
  `createVenuePlaceReply`, `withdrawVenuePlaceNote` added to the BE15 command
  registry, and `landmarkId` added to the registry's canonical path parameter
  validation. HTTP gateway mounting remains task 22/24 composition work.

## Migration reference sweep

Renamed with `git mv`; the tree contains no remaining `0073_place_notes`
reference (the only stale hit was this evidence file, updated here). No test,
doc, proto, plan or source file names the migration by number. The only
functional dependency on the number is the `sqlx::migrate::Migrator` that loads
the directory in `tests/place_notes.rs`; it is prefix-driven.

## Keep-required behavior (unchanged by the renumber)

- Friends audience: both read and create call the friend authority; with no
  bilateral owner the composed default returns `DependencyUnavailable` (503)
  and discloses no content. Friendship is never inferred from a directional
  flag.
- Everyone path: unchanged; a stranger sees an `EVERYONE` note and its author
  projection.
- No registry/route edits in the renumber commit. `createVenuePlaceNote`,
  `createVenuePlaceReply`, `withdrawVenuePlaceNote` registration from
  `ea63964b8` is untouched.

## Migration and ledger verification (fresh private cluster)

Fresh cluster `/Users/lhs/Desktop/mcr-build/exec-w3-notes-pg2`
(PostgreSQL 16.14, `max_prepared_transactions=256`, unix socket only,
`listen_addresses=''`), databases `mcr_place_notes_composed` and
`mcr_place_notes_test`.

Composed source set (the delivery-order chain):
- `origin/master` migrations `0001`–`0072` (56 files; the version range is
  sparse — 16 of the 72 version numbers are absent from master. `59` in the
  commands below is the composed total: 56 master files + `0073` + `0074` +
  `0075`. Extracted with
  `git archive origin/master backend/services/concert-service/migrations`);
- `0073_venue_location.sql` from the location lane worktree `exec-w3-loc`;
- `0074_venue_invitations.sql` from the invitation lane worktree `exec-w3-inv`;
- `0075_place_notes.sql` from this branch.

Commands:

| Command | Exit | Result |
| --- | --- | --- |
| `sqlx migrate run --source <composed> --database-url <composed db>` | 0 | applied 59 migrations in order; `73/migrate venue location`, `74/migrate venue invitations`, `75/migrate place notes` all success |
| same command re-run | 0 | no pending migrations, no checksum mismatch |
| ledger query on `_sqlx_migrations` | 0 | `total=59`, `distinct_versions=59`, `duplicate_versions=0`, `failed_rows=0`, `min=1`, `max=75`, `venue_rows=3` |

Ledger tail: `72 private data owner progress true`, `73 venue location true`,
`74 venue invitations true`, `75 place notes true`. There is exactly one row
per version; no duplicate version exists and the notes schema no longer
collides with either concurrent lane.

Test-database ledger (branch-only chain, no sibling migrations)
`_sqlx_migrations` contains `0072` and `0075`, with `0075` exactly once.

## Tests

`PLACE_NOTES_TEST_DATABASE_URL=postgresql://mcr_owner@localhost/mcr_place_notes_test?host=/Users/lhs/Desktop/mcr-build/exec-w3-notes-pg2/sock&port=55439`

| Command | Exit | Result |
| --- | --- | --- |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w3-notes CARGO_BUILD_JOBS=3 cargo +1.88.0 test -p concert-service --test place_notes -- --test-threads=1` | 0 | `1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s` |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w3-notes CARGO_BUILD_JOBS=3 cargo +1.88.0 check -p concert-service --all-targets` | 0 | `Finished dev profile`; pre-existing warnings only (25 lib warnings, 2 in `main.rs`) |

Asserted by the suite (unchanged and re-run after the rename):
- Everyone note visible to a stranger; author profile projected.
- Meet Here entry is an actual M-authorized composer entry and writes no
  invitation/reply row.
- One-hour expiry is exactly `created_at + 1h`; `VENUE_END_DAY` equals the next
  `Asia/Seoul` midnight capped by `concert_meeting_settings.event_cutoff`.
- Reply persists, inherits the parent thread, increments the visible reply count.
- Friends note read and creation with the unavailable bilateral owner returns
  `DependencyUnavailable` (503), not content; with a synthetic bilateral owner a
  friend sees it and a stranger does not.
- Expired note disappears from lists.
- Moderator hide removes the root and its replies and purges stored bodies.

## Receipt idempotency: current behavior (gap, not delivered)

The command seam accepts an `idempotency_key`, but the PostgreSQL adapter
discards it (`let _ = idempotency_key;` in `create_note`, `create_reply`,
`withdraw_note`), and no receipt table or unique constraint exists.

Runtime probe on the private cluster (throwaway two-test file
`tests/zz_duplicate_key_probe.rs`, deleted before commit; output):

```
PROBE duplicate_key first=e467ec80-87f1-4fd9-9b7e-3d71ebb91f9d second=33506125-fc24-44f5-bb25-860ca2d0f3bc same=false rows=2
PROBE stale_reply_replay_error=Conflict
PROBE duplicate_reply_key first=5945b78f-3c32-4c64-bf1f-869451738dec second=dfc68f1d-cbc7-4bb3-b6a6-a420efb81b95 same=false rows=2
test result: ok. 2 passed; 0 failed; ...
```

Explicit statement: **a duplicate-key replay currently creates a second
note** (same key, same body -> two rows, distinct ids, no error). For a reply,
the identical replay errors with `Conflict` only because creating a reply bumps
the parent revision, not because of the key; re-reading the parent revision and
replaying with the same key creates a **second reply** (two rows, distinct ids,
no error). Receipt/idempotency persistence is therefore **an open gap for tasks
49/39**, not a delivered behavior; it must not be claimed as working.

## Limitations / risks

- Receipt/idempotency persistence is not implemented for the three mutations
  (see the gap statement above). `withdraw_note` also ignores the key; a replay
  is bounded only by the revision guard. This belongs to tasks 49/39, and no
  receipt row stores note text.
- HTTP/gateway route mounting is not added (shared composition; task 22/24).
- L01/M01 are concurrent, so reviewed-place, friend, meeting and moderation
  owners are ports. The test authorities are synthetic and prove N01 behavior,
  not those owners.
- Expiry purge is immediate invisibility plus body purge on withdrawal and
  moderation; a scheduled expiry scrubber is not added.
- The branch is 4 commits behind `origin/master`; the composed ledger run used
  the newer master base chain copied into a temporary source directory, so the
  branch itself still carries its original base chain. Reconciliation with
  `origin/master` (including the sibling 0073/0074 migrations) remains a merge
  step for the delivery integrator, not a force-push.

## Cleanup

- Throwaway probe test removed before commit (`git status --short` empty after
  `3489b6934`).
- The composed migration source directory was temporary and has been deleted;
  no copied artifact is retained anywhere. No peer worktree was modified.
- The private cluster `exec-w3-notes-pg2` was stopped and its directory removed
  after the run, and no cluster process remains.
