# BE08 places and immutable map releases

Date: 2026-11-03. Status: **IN_PROGRESS / NATIVE_GATE_PASS**. The original candidate received an independent source review. Review fixes add the transactional selection seam for BE09, correct the SQL statement-time checks, and fix canonical JSON ordering and ASCII escaping. Focused Rust tests and one fresh owned-PostgreSQL gate pass. Integrator review and acceptance remain outstanding; no task, IA entry or production route is accepted here.

## Ownership and source basis

- Branch `codex/mcr-be-BE08`; isolated worktree `/Users/lhs/Desktop/mcr-worktrees/be-places`.
- Parent prerequisite commits: `f440df302c32e5490c1f95a9b1a7a1f61c72c2d2` exposes retained domain validation dependencies and `097651c24b741ce0d179ea9b7b8e8a761806d249` exposes retained JSON/digest dependencies. Those commits are parent-owned and are not restated as BE08 implementation.
- Only the integrator publishes reviewed work to `julymeltdown/mcr:master`. No push, Docker, external venue call, product route, shared router, generated contract, manifest, lockfile or `docs/source/` change is part of this worker patch.
- The assigned BE08 row is `MCR_IMPLEMENTATION_PLAN_V2_EN.md` line 463. C03 supplies `PlaceList`/`PlaceResponse`; C06 supplies the closed `MapManifest`, `Place` and `Provenance` definitions plus the synthetic golden fixture. Analysis documents inform review but do not silently change the product contract.
- All Rust, SQL, tests and this document are newly authored. No Bigyeting rendering or visual code is reused. The only place fixture is the synthetic C06 golden metadata: its virtual landmark is not a real venue and cannot authorize physical sharing.

| Contract input | SHA-256 |
| --- | --- |
| `MCR_IMPLEMENTATION_PLAN_V2_EN.md` | `e19732ac9c5c5d5397652bf047645da0640ec72c97f59869974680c4e7f69a36` |
| `contracts/assets/v1/map-manifest.schema.json` | `ed01d26b2f561c9885ad3fc5fe66912ee8175334393e49a85652756e301d6800` |
| `contracts/assets/v1/common.schema.json` | `b7be87ce387dd286bcd2ea895da04f1a8b886c1ea3dc35494f761ac05aad75b4` |
| `contracts/fixtures/assets/map-manifest.json` | `0cccde2c009a44cc2355dfa15ef2d030bc7b85be6f58bec7470108dba46562f7` |
| `contracts/core/v1/core.schema.json` | `9ddf3cd3c853ef6a8258a6215c546bd84eccde620ee73b636888d868cfca1c88` |

These inputs are read-only; this document does not claim C06 frozen status.

## Domain behavior

`concert-domain::place` defines canonical non-nil lowercase UUID place identity, closed C06 enums, bounded text, source/review metadata, geometry, tilemap and immutable release values. Physical places require `PLACE_REFERENCE` provenance and a non-null HTTP(S) source URI; verified physical places additionally require reviewed provenance with reviewer and timestamp. Virtual concepts require `CONCEPT_ONLY`. New physical selection requires a verified, reviewed physical place whose current closure is `OPEN`.

`parse_stored_release` accepts only exact C06 canonical JSON: sorted keys, compact separators, ASCII escaping, integer numbers and no unknown object fields. It validates the closed `MapManifest`, `Place`, `Provenance` and tilemap objects plus geometry dimensions, collision values, unique walkable spawns/interactions, default spawn resolution, interaction place references, four-neighbor reachability from every spawn, place count/identity and the closed provenance rules. It separately checks `geometry_hash` over the canonical `geometry` object.

`CatalogRelease::new` requires its already-decoded manifest to match the exact canonical bytes. Its `content_hash` follows C06: remove only the top-level `content_hash`, canonicalize the remainder, then SHA-256 it. `byte_digest()` is explicitly only a storage/staging byte identity; it is not substituted for C06's semantic content hash. `verify_immutable` distinguishes changed bytes from stale content/geometry hashes. A map identity `(map_id,map_revision)` is immutable; map revision and an event's catalog revision are separate values.

The synthetic fixture proves metadata parsing and C06 hash behavior. It does not prove a real venue, accuracy, photo rights, accessibility or physical arrival. There is no legacy or fallback character image, no real Tiled asset and no photo resource claimed by this task.

## Application and service behavior

The application exposes event-scoped catalog, detail and checked-current physical selection use cases behind an injected `PlaceAuthorization`. Denial or unavailable authority stops the call before persistence; event/operation grants must match exactly. Repository failure and malformed returned data have typed outcomes. The catalog validates that released metadata and current projection have the same closed place identities and immutable fields, while current closure may differ. The selection result must carry the exact requested event, catalog revision, map revision and place ID. Not found, concept-only, unverified, closed, stale catalog and map mismatch remain distinguishable. Selection is a point-in-time checked snapshot with its transaction committed before return; BE09 must recheck closure in the transaction that creates its private beacon, so the lock cannot be treated as an atomic write reservation.

`PlaceSnapshot` carries only durable identities: event, catalog revision, map ID and map revision. It has no invented lease or per-read expiry. Stable reads use PostgreSQL repeatable-read snapshots. The adapter applies only SQL parameters, validates stored canonical bytes and matches every stored hash/identity field before projecting a place. It returns no private location, GPS route, audience or beacon detail. `PgPlaceRepository::select_physical_in_transaction` is the atomic BE09 seam: it checks release identity and current closure inside the caller's transaction and retains SHARE locks until that caller inserts its own record and commits. The existing convenience read wraps this seam with its own transaction and releases locks on return.

Publication is a repository/administrator seam, not a public application authority. A future BE38 admin use case must authorize reviewed publication before invoking it. The adapter serializes first publication of a global map identity and creates or verifies exact immutable release bytes, full release-place membership and initial closure rows, then binds the event and increments its catalog revision atomically. Reusing identical bytes for another current binding increments only the event catalog revision. A same map identity with different bytes, mismatched member rows or a missing event fails with a typed conflict/not-found result. No map revision is rewritten.

## PostgreSQL schema

Migration `0003_places_map_releases.sql` adds four MCR-owned tables:

| Table | Purpose and enforcement |
| --- | --- |
| `concert_map_releases` | Canonical bytes and C06 hashes keyed by immutable `(map_id,map_revision)`; exact-byte update/delete rejection, bounds and hash shape |
| `concert_map_release_places` | Closed, validated release membership and provenance, under the release key; update/delete rejection |
| `concert_events_map_releases` | One current event binding with independent `catalog_revision`, one-way insertion/update checks and foreign key to a real release |
| `concert_place_current_state` | Mutable operator closure separate from immutable release metadata, with exact one-step revision and deletion rejection |

SQL constraints check the closed enums, text bounds, physical/verified provenance relationships, exact provenance key set and type/value relationships. Timestamps are database statements, and both mutable revisions advance by exactly one while remaining monotonic. The database protects state through direct SQL, not only through Rust preflight. It does not independently reparse every canonical geometry or prove release source truth; the adapter does strict domain parsing on every read and publication.

## Validation status

The focused Rust suites and fresh native gate pass from this isolated worktree with Rust `1.88.0`, `--locked --offline -j 2`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` and an isolated target directory:

```sh
cd backend
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/be08/target \
  CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  rustup run 1.88.0 cargo test --locked --offline -j 2 \
    -p concert-domain --test place
# exit 0; 4 passed, 0 failed

CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/be08/target \
  CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  rustup run 1.88.0 cargo test --locked --offline -j 2 \
    -p concert-application --test place
# exit 0; 5 passed, 0 failed

CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/be08/target \
  CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  rustup run 1.88.0 cargo test --locked --offline -j 2 \
    -p concert-service --test places --no-run
# prepared /Users/lhs/Desktop/mcr-build/be08/target/debug/deps/places-d1afcc5efcbf013b

cd ..
python3 backend/scripts/mcr/places_smoke.py \
  --binary /Users/lhs/Desktop/mcr-build/be08/target/debug/deps/places-d1afcc5efcbf013b \
  --report /Users/lhs/Desktop/mcr-build/be08/evidence/native-20261103231508.json
# exit 0; result PASS, native test exit 0, restart PASS, cleanup PASS
```

| Check | Result |
| --- | --- |
| Domain `cargo test --locked --offline -j 2 -p concert-domain --test place` | PASS; 4 passed, 0 failed |
| Application `cargo test --locked --offline -j 2 -p concert-application --test place` | PASS; 5 passed, 0 failed (test-double dead-field warnings only) |
| Native service fixture build, then `places_smoke.py` with one fresh owned cluster | PASS; native test exit 0, restart persistence PASS, cleanup PASS |
| `check-boundaries.py` against `097651c` with the assigned doc/harness allowances | PASS; 12 assigned changes |
| `check-plan.py` | PASS; planning/source preservation only |
| `py_compile`, Python Ruff, Rust `rustfmt 1.88 --check`, and `git diff --check` on assigned paths | PASS |

`places_smoke.py` creates a new marker/token/device/inode-owned PostgreSQL 16.14 cluster with TCP disabled. It applies all three direct SQL migrations; verifies content and geometry hashes and immutable release bytes; reopens and recloses current closure without rewriting bytes; rejects member mutation; exercises exact one-step revision checks; runs the focused native repository test; proves a competing closure update waits behind the transaction seam's SHARE lock until the caller records and commits; confirms independent event bindings and historical release bytes; restarts the cluster; and verifies persisted state. Direct SQL reapplication is **EXPECTED_REFUSED** at the existing schema; it is not SQLx migration-ledger qualification.

The latest passing report is `/Users/lhs/Desktop/mcr-build/be08/evidence/native-20261103231508.json`. It records source candidate `c135bff`, domain source SHA-256 `ee505aa435265ebade6e105d1126e2f059c43c658957dc0e52cee37156507584`, repository SHA-256 `9a6e004fb6d7a54ba43381fef7e13a65da05709ee82edcb6939b7aeff788b660`, migration SHA-256 `b2d4ab07a2addece8ddb4a3e413da2a32605d00ea34082a36f5940d1ff162e7a`, harness SHA-256 `ab0d26ef2274a8d2c39622f83645dab80ac14b4926cc26a21ce647d9f37e684f`, native test output SHA-256 `29b84c7494cd5ac19c63a59398f6624039531a80234bddb29eeb359b2a1c1bd7`, and test binary SHA-256 `555b435db1d595f8131d08bfbff511403b474d1494ba614cb7ec6fd138203429`. The report predates the review-fix commit, so its source digest and revision identify the tested candidate rather than the later documentation-only update.

The database proof is a direct SQL migration and reapply check, not an application SQLx migration ledger. Public HTTP routes, real browser/device use, production deployment, load/soak tests and integrator acceptance remain **NOT_RUN**. No external venue, source-default database or Docker instance was used.

## Changed paths

- `backend/crates/concert-domain/src/lib.rs`
- `backend/crates/concert-domain/src/place/mod.rs`
- `backend/crates/concert-domain/tests/place.rs`
- `backend/crates/concert-application/src/lib.rs`
- `backend/crates/concert-application/src/place/mod.rs`
- `backend/crates/concert-application/tests/place.rs`
- `backend/services/concert-service/src/adapters/out/repository.rs`
- `backend/services/concert-service/src/adapters/out/repository/place.rs`
- `backend/services/concert-service/migrations/0003_places_map_releases.sql`
- `backend/services/concert-service/tests/places.rs`
- `backend/scripts/mcr/places_smoke.py`
- `docs/mcr/backend/places.md`

The root manifests and lockfile are unchanged by BE08; the two dependency-prerequisite commits above are inherited from the parent. Job titles and handoff prose remain English as required by the workspace agreement.
