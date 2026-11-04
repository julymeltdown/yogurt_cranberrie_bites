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
BACKEND was explicitly granted and accepted for the source-ready compile/freeze slice;
no native database has started. No MEDIA or third target is used. The separate bounded avatar
transaction hook is coordinated with participant and authority owners, not granted
implicitly by this spatial scope.

Artist source: `8f89d1905a86143d34ff77d2951fc60651ab2497`,
`frontend/public/world/reference-scene-v2/ADOPTION.md`, geometry proposal v2,
painted layout/resources v3. The compact runtime derivative is separately pinned to
`e3af146ae4757794ca153363fd1408300ffde29e`, under `runtime-v4/`; frozen sources remain unchanged. One geometry: 48×81 cells, 16×16 pixels,
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
product bootstrap/operator configuration are missing. The compact PARADE runtime input
is 1,947,817 bytes and 2,043,904 pixels, below existing C06 per-scene limits of
2,097,152 bytes/pixels. The exact artist-owned integer projection and all 22 WORLD
objects are pinned through the closed C06 scenery file. Six MAP representations
remain noncolliding metadata. No schema budget was widened.
Full fidelity and application/server multiuser acceptance remain open.

## Validation checkpoint

Six source/candidate Python tests, exact builder `--check`, Rust 1.88 formatting
and `git diff --check` passed. Rust compilation and native PostgreSQL tests are
NOT_RUN at this checkpoint. The scoped harness requires an exact frozen Cargo
artifact/receipt, creates its own marker/inode-owned PG16.14 cluster over private
Unix sockets, applies real migrations, runs canonical-byte/virtual-eligibility
checks, restarts the postmaster, verifies persistence, then removes its own cluster.
This is component evidence; it does not activate an operator release.

### First frozen runtime qualification

Source `486253900a710fa5357ac7a67185a6ae4ac0d500` compiled with Rust1.88
locked/offline, two jobs, debug0/incremental0 on the explicitly leased BACKEND
target. A stale peer `contracts-build` cache initially produced a protobuf path
shadow error; package-only cache removal (816 KiB), with no source edit or global
cleanup, resolved it. Frozen library/runtime/bootstrap executables passed
38 + 8 + 5 pure tests. Thirteen library native cases and two incumbent bootstrap
native cases were not run by this slice.

The first scoped PostgreSQL run failed an authored test oracle: the existing
administrative publication API advances the event catalog revision even when
immutable map bytes are identical. Cleanup passed. Source `56e6e4ed` corrects the
assertion to require an incremented event revision plus identical canonical map
bytes, then uses the current event revision for virtual-place selection checks.
No place repository behavior was changed. A fresh named compile lease is queued;
BACKEND was explicitly returned after freezing, and no target access is retained.

### Published Tiled input and candidate revision 2

World's published `aa3e2f3d67afc1f288aee63f994dc829fcc81e7b` supplies
`world/reference-scene-v2/runtime/canonical-map-v2.json`, 24,058 bytes,
`sha256:5436a99d36c9b91a4c59302ee65130f327214e4ca78981620759846bb40e919f`.
The source-bound loader verifies actual bytes and exact finite canonical grids;
`mcr-theme.png` is an inert marker, never a resource request. The compatibility
texture dimensions are 768x1296 for each skin. Static ownership remains with the
World developer under the explicitly transferred NEW `runtime/**` prefix.

Current canonical map `mcr_reference_scene_r3` revision **2** has content hash
`sha256:3090185136d5e22138318831d230a7ce6581866a12dc3309df8ebf37825de9f5`.
Its geometry/places are identical to revision 1. The revision-1 checkpoint above
is historical and remains recoverable from `fcc03a8f`; its content is not resealed
under the same identity. Current readiness no longer lists the Tiled gap. All
three era art inputs, genuine nested place/source provenance review and operator
bundle configuration remain genuine admission prerequisites. Announced unpublished
art is not treated as a delivered input. Product state remains NOT_ADMITTED.

The separately assigned operator-bundle owner is Product API root; provenance
review/source ownership was escalated as a project decision. Neither an art schema
check nor component/native database evidence silently upgrades REVIEWED metadata.

### All-three source binding v2

The subsequent frozen artist handoff is
`023201343a452b859759e02039d367c11d1ff8ad`, with three distinct sources under
`world/reference-scene-v2/eras-v5/`. Its manifest is 6,138 bytes,
`sha256:65d3d8ecbdd7263b343a492aa78cb0ccb6422af50d17398571fef78be8a9dfd4`.
The source-binding format is now version **2**, with exactly named per-theme
`scenes` records and exact scenery/source/resource pins. Product admission requires
all three names, exact per-theme source scenery, actual PNG signature/IHDR dimensions
and bytes/hash, and the existing complete Product policy. Duplicate/aliased scenes
fail closed. Read-only candidate inspection remains separate from admission.

| Era | Runtime bytes | Runtime pixels |
| --- | ---: | ---: |
| REVENGE | 1,820,481 | 2,043,904 |
| PARADE | 1,839,826 | 2,043,904 |
| DANGER | 1,819,166 | 2,043,904 |

The 2,097,152 byte/pixel limits apply independently to each scene. The candidate's
aggregate counters sum all three scenes, including the same frozen ground once
per scene; they are not a new aggregate cap or measured network transfer. Source
atlases/provenance/prompts are pinned as source files, not runtime resources.
All 22 integer WORLD destinations and footprints match across skins; six MAP
representations remain separate. The original v3/v4 bytes are preserved; selecting
the new optional v5 PARADE is an explicit source pin change, not a mutation of v4.
The canonical map revision-2 hash above stays unchanged because its geometry,
places and actual Tiled input did not change. Existing FE World v4 qualification
does not qualify adoption of this v5 artwork; consumers must update explicitly.

Six Python source tests and exact builder check pass for these all-three inputs.
Rust compilation/native reruns remain pending a fresh named resource grant.
Current readiness accurately removes the missing-era blockers but retains
UNREVIEWED_SOURCE_PROVENANCE, PRODUCT_BOOTSTRAP_NOT_CONFIGURED and
ART_FIDELITY_NOT_ACCEPTED. No source review state was upgraded or invented.

## Qualified all-three component checkpoint

Exact tested source: `6c700f040888cc40592444edf6772e784395f156`.
Rust 1.88 locked/offline, jobs2, debug0/incremental0 compiled the scoped service
library/bootstrap/runtime/avatar targets and application avatar target (exit0).
Exact compiler receipts and five copied/hash-checked executables are under
`/tmp/mcr-spatial-6c700f04/`. BACKEND was explicitly released before native work.
Pure results: library38, runtime10, bootstrap5, service-avatar2 and
application-avatar2 passed (57 total). Native library cases outside this slice
remain NOT_RUN; the two incumbent native bootstrap cases were not rerun here.

`test_spatial_release.py --native --build-metadata /tmp/mcr-spatial-6c700f04/compiler-artifacts.jsonl --pinned-binaries /tmp/mcr-spatial-6c700f04/binary.json --pg-bin /opt/homebrew/opt/postgresql@16/bin`
exited0: PostgreSQL16.14, canonical map2 bytes/geometry/places preserved through
publication and restart, event catalog revision advances as specified, all four
virtual physical-selection attempts denied, wrong map rejected. Native tests2,
restart14911→15701, marker/socket-owned cluster cleanup PASS. Frozen bootstrap
SHA256 `abd3eafa3dfafce5b6a1c33e0205fde85ac5b6a4646158bb4690b6819ac9c379`.
All-three source Python6/builder-check/plan/diff checks passed.

These are source/database/component results. They do not certify actual operator
configuration, full Product admission, FE v5 adoption, art fidelity or a live
multiuser journey. The four places are VIRTUAL_CONCEPT/CONCEPT_ONLY and require
no physical venue-source request for virtual display. The historical recursive
Product check also required their nested provenance to be REVIEWED, which is
broader than BE08/AR01/FE19. The Product API root and this owner agreed a follow-up
narrow typed virtual-place correction; physical places and fixture rejection
retain their guards. Honest map/theme source engineering qualification remains
separate from physical destination authorization and fidelity acceptance.
