# C01 source and rewrite inventory

Reviewed 2026-11-03. Delivery is `julymeltdown/mcr/master`. This inventory accepts
the bounded source/rewrite mapping; it does not accept the proposed extracts,
source security, runtime behavior, or redistribution readiness.

## Immutable baselines

| Source | Revision | Mapping and regression evidence |
|---|---|---|
| port-rust | `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2` | Exactly 1,071 files below `boilerplate-grpc-rust/` map to `backend/`; [manifest](backend/import-manifest.json) names every source, destination, Git blob and SHA-256. [Audit](backend/source-audit.md) maps workspace, migration and test obligations. |
| Bigyeting | `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a` | [Frontend inventory](frontend/reuse-inventory.json) maps seven headless candidates to destinations and source regressions, and excludes 422 legacy visual files. [Backend inventory](backend/source-inventory.json) maps policy candidates and their required adaptation/tests. No Bigyeting implementation has been copied. |
| Developer delivery | Original ZIP and upstream narrative | [Intake provenance](source-provenance.json); preserved `docs/source/` is immutable. Active delivery routing is adapted separately. |

The source checkouts were clean at these revisions. The integrator checked every
imported backend byte against the manifest before commit `909e2bf`. The provenance
mechanism is the upstream repository/revision plus per-file Git blobs and hashes,
not an invented shared Git ancestry. Preserve the pinned upstream repositories.
Later adaptations must be separate commits with their own behavior tests.

## Reuse decisions and owner boundaries

- Import the Rust workspace graph, six existing services, six proto source files,
  migrations, exact locks/build configuration and final OpenAPI generator. Retain
  Auth/Profile/Friend/Post/Notification/Gateway ownership. Concert remains a new
  service; no cross-database reads or old identity scheme are authorized.
- Keep `backend/proto/{auth,profile,friend,post,notification,authority}/v1/` at
  its original relative depth. Tonic build scripts resolve this path. OpenAPI's
  index and core pair are both required; generate through the final assembler.
- Retain the 25 selected frontend lockfile resolutions; Phaser 3.90.0 is the only
  new direct dependency. [Lock evidence](evidence/frontend-lockfile.json) records
  the comparison. Newly authored React entry/styles do not reuse legacy markup.
- Headless frontend candidates are review proposals, never blanket permission.
  The [reuse allowlist](reuse-allowlist.json) is empty. A future extract must name
  exact source/destination hashes, reviewed symbols, adaptation and regression
  tests. The C08 gate rejects exact unreviewed copies and legacy static imports.
- Age, introduction, verification and consent candidates remain deferred to
  BE25–BE30. Their source/destination/test mappings are in the backend inventory.
  Null-bound, claim-order and plaintext-handle differences require explicit C04
  decisions and new canonical-identity/crypto/privacy implementations.
- Auth transport is newly adapted to the inherited API, not mechanically renamed
  from `/v1`. The [browser review](frontend/auth-contract-review.md) records exact
  operations and unresolved C03/BE03 behavior.

## Entrypoints and isolated environments

The destination was empty before intake; there was no old executable to delete.
Do not copy Bigyeting entrypoints, visual assets/components, rendering packages,
old member/X-account session handling, source environment files, keys, databases,
uploads, volumes, historical acceptance reports or deployment/reset/seed scripts.
The two imported admin SQL files are compile-time test fixtures, not run targets.
The full exclusion list and exact file whitelist are in the backend manifest.

The [process/port/database map](backend/source-audit.md#isolated-environment-proposal-for-be02)
allocates MCR HTTP 18080–18086, gRPC 51050–51056, metrics 19080–19086 and new
`mcr_*` databases. These are reserved design values, not observed running services;
collision checks and explicit configuration are required before startup. Normal
imported service defaults are not approved MCR launch commands. BE02 qualifies a
fresh private PostgreSQL cluster first; no existing store or volume is attached.
Do not execute data cutover, cleanup or source reset tooling. There is no migration
of production data in this plan.

## Review and limitations

The integrator reviewed the FE and BE inventories separately, source file counts,
hashes, dependency graph and changed paths before serial cherry-picks/import.
Full pinned source and imported Rust workspace builds passed. The new frontend
passed frozen installation, strict build and lint; the final upstream OpenAPI
generator and 22 Python regressions passed. The foundation evidence records exact
commands and distinguishes build from runtime checks.

No tracked source LICENSE/COPYING/NOTICE was found. Internal reuse is explicitly
authorized by the user. Third-party/source redistribution and SBOM qualification
remain release work; this inventory grants no new license. Broad inherited test
suites may reference excluded historical fixtures and need scoped relocation.
No production service, OAuth provider, attendee venue or real-user flow has been
qualified by C01.
