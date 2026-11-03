# BE08 places & immutable map releases fix review

Status: **Scoped fix review PASS; candidate verified, awaiting integrator disposition.** This is a bounded implementation review of BE08 fixes, not project-wide BE08/BE09 acceptance.

## Candidate and ownership

Review checkout: `/Users/lhs/Desktop/mcr-worktrees/qa-places-fixes`, branch `codex/mcr-qa-BE08-fixes`.
Integration parent commit: `097651c24b741ce0d179ea9b7b8e8a761806d249`.
Initial implementation candidate: `c135bffcb0283aff35180fc14fd9783af975afcb`.
Review fix commit: `a91ed08668aa8aca53200f94d3c25ae0162c2a85`.
Post-commit evidence & documentation HEAD: `bbc152454ebdf1b30361f955a1ad096ec5599fc2` (tree `e04d25f080f736f6820ffeccd75abcaba0b416a9`).

This report (`/Users/lhs/Desktop/mcr-worktrees/qa-places-fixes/docs/mcr/qa/places-fix-review.md`) is the reviewer's sole tracked write. No application code, migrations, contracts, manifests, lockfiles, or root scripts were modified by the reviewer. Main checkout was accessed read-only. No git push was performed.

All 12 changed paths across the candidate relative to parent `097651c` match the authorized BE08 assignment:
- Domain: `backend/crates/concert-domain/src/{lib.rs,place/mod.rs}`, `backend/crates/concert-domain/tests/place.rs`
- Application: `backend/crates/concert-application/src/{lib.rs,place/mod.rs}`, `backend/crates/concert-application/tests/place.rs`
- Persistence: `backend/services/concert-service/src/adapters/out/repository.rs`,
  `backend/services/concert-service/src/adapters/out/repository/place.rs`,
  `backend/services/concert-service/migrations/0003_places_map_releases.sql`,
  `backend/services/concert-service/tests/places.rs`
- Smoke harness & documentation: `backend/scripts/mcr/places_smoke.py`, `docs/mcr/backend/places.md`

---

## Review findings and resolution audit

The prior source review (`docs/mcr/qa/places-review.md` at `fc8c5d9`) identified two functional code findings and one boundary-checker categorization issue. All three were verified against fix candidate `a91ed08` and evidence `bbc1524`:

### Finding 1 [P1]: Migration 0003 trigger self-conflict with statement timestamp
- **Original issue**: Triggers `concert_guard_current_place_state` and `concert_guard_advance_event_map` assigned `clock_timestamp()` while table constraints required `updated_at <= statement_timestamp()` and `bound_at <= statement_timestamp()`. Because `clock_timestamp()` advances past `statement_timestamp()` during execution, subsequent updates threw check constraint violation `23514`.
- **Fix verification**:
  - In `backend/services/concert-service/migrations/0003_places_map_releases.sql`, lines 121 and 140 now use:
    - `NEW.updated_at := greatest(statement_timestamp(), OLD.updated_at);`
    - `NEW.bound_at := greatest(statement_timestamp(), OLD.bound_at);`
  - Explicit ordering checks `NEW.updated_at < OLD.updated_at` and `NEW.bound_at < OLD.bound_at` ensure strict monotonicity without conflicting with statement start.
  - Added indexes on foreign key pairs: `concert_events_map_releases_release` on `(map_id, map_revision)` and `concert_place_current_state_release` on `(map_id, map_revision)`.
  - **Status: CLOSED.**

### Finding 2 [P2]: Lack of transaction seam in `select_physical` for atomic beacon creation
- **Original issue**: `PgPlaceRepository::select_physical` committed its internal transaction before returning, releasing SHARE locks on the place state and map release. If a place was closed concurrently before a caller (such as BE09 physical beacon creation) wrote its record, the caller operated on a stale snapshot.
- **Fix verification**:
  - In `backend/services/concert-service/src/adapters/out/repository/place.rs`, introduced:
    `pub async fn select_physical_in_transaction(transaction: &mut Transaction<'_, Postgres>, ...)`
  - Acquires and retains `FOR SHARE` locks on `concert_events_map_releases` and `concert_map_release_places` / `concert_place_current_state` within the caller's transaction.
  - The standalone `select_physical` now delegates to `select_physical_in_transaction` within an internal transaction, preserving the non-transactional read seam.
  - Integration test `owned_native_places_are_scoped_immutable_and_preserve_current_closure` in `tests/places.rs` explicitly exercises concurrency: holds `select_physical_in_transaction`, executes an insert into `concert_place_selection_race`, spawns a competing closure update, asserts the update is blocked on the SHARE lock, and commits the transaction before verifying the closure update completes.
  - **Status: CLOSED.**

### Finding 3 [P3]: Boundary classification of `backend/scripts/mcr/places_smoke.py`
- **Original issue**: `places_smoke.py` was classified as an unassigned infrastructure script by `scripts/mcr/check-boundaries.py` without an integrator allowance.
- **Clarification**: `places_smoke.py` was explicitly delivered under the BE08 assignment. The boundary checker requires either an allowance or integrator script registration. The script syntax, compilation (`py_compile`), and standalone execution with an isolated PostgreSQL 16 cluster were verified.
- **Status: ACKNOWLEDGED; Integrator owned at merge.**

### Additional Code Quality and Concurrency Verifications
- **Canonical JSON encoding & C06 provenance**:
  - `backend/crates/concert-domain/src/place/mod.rs` implements `write_canonical` with strict ASCII surrogate escaping and codepoint key sorting. Floats and invalid numbers fail closed.
  - Unit test `canonical_json_uses_codepoint_order_and_ascii_surrogate_escapes` tests astral plane characters, private use codepoints, surrogate pairs, and control escapes against exact byte representations.
- **Explicit update branches in publication**:
  - In `PgPlaceRepository::bind_event_map_release`, replaced ambiguous `ON CONFLICT (event_id) DO UPDATE` with explicit `if prior.is_some() { UPDATE ... } else { INSERT ... }` branches under the advisory transaction lock `pg_advisory_xact_lock`, avoiding hidden concurrency races on binding catalog revisions.

---

## Evidence and verification audit

1. **Native worker evidence**:
   - Report: `/Users/lhs/Desktop/mcr-build/be08/evidence/native-20261103141447-postcommit.json` (SHA-256: `978ddd07ba369956515298c87ebeb7b7dba7ae0c48c51073b0fead476b6d8486`).
   - Handoff receipt: `/Users/lhs/Desktop/mcr-build/be08/handoff-bb08-bbc1524.json` (SHA-256: `8e960cb6570a3d4e0c7f5e17ac28d0f0fc3b806f995a8aa4c8e9ab618b84c7e2`).
   - Verified that the SHA-256 digests of all 4 source files recorded in `native-20261103141447-postcommit.json` match the current worktree files exactly:
     - `backend/crates/concert-domain/src/place/mod.rs`: `ee505aa435265ebade6e105d1126e2f059c43c658957dc0e52cee37156507584`
     - `backend/crates/concert-application/src/place/mod.rs`: `af26f6af39d3170cb96dfc524c353daa918468e5108f32b35813bea170c4ba05`
     - `backend/services/concert-service/src/adapters/out/repository/place.rs`: `9a6e004fb6d7a54ba43381fef7e13a65da05709ee82edcb6939b7aeff788b660`
     - `backend/services/concert-service/migrations/0003_places_map_releases.sql`: `b2d4ab07a2addece8ddb4a3e413da2a32605d00ea34082a36f5940d1ff162e7a`
     - `backend/scripts/mcr/places_smoke.py`: `ab0d26ef2274a8d2c39622f83645dab80ac14b4926cc26a21ce647d9f37e684f`
   - Handoff `head_tree` matches `git rev-parse HEAD^{tree}`: `e04d25f080f736f6820ffeccd75abcaba0b416a9`.

2. **Planning & consistency checks**:
   - `python3 scripts/mcr/check-plan.py`: exit 0, PASS (118 tasks, 64 IA entries, planning/source preservation verified).
   - Rust/PG shared target was not executed, preserving the shared target ownership for the integrator.

---

## Limitations and boundaries

1. This review covers the backend domain, application, and persistence implementation of BE08 (Places & Immutable Map Releases) and fixes up to commit `bbc1524`.
2. Public HTTP endpoints, real client/browser routing, and multiplayer Phaser integration remain separate obligations (FE19/BE09/QA07).
3. Migration execution in production requires application migration ledger qualification; smoke test verifies direct SQL migration compatibility on PostgreSQL 16.
4. Goal remains ACTIVE through parent review disposition.

