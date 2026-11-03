# BE08 places and immutable map releases

Date: 2026-11-03. Status: **UNVALIDATED_CANDIDATE / IN_PROGRESS**. This commit is sealed for independent source review while BE04 retains the shared Rust target. Native Rust build/test, native PostgreSQL execution and integrator acceptance remain **NOT_RUN**. The source is frozen for that reviewer until the target is explicitly transferred. No task, IA entry or production route is accepted here.

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

The application exposes event-scoped catalog, detail and checked-current physical selection use cases behind an injected `PlaceAuthorization`. Denial or unavailable authority stops the call before persistence; event/operation grants must match exactly. Repository failure and malformed returned data have typed outcomes. The catalog validates that released metadata and current projection have the same closed place identities and immutable fields, while current closure may differ. The selection result must carry the exact requested event, catalog revision, map revision and place ID. Not found, concept-only, unverified, closed, stale catalog and map mismatch remain distinguishable.

`PlaceSnapshot` carries only durable identities: event, catalog revision, map ID and map revision. It has no invented lease or per-read expiry. Stable reads use PostgreSQL repeatable-read snapshots. The adapter applies only SQL parameters, validates stored canonical bytes and matches every stored hash/identity field before projecting a place. It returns no private location, GPS route, audience or beacon detail.

Publication is a repository/administrator seam, not a public application authority. A future BE38 admin use case must authorize reviewed publication before invoking it. The adapter serializes first publication of a global map identity and creates or verifies exact immutable release bytes, full release-place membership and initial closure rows, then binds the event and increments its catalog revision atomically. Reusing identical bytes for another current binding increments only the event catalog revision. A same map identity with different bytes, mismatched member rows or a missing event fails with a typed conflict/not-found result. No map revision is rewritten.

## PostgreSQL schema

Migration `0003_places_map_releases.sql` adds four MCR-owned tables:

| Table | Purpose and enforcement |
| --- | --- |
| `concert_map_releases` | Canonical bytes and C06 hashes keyed by immutable `(map_id,map_revision)`; exact-byte update/delete rejection, bounds and hash shape |
| `concert_map_release_places` | Closed, validated release membership and provenance, under the release key; update/delete rejection |
| `concert_events_map_releases` | One current event binding with independent `catalog_revision`, one-way insertion/update checks and foreign key to a real release |
| `concert_place_current_state` | Mutable operator closure separate from immutable release metadata, with exact one-step revision and forbidding deletion |

SQL constraints check the closed enums, text bounds, physical/verified provenance relationships, exact provenance key set and type/value relationships. The database protects state through direct SQL, not only through Rust preflight. It does not independently reparse every canonical geometry or prove release source truth; the adapter does strict domain parsing on every read and publication.

## Validation status

Completed static checks in this worktree:

| Command | Result |
| --- | --- |
| `python3 scripts/mcr/check-plan.py` | PASS; planning/source preservation only; 118 tasks, 64 IA entries and exporter regressions |
| `python3 -m py_compile backend/scripts/mcr/places_smoke.py` | PASS |
| Rust 1.88 `rustfmt --check` on the six assigned Rust source/test paths | PASS |
| `git diff --check` | PASS |

The domain/application/service tests in the patch cover C06 parsing and malformed-but-rehashed manifests, hash and immutable-byte behavior, event grant mismatch, stable revision identity, catalog/detail cross-event rejection, every typed selection refusal, independent event bindings, exact release reuse/rebind refusal, concept-only refusal, verified-physical eligibility, closure changes and persisted release bytes. These tests are **NOT_RUN** as Rust binaries until the shared target is explicitly transferred.

The owned-PostgreSQL harness `backend/scripts/mcr/places_smoke.py` creates a fresh marker/token/device/inode-owned cluster with TCP disabled. It applies the three direct SQL migrations, verifies exact C06 content/geometry hashes, release/current-state constraints, event binding isolation, native focused tests, restart persistence and exact cleanup. Direct SQL reapplication is expected to fail at an existing object; this is not SQLx ledger qualification. Docker, a source-default database and external venue calls are not used.

This source-only candidate commit is intentionally not a delivery claim. After explicit target transfer, the required evidence is one focused Rust suite from this worktree with Rust `1.88.0`, `--locked --offline -j 2`, `CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0`, followed by one fresh owned-PG gate. Concrete failures and review findings will be fixed in follow-up commits before final handoff. Until that run and integrator review complete, BE08 remains active, with native Rust/PG, public HTTP, browser/device, load, production and acceptance checks **NOT_RUN**.

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
