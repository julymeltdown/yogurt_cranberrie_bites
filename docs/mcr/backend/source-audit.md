# C01 backend foundation source audit

Date: 2026-11-03. Worker branch: `codex/mcr-be-foundation`; base:
`69c17cc0e85eb7209e854a56da0f1388072e00b6`.

This is a bounded foundation import proposal for integrator review. Overall C01
remains **IN_PROGRESS**. No application source, Cargo manifest, lockfile, contract,
infrastructure or source checkout was changed by this worker. No push was made.
The full introduction/age/privacy extraction audit and task acceptance are
deferred to the explicitly named work below. This report does not amend product
scope or accept BE01, C02, or a runtime flow.

## Pinned sources and provenance

| Source | Exact inspected commit | Selected scope |
|---|---|---|
| `julymeltdown/port-rust` | `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2` | `boilerplate-grpc-rust/` foundation |
| `julymeltdown/bigyeting` | `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a` | Read-only policy reconnaissance; no initial import |

Both source checkouts resolved to these SHAs and had clean tracked/untracked Git
status at inspection. The assigned MCR worktree resolved to the base above and
was clean before authoring these two audit files. Root/lane AGENTS, project
assessment, execution plan and relevant specification sections/tasks were read.
The destination is `julymeltdown/mcr`, `master`, through the parent integrator.

Neither source returned a tracked LICENSE/COPYING/NOTICE file in the filename
inventory. The inspected port-rust package manifests omit a license field, and
the generated OpenAPI describes a private repository. User authorization permits
this internal source-reuse task; this inspection establishes no general
redistribution license. Retain source commit/path attribution and resolve source
and third-party redistribution obligations before release. `deny.toml` is an
allowlist, not a completed dependency-license audit. Its advisory exception must
not be silently inherited. Full dependency license/SBOM review is deferred to the
integrator's C01 consolidation/release work.

`source-inventory.json` records the source tree, exact paths, Git blobs and
SHA-256 digests. The parent should preserve the source commit/history as a
reachable source reference or equivalent reviewed provenance mechanism, verify
imported bytes against this inventory, and record its import commit and later
delivered ancestry. This worker has not imported Git history or source code.

## Reviewed import whitelist

**Import only the 1,071 exact files in `import_proposal.files` of the inventory.**
Every entry maps `boilerplate-grpc-rust/<path>` to `backend/<path>` without
flattening. Directory rows below describe the enumerated files; they do not
authorize recursive copies of everything in those directories.

| Source below `boilerplate-grpc-rust/` | Destination | Reason / regression obligation |
|---|---|---|
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml` | Same paths below `backend/` | Preserve workspace graph, exact dependency resolutions and Rust 1.88.0 pin. Integrator owns these files. Run locked workspace build after import. |
| `src/lib.rs` | `backend/src/lib.rs` | Root package is also a Cargo package; retain its dependencies on `api-contracts`, `cutover-cli`, `parity-report`. Its legacy display label is not a filesystem dependency. |
| Enumerated `.rs`/`.toml` files in all 19 `crates/` directories | `backend/crates/…` | Domain/application/platform/contracts/migration/test-support layers. Inventory maps every member and local dependency, including dev/build dependencies. Run affected unit and architecture tests under isolated configuration. |
| Enumerated `.rs`/`.toml`/`.sql` files in all six `services/` directories | `backend/services/…` | Preserve source, service build scripts, tests, migrations, schema probes and runtime-grant SQL. No service is dropped. Build all services; then BE02 migration/restart tests in fresh MCR stores. |
| Enumerated `.rs`/`.toml` files in all four `tools/` directories | `backend/tools/…` | All four are workspace members, including full-stack-e2e; source presence does not authorize running their deployment/data tooling. |
| `proto/{auth,profile,friend,notification,post,authority}/v1/*.proto` (six files) | `backend/proto/…` | Preserve actual source hierarchy. Service build scripts and source-level method-policy assertions depend on these paths. |
| `docs/api/openapi.json`, `docs/api/openapi-core.json` | `backend/docs/api/…` | Index plus core are one contract input pair. Gateway embeds both; api-contracts tests load both. Copying index alone breaks this dependency. |
| `scripts/docs/generate-openapi.py`, `openapi_core.py`, `openapi_bundle.py`, `check-openapi.py`, and eight enumerated `scripts/docs/openapi/*.py` modules | `backend/scripts/docs/…` | Final generator and checker, not core-only assembly. Relative ROOT remains the workspace directory after relocation. Re-run generation and route/payload coverage checks. |
| `scripts/tests/test_generate_openapi.py`, `test_verification_openapi.py`, `test_oauth_form_post_openapi.py` | `backend/scripts/tests/…` | Determinism, split/bundle, verification and form-handler regression tests; 22 source Python tests executed successfully in this audit. |
| `scripts/admin/inspect-post-outbox-dead-letters.sql`, `post-outbox-dead-letter-inspection.sql` | `backend/scripts/admin/…` | Literal compile-time includes in Post tests; preserve as inert SQL source. Do not execute against an existing database. |

The workspace is 29 members: 19 crates, six services and four tools, plus its
root package. The source directory totals reported by the parent are broader
than this selected file set. Retaining all member manifests avoids central
workspace surgery and missing path dependencies. Auth dev-depends on Gateway;
Gateway dev-depends on several sibling services. Even a small package selection
does not justify dropping other member manifests.

Required exclusions:

- Source AGENTS files: retain the parent's authoritative `backend/AGENTS.md`.
- All `.env` files/examples, credential/key material, databases, user uploads,
  volumes, generated build outputs, caches, untracked files and source `.git/`.
- Operational/readiness/release reports, generated evidence and
  `scripts/parity/results/`. Historical success reports are not MCR evidence.
- Deployment/compose/Docker/k3s definitions, reset/seed/data mutation scripts,
  CI composition, observability, scaffold/fuzz/template trees, and other root
  scripts pending separately reviewed relocation. The two whitelisted SQL files
  are included for compile-time tests, not as an operational entrypoint.
- Service READMEs and internal refactoring plans; all other source documentation
  except the two enumerated OpenAPI artifacts. Source architecture rules remain
  review context, not a replacement for target AGENTS.
- `deny.toml` and `.cargo/audit.toml` pending explicit dependency-policy review.
- Every Bigyeting runtime/identity/session module, migration/seed tree, frontend
  rendering file and visual dependency. Policy adaptation is a separate patch.
- Every file absent from the inventory, including newly appearing source files.

## Build, contract and migration topology

The root pins Rust **1.88.0**, edition 2021, with minimal profile plus rustfmt and
clippy. Keep the existing lockfile; declarations are not resolved versions.
Examples inspected in the lockfile: Axum 0.7.9 and 0.8.9, Tokio 1.50.0, Tonic
0.14.6, Prost 0.14.4, SQLx 0.8.6, Redis 0.28.2. Inspection of all 460 cached
registry-package manifests from this lockfile found none declaring a Rust
version above 1.88.0. That is metadata evidence, not a build result.

The parent reports explicit `+1.88.0` works while global stable is broken; use
the explicit toolchain. No Cargo command was run by this worker, to avoid
competing with parent qualification. The parent reports both the locked
`auth-domain` check and the full source `--workspace` build passed, the latter
in 93 seconds. These are parent-reported upstream qualification, not worker-run
checks or imported MCR runtime acceptance. Parent log pointer:
`/Users/lhs/Desktop/mcr-build/source-workspace-build.log`.

Minimal initial C02 smoke:

```sh
cargo +1.88.0 check --locked -p auth-domain
```

That smoke omits Tonic generation and service adapters. A minimal representative
executable build is:

```sh
cargo +1.88.0 build --locked -p auth-service --bin auth-service
```

BE01's actual workspace-build obligation still requires, after import:

```sh
cargo +1.88.0 build --locked --workspace
```

Run from `backend/` using the parent's isolated target directory/build job
limit. Do not run a competing build. Record source-stage and destination-stage
results separately. No test invocation above is a startup authorization.

All six service `build.rs` files use `contracts-build` plus vendored protoc and
Tonic/Prost generation. `contracts-build::proto_root` resolves the calling
service's `CARGO_MANIFEST_DIR/../../proto`; preserving relative depth keeps that
mapping valid. Generated Rust belongs in Cargo OUT_DIR, not an imported evidence
tree. Include the authority proto even where it is inspected through source
assertions rather than a separately compiled service.

No `sqlx::query!`, `query_as!` or `query_scalar!` macros were found in the inspected
service/crate Rust tree. Runtime query APIs and the service-owned migration
directories are present; compile checks do not require a live database on this
evidence. `migration-runtime` loads migration directories at runtime and supports
`--migrate`, `--verify-migrations`, startup and external modes. Retain service
`src/config/schema-probe.sql` files and `runtime-grants.sql` files. Migration
counts are Gateway 17, Auth 12, Profile 7, Friend 14, Post 30, Notification 4.
An imported build cannot prove fresh migration, role grants or restart safety.

The final OpenAPI generator imports its authored Python modules, emits both
index/core artifacts and can bundle them. The checker compares actual Gateway
routes/DTOs. These relocated scripts should be rerun before C03 edits; contract
inputs remain integrator-owned. Older contract-management/deployment scripts
may use Git baselines, historical repo layout or operational files not imported
here; they need BE02/C07 review before use. Broad workspace tests also need a
separate runtime-fixture inventory; importing their Rust files does not certify
that every historical external fixture/tool is present.

## Auth/browser seam for C03 and BE03

Source route prefixes differ: auth lives at `/auth/…`, while much of the content
API uses `/api/…`. The specification's proposed `/api/auth/…` must be mapped
explicitly; do not silently assume it is inherited. The following operation IDs
were read from `docs/api/openapi-core.json` and checked against source routes:

| Source method/path | Operation ID | Destination implementation/test mapping |
|---|---|---|
| POST `/auth/email/signup` | `signupWithEmail` | `backend/services/api-gateway/src/adapters/in/http/auth_routes.rs`; Gateway auth route tests |
| POST `/auth/email/verify` | `verifyEmail` | Same adapter; Auth lifecycle tests |
| POST `/auth/email/check` | `checkEmailAvailability` | Same adapter; do not assume neutral enumeration semantics |
| POST `/auth/login` | `login` | Same adapter; `tests/unit/http_auth_routes.rs` |
| POST `/auth/password/reset/request`, `/confirm` | `requestPasswordReset`, `confirmPasswordReset` | Same adapter; Auth password-reset PostgreSQL tests |
| POST `/auth/refresh` | `refreshAccessToken` | `…/http/refresh_routes.rs`; Gateway cookie/rotation unit tests |
| POST `/auth/logout` | `logout` | `…/http/auth_routes.rs`; cookie clearing/session revocation tests |
| GET `/auth/oauth/{provider}/authorize`, `/callback` | `authorizeOAuth`, `completeOAuthCallback` | `…/http/oauth_routes.rs`; OAuth state tests |
| POST `/auth/oauth/{provider}/callback`, `/callback/complete` | `receiveAppleOAuthFormPost`, `completeAppleOAuthFormPost` | `…/http/oauth_routes/form_post.rs`; form-post unit/browser tests |
| POST `/auth/oauth/{provider}/link` | `linkOAuthIdentity` | OAuth adapter; explicit account-link binding review |

There is no advertised provider-discovery operation in the inspected auth
OpenAPI operations. Likewise, verify/resend and configured provider availability
need a C03 decision; do not claim they exist from route naming. The declared
OAuth provider enum is GOOGLE/APPLE/X/TWITTER, not proof of configured providers.

Login/refresh set an HttpOnly refresh cookie, strip its credential from the JSON
response (the field becomes empty), and return the access token. Cookie code uses
SameSite=Lax, optional Secure and configured Domain; production validation
requires Secure. Browser access-token memory, single-flight refresh and cache
identity boundaries need the new frontend adapter and joint tests.
`auth-service/src/adapters/out/cache/session_store.rs` actually uses PostgreSQL,
with hashed refresh rows, transaction/row-lock rotation, family revocation and
live-session queries. Its directory name must not be mistaken for Redis storage.

CORS origin configuration and OAuth state-cookie checks exist. No request-origin
or CSRF enforcement for ordinary refresh/logout cookie mutations was found in
the inspected Gateway source path; CORS alone does not establish that property.
BE03 must add/qualify it, test invalid origins and stale/reused credentials, and
make safe OAuth return targets explicit. Existing tests were inventoried, not
executed in Rust: `services/api-gateway/tests/unit/http_auth_routes.rs`,
`tests/unit/oauth_form_post.rs`, `tests/integration/gateway_auth_route_matrix.rs`;
Auth `tests/auth_lifecycle_postgres.rs`, `tests/auth_password_reset_postgres.rs`
and `tests/unit/oauth_use_cases.rs`.

Canonical MCR identity remains Auth UUID. `crates/platform/src/grpc_auth.rs`
contains workload authentication and explicit caller policies. Adding Concert
requires integrator-owned identity/caller/contract decisions; a new service
cannot simply impersonate an existing workload. Profile/blocks/deletion/feed
and notification adapters remain owner-service calls, never cross-DB joins.

## Isolated environment proposal for BE02

These are proposed allocations, not observed listeners or configured targets.
Do not start imported binaries with source defaults: startup migration mode can
apply migrations. Explicitly supply new MCR configuration and validate every
database/endpoint before startup. No live database or host listener was inspected.

| Owner | Source default HTTP / gRPC / metrics | Proposed MCR HTTP / gRPC / metrics | New database |
|---|---|---|---|
| Gateway | 8080 / 50050 / 9080 | 18080 / 51050 / 19080 | `mcr_gateway` |
| Auth | 8081 / 50051 / 9081 | 18081 / 51051 / 19081 | `mcr_auth` |
| Profile | 8082 / 50052 / 9082 | 18082 / 51052 / 19082 | `mcr_profile` |
| Friend | 8083 / 50053 / 9083 | 18083 / 51053 / 19083 | `mcr_friend` |
| Notification | 8084 / 50054 / 9084 | 18084 / 51054 / 19084 | `mcr_notification` |
| Post | 8085 / 50055 / 9085 | 18085 / 51055 / 19085 | `mcr_post` |
| Concert (new) | Not present | 18086 / 51056 / 19086 | `mcr_concert` |

Use new `mcr_dev_*` volumes, a dedicated PostgreSQL service (proposed loopback
15432) and isolated Redis (proposed 16379), with newly generated credentials and
namespace. These ports still require parent collision checks. Explicitly set
all six inherited `*_DATABASE_URL` and all `*_HTTP_ADDR`, `*_GRPC_ADDR`,
`*_METRICS_ADDR`, dependent `*_GRPC_ENDPOINT`, migration-mode, JWT and workload
credential settings. Set Post Redis and publisher/Kafka configuration explicitly;
do not infer disabled external delivery is successful outbox delivery. Redis
Pub/Sub alone cannot replace durable domain notifications. Infrastructure and
new Concert variable names are for integrator/BE02/BE05 design.

The initial MCR tree has no old executable backend to delete. Retire references
to source checkout runtime paths when constructing target entrypoints, rather
than deleting source files or data. Never run copied reset/seed/cutover scripts,
attach existing volumes, point to source databases, or migrate old records.
BE02 must create fresh service-owned stores, migrate/verify, restart and prove a
smoke record persists; rollback/forward recovery stays separately reviewable.

## Deferred policy extraction: BE25–BE30

The inventory names concrete candidate-to-destination mappings and original
tests. These are **adaptation proposals**, not an allowlist for copying or an
accepted full policy audit. No Bigyeting code is in the foundation whitelist.

- **BE25 age rules:** `domain/privacy.rs` contains ExactAgeClaim, SeoulDate,
  supported 14–100 claim derivation, next-birthday handling and embedded tests
  for before/on/after birthday, Seoul year change, leap day, malformed input and
  range limits. It also reexports unrelated analytics privacy: extract only the
  age rules into `backend/crates/concert-domain/src/age_policy/`. Dependencies
  are chrono, serde and thiserror; remove the unrelated reexport. The one-birthday
  increment relies on the claim's bounded lifetime, not perpetual validity.
- **BE25/BE26 claim validity:** `adapters/out/repository/current_age.rs` uses the
  Seoul date, verified status, strictly future expiry, required fields and
  supported current age. Canonical ordering is verified_at, expires_at, id
  descending. Its embedded source-guard test is not DB-boundary proof. Adapt
  into Concert pure policy plus explicit claim-repository/clock ports; do not
  copy SQL naming member/naver tables into Auth or join source identity tables.
- **BE25/BE27 introductions:** `application/use_cases/social_intro_policy.rs`
  validates drafts and public contact text with
  `social_intro_contact_policy.rs` and DTO/port dependencies. Its newer age-range
  validator requires both bounds or neither; older `intro_policy.rs` SQL permits
  independently absent bounds. C04 must resolve the required null-bound
  semantics; preserve the distinction rather than copy either silently.
  Contact-filter Korean fallback strings also need newly authored English copy.
- **BE26 verification binding:** `application/use_cases/naver_verification.rs`
  consumes session-bound callback state, checks member binding and derives the
  Seoul date. Adapt to canonical Auth UUID through explicit provider/state/claim
  ports. Provider configuration, callback replay/recovery and replacement
  concurrency still require complete inspection and isolated execution.
- **BE27–BE30 private reads and sharing:** inspected request/reveal SQL is bound
  to old member/social-account identities, database functions and transaction
  helpers. `migrations/0062_provider_neutral_intros.sql` owns important eligibility
  functions, and its canonical claim order differs from the shared helper by
  omitting expiry as a tie-break. Reveal code selects plain handles and writes
  plaintext handle snapshots. Do not port this as MCR encrypted storage or
  mutual, per-platform, version-bound grants. A grant/crypto/revocation design
  and complete dependency/migration-chain review remain necessary.
- **BE28–BE30 privacy:** `domain/privacy.rs` is principally age arithmetic, not
  a complete handle privacy implementation. `domain/analytics_privacy.rs` is
  separate. Block/suspension, public projection, consent, replacement, deletion,
  no-store and subscription/cache invalidation must be audited and tested
  across their actual boundaries. This handoff does not certify those policies.

Original useful test sources include `tests/exact_age_intro_policy_contract.rs`,
`tests/exact_age_eligibility_contract.rs` and support modules,
`tests/intro_private_access_clock_contract.rs`,
`tests/social_intro_naver_gate_contract.rs`,
`tests/social_intro_disclosure_contract.rs` and
`tests/intro_direct_reveal_contract.rs`. Many cases are `#[ignore]` and require
the old PostgreSQL/session harness, source schemas, seeds and fixture documents.
Read their assertions and author isolated MCR fixtures; neither a normal test
invocation nor copying those test names establishes execution. Source seeds and
old databases are excluded. All policy runtime regression checks here are
**NOT_RUN**.

## Commands, evidence and limitations

| Executed by this worker | Exit/result | What it establishes |
|---|---|---|
| `git rev-parse HEAD`, `git status --porcelain` in each checkout | 0; exact revisions and initially clean trees | Source identity/worktree boundary |
| Python TOML/lock/path/include inventory | 0; 29 members, six proto inputs, exact file hashes and local dependency graph | Static foundation topology only |
| `PYTHONDONTWRITEBYTECODE=1 python3 scripts/docs/generate-openapi.py --check` in source workspace | 0; 92 operations, two artifacts | Source contract regeneration |
| `PYTHONDONTWRITEBYTECODE=1 python3 scripts/docs/check-openapi.py` in source workspace | 0; 92 operations | Source Gateway route/payload coverage |
| `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/tests -p test_generate_openapi.py` | 0; 13 tests | Existing generator regressions |
| Same discovery with `test_verification_openapi.py` | 0; 7 tests | Existing verification/bundle regressions |
| Same discovery with `test_oauth_form_post_openapi.py` | 0; 2 tests | Existing form-contract regressions |
| `PYTHONDONTWRITEBYTECODE=1 python3 scripts/mcr/check-plan.py` in assigned MCR worktree | 0; planning PASS | Preserved source hashes, exporter regressions and task/IA accounting only |

The artifact consistency check verifies all proposed source files/hashes, exact
member/path dependency closure and literal compile-time includes. Its recorded
result is in the adjacent JSON verification section. No source operational
evidence was imported. No live database, hardware, user data, provider, Rust
runtime tests, browser, deployment, device or multiuser flow was accessed/run.

C01 consolidation still needs the frontend/headless inventory, full policy
extraction decisions, dependency-license treatment and reviewed overall cutover
map. BE01 import/build, BE02 isolated persistence, C03/C04 contracts, C02 complete
toolchain/environment evidence and all product acceptance remain separate. The
parent must attach the reviewed import commit, actual command logs and delivery
ancestry; the audit commit itself is not the import commit.
