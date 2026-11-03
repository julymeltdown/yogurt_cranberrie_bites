# BE08 places and immutable map releases independent source review

Date: 2026-11-03. **Bounded source review complete: Candidate `c135bff` implements
C03/C06 immutable map release parsing, closed provenance validation, and event-scoped
place catalog/detail projection. Three actionable findings/limitations were identified:
(1) SQL migration check constraint self-conflict rejecting later updates,
(2) lack of a transaction seam in `select_physical` preventing atomic beacon creation, and
(3) boundary checker rejection of assigned script `backend/scripts/mcr/places_smoke.py`
as an integrator registration prerequisite.
Native Rust build, unit tests, and PostgreSQL execution remain NOT_RUN.**
Implementation status remains UNVALIDATED_CANDIDATE / IN_PROGRESS.

## Scope and authority

QA worktree: `/Users/lhs/Desktop/mcr-worktrees/qa-places`, branch `codex/mcr-qa-BE08`.
Evaluated candidate commit: **`c135bffcb0283aff35180fc14fd9783af975afcb`**.
Candidate parent: `097651c24b741ce0d179ea9b7b8e8a761806d249`.
Sole tracked writable file is this report at its absolute path:
`/Users/lhs/Desktop/mcr-worktrees/qa-places/docs/mcr/qa/places-review.md`.
No application source, migration SQL, shared contracts, manifests, or locks were edited.
No Cargo, rustc, or PostgreSQL processes were executed.

All 12 candidate paths relative to parent `097651c` were reviewed:
- Domain: `backend/crates/concert-domain/src/{lib.rs,place/mod.rs}`, `tests/place.rs`
- Application: `backend/crates/concert-application/src/{lib.rs,place/mod.rs}`, `tests/place.rs`
- Persistence: `backend/services/concert-service/src/adapters/out/repository.rs`,
  `backend/services/concert-service/src/adapters/out/repository/place.rs`,
  `backend/services/concert-service/migrations/0003_places_map_releases.sql`,
  `backend/services/concert-service/tests/places.rs`
- Smoke harness & documentation: `backend/scripts/mcr/places_smoke.py`, `docs/mcr/backend/places.md`

---

## Actionable review findings

### Finding 1: Migration 0003 triggers assign `clock_timestamp()` under `<= statement_timestamp()` check constraints
- **Location**: `backend/services/concert-service/migrations/0003_places_map_releases.sql:66`, `:80–97`, and `:114–126`.
- **Severity**: P1 (Runtime SQL failure on update).
- **Analysis**:
  1. Table `public.concert_place_current_state` defines:
     ```sql
     updated_at timestamptz NOT NULL DEFAULT statement_timestamp(),
     CONSTRAINT concert_place_current_times CHECK (updated_at <= statement_timestamp())
     ```
     However, trigger function `public.concert_guard_current_place_state` on `BEFORE UPDATE` executes:
     ```sql
     NEW.updated_at := greatest(clock_timestamp(), OLD.updated_at);
     ```
     In PostgreSQL, `statement_timestamp()` is fixed at statement start, whereas `clock_timestamp()`
     advances during statement execution. For any later update statement, `clock_timestamp()` is
     strictly greater than `statement_timestamp()`. Assigning `clock_timestamp()` causes the table's
     CHECK constraint `updated_at <= statement_timestamp()` to evaluate to `FALSE`, raising
     `check_violation (23514)`. Every normal operator update to place closure state
     (`OPEN` -> `CLOSED` or `CLOSED` -> `OPEN`) will fail.
  2. Similarly, table `public.concert_events_map_releases` defines:
     ```sql
     bound_at timestamptz NOT NULL DEFAULT statement_timestamp(),
     CONSTRAINT concert_events_map_bound_at CHECK (bound_at <= statement_timestamp())
     ```
     While trigger function `public.concert_guard_event_map_revision` on `BEFORE UPDATE` executes:
     ```sql
     NEW.bound_at := clock_timestamp();
     ```
     Any update advancing `catalog_revision` will violate `bound_at <= statement_timestamp()`
     with SQLSTATE 23514.
- **Recommendation**:
  In `0003_places_map_releases.sql`, align trigger assignments with the check constraints:
  use `NEW.updated_at := greatest(statement_timestamp(), OLD.updated_at);` and
  `NEW.bound_at := statement_timestamp();`, or alternatively relax the CHECK constraints
  to compare against `clock_timestamp()`.

### Finding 2: `select_physical` commits private transaction before return; lock is released before BE09 writes
- **Location**: `backend/crates/concert-application/src/place/mod.rs:88` and `backend/services/concert-service/src/adapters/out/repository/place.rs:434–480`.
- **Severity**: P2 (Concurrency / architectural limitation).
- **Analysis**:
  - `PlaceRepository::select_physical` manages its own transaction internally:
    `let mut tx = self.pool.begin().await.map_err(db)?;`
    It acquires `FOR SHARE` locks on `concert_events_map_releases` and `concert_place_current_state`,
    evaluates closure/eligibility, and then immediately runs:
    `tx.commit().await.map_err(db)?;`
  - Committing the transaction releases all acquired row locks prior to returning `SelectionOutcome`.
  - When a downstream consumer such as BE09 (Beacon creation) calls `select_physical`, it receives
    an eligible `PlaceDetail`, but the lock is already released. A concurrent operator closure update
    can commit in the database before BE09 inserts the beacon row in a subsequent transaction.
  - The current trait `PlaceRepository` provides no transactional seam (e.g. passing an open
    transaction handle `&mut sqlx::Transaction`) to hold the share lock across the caller's write.
- **Recommendation**:
  Document that `select_physical` provides a point-in-time eligibility check only. For atomic
  guarantees during BE09 beacon creation or BE20 giveaway meeting updates, either:
  (a) Provide a transactional repository method accepting an open transaction handle, or
  (b) Enforce atomic verification in the downstream beacon insertion query by joining
      `concert_place_current_state` with `WHERE closure_state = 'OPEN'`.

### Finding 3: Boundary checker rejects explicitly assigned test script as integrator registration prerequisite
- **Location**: `backend/scripts/mcr/places_smoke.py`.
- **Severity**: P3 (Boundary checker registration prerequisite; NOT a worker scope breach).
- **Analysis**:
  - `backend/scripts/mcr/places_smoke.py` was explicitly assigned as one of the 12 writable paths
    in the BE08 worker's authorized scope and ownership register. Its inclusion in the worker
    patch is fully authorized and does not represent an out-of-scope breach.
  - When `scripts/mcr/check-boundaries.py` is invoked with:
    `python3 scripts/mcr/check-boundaries.py --base 097651c24b741ce0d179ea9b7b8e8a761806d249 --lane backend --allow-doc docs/mcr/backend/places.md --allow-doc docs/mcr/qa/places-review.md`
    the tool fails with exit code 1:
    ```
    BOUNDARIES FAIL
    - 'backend/scripts/mcr/places_smoke.py' (index, A): integrator-owned contracts/generated output/CI/scripts/infrastructure
    - 'backend/scripts/mcr/places_smoke.py' (working tree, A): integrator-owned contracts/generated output/CI/scripts/infrastructure
    ```
  - This occurs because `check-boundaries.py` automatically flags any path containing a `scripts`
    segment as central/integrator-owned by default unless explicitly accommodated.
- **Recommendation**:
  The integrator owns reconciling this tool-level registration prerequisite (e.g. updating
  `check-boundaries.py` rules or passing appropriate delegation options during integration
  auditing). The worker's delivery is conformant with its explicit assignment.

---

## Contract, domain, and persistence verification

1. **C03/C06 Immutable Map Release Identity**:
   - `CatalogRelease` strictly validates C06 canonical JSON formatting: sorted keys, compact
     separators, ASCII escaping, and no floating-point numbers.
   - `manifest_digest` removes only the top-level `content_hash` before computing SHA-256:
     verified against golden fixture `contracts/fixtures/assets/map-manifest.json`
     yielding `sha256:21b91a4e02b04943100909d7a008f68cafb9e21b284f26b93ba09ddcc39a4dff`.
   - `geometry_digest` matches golden `sha256:50ef23b282b18f10aaf28d18c9bc1f2ee44ff46504dd81b41df9d7ddca363a94`.
   - `verify_immutable` distinguishes byte divergence (`ImmutableRelease`) from stale hashes.
   - Database enforces global immutability on `concert_map_releases` and `concert_map_release_places`
     via triggers rejecting `UPDATE` and `DELETE`.
2. **Physical Place Eligibility & Provenance**:
   - Virtual concepts are restricted to `CONCEPT_ONLY` and cannot be selected for physical meetups
     (`permits_new_physical_selection() == false`).
   - Physical places require `PLACE_REFERENCE` source kind and non-null HTTP(S) source URI.
   - Verified physical places require `review_state == ReviewState::Reviewed` with non-null reviewer
     and timestamp.
   - `permits_new_physical_selection` strictly requires `Physical + Verified + Open + valid provenance`.
3. **Event Scoping & Fail-Closed Authorization**:
   - `Places` application service enforces event-scoped authorization via `PlaceAuthorization`.
   - All errors other than `Denied` map to `PermissionUnavailable` (fail-closed).
   - Detail and catalog projections verify `snapshot.event_id == event_id`; cross-event data
     returns `PlaceError::InvalidState`.
   - `catalog_valid` enforces full equality of released place metadata against current projection,
     permitting only closure state changes.
4. **Data Isolation & Error Mappings**:
   - Stored places contain only public C03 place metadata; GPS coordinates, beacon descriptions,
     and private instructions are absent.
   - Repository maps PostgreSQL unique violation (`23505`) to `PlaceError::Conflict`, and
     unexpected storage errors to `PlaceError::Unavailable`.

---

## Static check verification

The following static checks were executed in worktree `/Users/lhs/Desktop/mcr-worktrees/qa-places`:

| Check | Command | Result |
|---|---|---|
| Planning gate | `python3 scripts/mcr/check-plan.py` | Exit 0 (PASS; 118 tasks, 64 IA entries) |
| Python compile | `python3 -m py_compile backend/scripts/mcr/places_smoke.py` | Exit 0 (syntax valid) |
| Diff whitespace | `git diff --check 097651c..c135bff` | Exit 0 (no whitespace errors) |

---

## Status and limitations

- Candidate commit `c135bff` is **UNVALIDATED_CANDIDATE / IN_PROGRESS**.
- Rust compilation, `cargo test`, and PostgreSQL execution are **NOT_RUN** because BE04 exclusively
  retains the shared build target at `/Users/lhs/Desktop/mcr-build/backend`.
- Existing tests in `backend/crates/concert-domain/tests/place.rs`, `concert-application/tests/place.rs`,
  and `concert-service/tests/places.rs` represent source coverage intent but are not runtime proof.
- No public HTTP routes, shared router composition, or BE08 task acceptance is claimed.
- Goal remains **ACTIVE** awaiting parent review and disposition.

