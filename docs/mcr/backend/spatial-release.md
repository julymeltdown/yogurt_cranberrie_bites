# BE-SPATIAL-RELEASE

Status: IN_PROGRESS. This is implementation of the canonical spatial release;
source art is a proposal and is not accepted product artwork.

## Objective and completion criteria

Deliver a source-bound canonical spatial geometry and product-release/bootstrap
admission path for the supplied scene, perform developer-chosen validation,
record provenance and limits, publish the exclusive committed scope directly
non-force to master, and fix concrete failures. Completion requires compatible
source resources and all existing admission requirements, not an alias of missing
skins or invented operator/physical-place qualification.

The actual goal API reports the historical incompatible BE04 goal BLOCKED and
unfinished. It has been preserved without reset or false completion. This manual
assignment does not claim a second ACTIVE persistent goal.

## Ownership and source

Reuse `/Users/lhs/Desktop/mcr-worktrees/be-world-profile-producer`, branch
`codex/mcr-be-spatial-release`, base `8f89d1905a86143d34ff77d2951fc60651ab2497`.
Product API root confirmed the exact scope directly; World confirmed no overlap:

- `backend/services/concert-service/src/bootstrap/{release,schema}.rs`: spatial portions only.
- New `bootstrap/spatial.rs` and necessary additive `bootstrap/mod.rs` export/constructor/projection.
- Spatial cases in `tests/bootstrap.rs` and `tests/bootstrap_runtime_config.rs`.
- `backend/scripts/mcr/{build_spatial_release.py,test_spatial_release.py}`.
- `backend/releases/reference-scene-v2/**`: source-bound candidate/readiness artifacts.
- This document.

No World/config/0026, frontend, art originals, shared schemas/contracts/protos,
manifests/lockfiles, migrations, root bindings or static URL mapping is owned here.
No build slot or native database is currently held. The separate bounded avatar
transaction hook is coordinated with participant and authority owners, not granted
implicitly by this spatial scope.

Artist source: `8f89d1905a86143d34ff77d2951fc60651ab2497`,
`frontend/public/world/reference-scene-v2/ADOPTION.md`, geometry proposal v2,
painted layout/resources v3. One geometry: 48×81 cells, 16×16 pixels,
768×1296 virtual pixels, top-left origin. Canonical geometry hash:
`sha256:20168a1df64cd3c72ea3b8cb576ca895dcff8c7edfb0e77886cc647fb1df37e5`.
The four existing interaction/place UUID pairs and place qualification are copied
unchanged. MAP representations have no collision footprints or new backend IDs.
Virtual movement and illustrative routes establish no physical arrival or safety.

## Source-bound candidate

`build_spatial_release.py` freezes exact published artist bytes, detects checkout
drift, validates the actual C06 MapManifest and geometry semantics, and emits
canonical map bytes plus source/resource pins and explicit readiness. It does not
publish an event-current map or produce an admitted bootstrap by implication.

Candidate map: `mcr_reference_scene_r3`, revision 1, content hash
`sha256:d455e77ddeadeaf120fefad4d2eaec6d92d4c0ccc7c091688b5471598776950a`.
Places retain VIRTUAL_CONCEPT / CONCEPT_ONLY and original UNREVIEWED provenance;
the new map provenance is also UNREVIEWED. The missing Tiled resource is
NOT_PROVIDED with null path/hash. No frontend URL is fabricated for backend output.

Readiness is NOT_ADMITTED: painted REVENGE and DANGER are unavailable; PARADE is
only a source proposal; source provenance is unreviewed; the Tiled resource and
product bootstrap/operator configuration are missing. Actual PARADE runtime input
is 3,179,667 bytes and 2,567,844 pixels, above existing C06 per-scene limits of
2,097,152 bytes/pixels. Authored fractional depth coordinates also need an explicit
artist-owned integer C06 projection. The artist is preparing a versioned bounded
runtime derivative while preserving frozen originals. No schema budget was widened.
Full fidelity and application/server multiuser acceptance remain open.
