# BE05 Concert service and core schema

Date: 2026-11-03. Status: **IN_PROGRESS — worker implementation, verification and committed handoff; integrator review pending**. This delivers the assigned Concert foundation, not the later identity/bootstrap, catalog, realtime, browser, or production gates.

## Ancestry and ownership

- Branch: `codex/mcr-be-BE05`; isolated worktree: `mcr-worktrees/be-concert-core`.
- Base: `f795ed4f0ae6dc760ffd8a63e335d98db595323b`.
- Source/tooling/new package manifests: `8a8a499e1f82fea2bd980a598ad3c11f3465eb02`.
- Separately delegated root workspace/lock patch: `a9c7560952e742927d7b5297cc6d860382ef30fe`. Apply both commits serially before checking the new packages. The first introduces the files; the second adds their workspace membership. This document is a separate subsequent commit.
- Only the integrator publishes reviewed work to `julymeltdown/mcr:master`. This worker has not pushed. No existing Auth/Gateway/Profile implementation, shared platform/migration helper, proto, HTTP/OpenAPI contract, frontend, assets, infrastructure, CI, or original source document changed.
- Three new local packages reuse existing pinned workspace dependencies. Root `Cargo.toml` adds only three members; Cargo generated only the three new local lock entries and their dependency edges. The boundary test compares every prior package entry, dependency edge, workspace dependency declaration, and newly added member against the base.

## Layer and authority behavior

`concert-domain` owns canonical typed `EventId`/`AuthUserId` references, event slug/title/phase, positive revisions, themes/preferences, participant keys and structural avatar metadata references. It imports only existing value/error libraries (`chrono`, `uuid`, `thiserror`). Canonical string parsing refuses provider identifiers, uppercase/compact UUID aliases and malformed IDs. Auth UUIDs are references; there is no copied user credential, provider claim, X prerequisite, client-granted role or second identity store. Event timezone is `Asia/Seoul`.

`concert-application` depends on that domain and explicit `Clock`, `ParticipationPolicy` and `CoreRepository` ports. Every own-participant/own-avatar read and preference mutation asks policy before persistence, using the injected UTC clock. Grants must bind the exact event/user key and operation; denial, wrong scope/operation and dependency outage fail closed. Store failures and invalid data become typed dependency failures. Missing resources, stale revisions, and events that do not accept writes have distinct typed outcomes. Returned private rows must match the requested key; returned mutation revisions must advance exactly one. No framework, transport, database, serialization DTO or service implementation enters these two layers.

`concert-service` composes the PostgreSQL adapter, real UTC clock and **UnavailablePolicy**. BE04 authority is not implemented by a fabricated approval. Native verification proves this assembled application rejects private reads/writes even when participant rows exist. Preference repository writes are a foundation seam for later features, not an exposed product endpoint.

The service exposes only its operational listener: GET `/livez` and GET `/readyz`, both no-store. Readiness uses the retained bounded/cached database probe and draining semantics; dependency outage returns 503 while liveness remains 200. SIGTERM uses retained process shutdown/supervision and Axum draining, then closes the pool. A 200 health response proves the foundation's database health, not product/policy availability. No Concert product HTTP API, RPC, workload credential or WebSocket endpoint is registered.

## Schema and scoped persistence

One forward migration creates three MCR-owned tables, their keys/checks, and a shared revision/scope trigger:

| Table | Relationships and invariants |
| --- | --- |
| `concert_events` | UUID primary key; unique slug matching C03's 1–64 ASCII pattern; title 1–120 Unicode characters with nonblank/no-control content; five known phases; fixed timezone; positive revision and coherent timestamps |
| `concert_participants` | Composite `(event_id,user_id)` primary key; event FK with cascade; canonical Auth UUID reference; explicit theme/boolean preferences; revision/timestamps; no email, exact age, handles, or provider claims |
| `concert_avatar_settings` | One row per event/user, composite FK to that participant, cascade removal; current C06 metadata version, manifest/item/tone identifiers and positive manifest version; nullable accessory; revision/timestamps; no image URLs or character fallback |

Revisions and manifest versions are positive SQL bigint values capped at `9007199254740991` for safe JSON/realtime representation. Domain revision increment checks the bound. Inserts start at revision 1. Every update must advance exactly one revision, preserve scope and creation time, and receives a database update timestamp. Constraints enforce state even through raw SQL; application preflight is not the sole protection. Title nonblank/control rejection and storage integer limits are explicitly recorded persistence validation, not an amended public schema.

The repository queries only its own database and always binds the event/user pair for participant/avatar reads and writes. Preference updates hold a SHARE lock on the event while checking phase, then lock the participant and compare revision in the same transaction. This serializes event closure and competing writes: one same-revision writer commits, the other conflicts; no write can slip between a phase check and closure. Real PostgreSQL tests exercise this race and event-scoped isolation. No cross-service join, automatic participant creation, default avatar, live-data migration or seed row is introduced.

Avatar columns store only the current C06 structural references. They do not certify catalog membership, compatibility, approved source-pack derivatives or production artwork. There is no atlas/preset/image fallback. BE07 must implement catalog qualification and a reviewed metadata revision if the selected source pack needs a new shape; BE06 owns qualified default selection. `owned_test` references exist only inside disposable database tests and never in migrations or production defaults.

## Configuration, migration and integration inputs

- `CONCERT_DATABASE_URL` is mandatory, with an explicit PostgreSQL scheme/host/database path naming `mcr_concert` or `mcr_concert_<lowercase/digits/underscores>` (at most 63 bytes). Incomplete URLs cannot inherit PGDATABASE into a target. URL-supplied server options are rejected; the driver cannot silently select another schema. The same dedicated-target check runs before standalone migration commands. Credentials and driver diagnostics are not echoed by the service boundary; statement logging is disabled.
- `CONCERT_METRICS_ADDR` is mandatory. Local mode binds loopback and may use port 0; the actual operational address is announced safely. Production requires an explicit nonzero port.
- `CONCERT_RUNTIME_MODE` defaults to `production`; only explicit `local` enables local behavior. Unknown/empty/non-Unicode settings fail. `CONCERT_MIGRATION_MODE` defaults to `external`; `startup` is accepted only when explicitly selected in local mode. No source-default database, silent production DDL or auto-seeding exists.
- Standalone `--migrate`, `--verify-migrations`, `--help` reuse `migration-runtime`. No server/provider settings are needed for those commands. Serving verifies exact receipts plus required core constraints and revision triggers before binding. The probe checks selected required objects, not exhaustive semantic schema drift or grants. CLI receipt verification intentionally does not certify physical constraints.
- Runtime database acquire/statement/lock limits are 3s/3s/1s. Readiness retains the separate bounded probe behavior. Native verification uses external migrations; local `startup` configuration is unit-qualified, not separately certified as a deployment flow.
- The retained migration directory convention is the compiled service manifest directory plus `migrations/`. Packaged/container relocation and restricted-role grants remain integrator infrastructure work; no deployable image is claimed here.
- Before BE04/BE06 RPC integration, the integrator must add a distinct Concert workload/caller identity, registered credentials, method/call/deadline policy and explicit authorization edges for the agreed Auth UUID/session and minimal Profile projections. The shared platform currently has no Concert identity. No existing service is impersonated. A reviewed Concert proto/method mapping to the C03 operations is required; this foundation adds no competing wire authority. C05 ticket/world implementation and its live-session checks remain BE10 onward.

## Executed checks

All commands ran in the assigned worktree. Rust uses `+1.88.0`, two jobs, retained resolution/offline cache and `CARGO_PROFILE_DEV_DEBUG=0`. Target and binaries are exclusively under `/Users/lhs/Desktop/mcr-build/be05/`, separate from BE03/BE02. No Docker daemon/global service was started or changed.

| Command | Exit | Evidence |
| --- | --- | --- |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/be05/target CARGO_PROFILE_DEV_DEBUG=0 cargo +1.88.0 check --manifest-path backend/Cargo.toml --offline -j 2 -p concert-domain -p concert-application -p concert-service` | 0 | Initial check generated only the new local lock entries; `check-01.log` |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/be05/target CARGO_PROFILE_DEV_DEBUG=0 cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p concert-domain -p concert-application -p concert-service --lib --test values --test policy --test config --test health` | 0 | `unit-final-02.log`; five domain, four application, two configuration, one health test; empty library harnesses are not additional tests |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/be05/target CARGO_PROFILE_DEV_DEBUG=0 cargo +1.88.0 build --manifest-path backend/Cargo.toml --locked --offline -j 2 -p concert-service` | 0 | `build-final.log`; executable copied into the BE05 bin directory |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/be05/target CARGO_PROFILE_DEV_DEBUG=0 cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p concert-service --test postgres --no-run --message-format=json` | 0 | `postgres-build-final-02.jsonl`/`.log`; emitted test executable copied as `bin/concert-postgres-tests` |
| `python3 -B -m unittest discover -s backend/scripts/mcr -p test_concert_core_smoke.py` | 0 | `python-final.log`; 12 boundary/ownership/failure/redaction/receipt tests |
| `python3 -B backend/scripts/mcr/concert_core_smoke.py --binary /Users/lhs/Desktop/mcr-build/be05/bin/concert-service --repository-test /Users/lhs/Desktop/mcr-build/be05/bin/concert-postgres-tests --report /Users/lhs/Desktop/mcr-build/be05/native-03.json` | 0 | Three actual PostgreSQL repository tests plus executable/migration/outage/restart/cleanup checks; PASS |
| `python3 scripts/mcr/check-plan.py` | 0 | `plan-check.log`; planning/source preservation only |
| `cargo +1.88.0 fmt --manifest-path backend/Cargo.toml -p concert-domain -p concert-application -p concert-service --check`; `git diff --check` | 0 | Scoped formatting/whitespace checks |

The native harness owns a new PostgreSQL `16.14 (Homebrew)` cluster with TCP disabled and a new `mcr_concert` database. Marker/token/inode checks guard destructive operations. The Rust fixture independently validates the private root/marker identity, exact Unix socket/database/role, and server data directory before queries. No existing/global database or service is accepted. Receipts never contain the URL, password, ownership token, private bodies or test process output.

The native result proves: missing/foreign database rejection, external startup refusing an empty schema without creating tables, migration/reapply/exact checksum ledger, actual event/participant uniqueness and FK rejection, value/revision checks, rejection of URL/path values in avatar identifier columns, cascade relationships, same-user separate-event isolation, atomic stale-write rejection, closure/write serialization, fail-closed assembled policy, PostgreSQL restart persistence and stable receipts, database outage readiness/liveness split, readiness recovery and zero-exit graceful service stop.

The damaged-schema negative drops only the owned avatar FK: receipt-only CLI verification still succeeds, while serving rejects the missing constraint. The harness restores the FK before the healthy service run. This is a verified negative oracle, not treating a random startup failure as success. `native-01.json` and `native-02.json` are preserved earlier development passes; only `native-03.json` binds the final configuration/source revision. The final run's 13 recorded command outcomes include five expected failure exits and eight success exits; expected failures are not reported as successful commands.

`cleanup-final.json` freshly verifies the final root is absent, the recorded service PID is absent, and no process command references that owned root. Earlier roots/children were separately checked too. Neither a mock nor this one-service run certifies real users, Auth/Profile integration, public APIs, approved avatar catalogs, multiple processes, browser sessions, realtime, load, devices, PostgreSQL 17, production TLS/grants/images or release acceptance. Those remain NOT_RUN/outside this foundation.

## Provenance and evidence binding

Domain/application/schema/service composition and new harness/tests are newly authored. Existing headless migration/readiness/shutdown logic is reused by linking/importing it, without editing or copying those implementations. The health composition follows the imported Profile executable's operational pattern. The port-rust pin is `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2` under `boilerplate-grpc-rust/`; [source audit](source-audit.md) and its inventory record the original mapping to `backend/`. This task relies on that audited MCR foundation, not a new upstream history import or qualification of historical claims.

| Read-only MCR source at base `f795ed4` | SHA-256 | Use/destination |
| --- | --- | --- |
| `backend/crates/migration-runtime/src/lib.rs` | `926c8a9daa479dd364e51fcf75c874c355b1e8afccae0caf3bace184c6ae6d8b` | linked/imported unchanged; Concert config/main/health or isolated harness |
| `backend/crates/platform/src/readiness.rs` | `ae35c9b0ad45b3eb6e756c692d8f3bdb1cd6bd7ceee5c93b0927cdc85f4f0f7b` | linked/imported unchanged; Concert config/main/health or isolated harness |
| `backend/crates/platform/src/shutdown.rs` | `bcbefbbec54ff68538afcdef01fac6813af2c639be3571478a96894c3e55ccad` | linked/imported unchanged; Concert config/main/health or isolated harness |
| `backend/services/profile-service/src/main.rs` | `cd1b0a9dc2ca301caab9ea5ef2f58aada5939fb035fae4b90da69c9b153325a6` | linked/imported unchanged; Concert config/main/health or isolated harness |
| `backend/scripts/mcr/migration_smoke.py` | `a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8` | linked/imported unchanged; Concert config/main/health or isolated harness |

The final receipt records every new package source/test/SQL/manifest hash and both root Cargo hashes, the executed script/helper, binary and repository-test executable digests, migration SHA-384 ledger manifest and aggregate SHA-256, and these frozen metadata inputs. It ran while HEAD was the base with implementation changes uncommitted; the later source/workspace commits preserve those exact bytes. Post-commit source/input/binary comparison found zero drift. No post-commit rebuild is implied.

| Contract input | SHA-256 |
| --- | --- |
| `contracts/core/v1/core.schema.json` | `9ddf3cd3c853ef6a8258a6215c546bd84eccde620ee73b636888d868cfca1c88` |
| `contracts/assets/v1/avatar-config.schema.json` | `37b9be22cff7cbde882ab144aec3b75b5edc24f70653f97c9dcec14e87c652bd` |
| `contracts/assets/v1/common.schema.json` | `b7be87ce387dd286bcd2ea895da04f1a8b886c1ea3dc35494f761ac05aad75b4` |
| `contracts/realtime/v1/world.schema.json` | `1732e627037fe91533ac6f7fe74e36683ce9b846bfbfc020b3cce298ae84df36` |

Qualified service SHA-256: `645c1073e7aee7c75567efcb7fe7964acc972795794f6c1788599932613e8c0c`. Repository-test executable SHA-256: `ff2253fe3f1f2a9a3c0aa3840ff2359d57910a54b7b940ee476cfa0c5cab4106`. Harness SHA-256: `967f206689b036ec2b9619079508f3f790c20f87d9777180ecfb0a82e4e62c09`. Migration manifest SHA-256: `6a70df4144ac5782c058ae7b05c90f75268d76e62e1112d2fced9c64c46beb2e`.

| Local evidence under `/Users/lhs/Desktop/mcr-build/be05/` | SHA-256 |
| --- | --- |
| `native-03.json` | `616fd5a16ae3ed24c6c14c4ce46ea76a3bebbf24956041c8b5c09776b8c8281b` |
| `unit-final-02.log` | `df468a216e040fddee6cd1f9b3e3d17a1c72990d1630d4636edb576afcdb8d32` |
| `build-final.log` | `048748c04c5e628140062ae02a0e9f52b6bd6af392155a9bfa97e0a7e0e489f2` |
| `postgres-build-final-02.jsonl` | `6f9706008b7515e09a633fa17d16ce06cba52eb84a23c4d36278514b6a258780` |
| `postgres-build-final-02.log` | `7d25cd1de380bb55c91324bbce6c4c91acf47a8baf7f7a0d5cd8f8960ee4bb05` |
| `python-final.log` | `bcec33b1e2feacdbbbb366f02f2fb9a06b407974100716ad6cadd2c096a7c049` |
| `plan-check.log` | `88606f33d640a29811a4cbdc9748fc0f1e16225b81ad7e697deaf3b20c9b86e1` |
| `cleanup-final.json` | `886c6188cc7c62d4ecd255f87d764b7366a9d1462339caba852a5202e0f909eb` |

## Complete worker changed paths

- `backend/Cargo.lock`
- `backend/Cargo.toml`
- `backend/crates/concert-application/Cargo.toml`
- `backend/crates/concert-application/src/lib.rs`
- `backend/crates/concert-application/tests/policy.rs`
- `backend/crates/concert-domain/Cargo.toml`
- `backend/crates/concert-domain/src/lib.rs`
- `backend/crates/concert-domain/tests/values.rs`
- `backend/scripts/mcr/concert_core_smoke.py`
- `backend/scripts/mcr/test_concert_core_smoke.py`
- `backend/services/concert-service/Cargo.toml`
- `backend/services/concert-service/migrations/0001_concert_core.sql`
- `backend/services/concert-service/src/adapters/mod.rs`
- `backend/services/concert-service/src/adapters/out/mod.rs`
- `backend/services/concert-service/src/adapters/out/repository.rs`
- `backend/services/concert-service/src/config/mod.rs`
- `backend/services/concert-service/src/config/schema-probe.sql`
- `backend/services/concert-service/src/health.rs`
- `backend/services/concert-service/src/lib.rs`
- `backend/services/concert-service/src/main.rs`
- `backend/services/concert-service/tests/config.rs`
- `backend/services/concert-service/tests/health.rs`
- `backend/services/concert-service/tests/postgres.rs`
- `docs/mcr/backend/concert-core.md`

The root manifest/lock and exact new harness paths are explicitly delegated exceptions to central ownership, not blanket permission to edit shared tooling. The integrator reviews those paths separately, integrates the three handoff commits serially, and reruns the affected combined checks before declaring delivery. The persistent goal stays active for concrete review fixes.
