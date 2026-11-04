# BE-SPATIAL-RELEASE

Status: IN_PROGRESS. This is implementation of the canonical spatial release;
source art is a proposal and is not accepted product artwork.

Current producer: quality-v8, server map revision5, geometry0448, exact frontend
Tiled c589. Source/Python, Rust and private PostgreSQL component qualification
pass; the final committed handoff and integrator publication remain pending.
Existing map3/map4 component qualification
remains intact. The bounded avatar witness goal completed after verified master
publication; the separate v8 delivery remains unfinished. Only the designated delivery
integrator publishes this handoff to master. Current v8 details are at the end;
historical checkpoints below retain their original evidence boundaries.

## Objective and completion criteria

Deliver a source-bound canonical spatial geometry and product-release/bootstrap
admission path for the supplied scene, perform developer-chosen validation,
record provenance and limits, publish the exclusive committed scope directly
non-force to master, and fix concrete failures. Completion requires compatible
source resources and all existing admission requirements, not an alias of missing
skins or invented operator/physical-place qualification.

At this historical checkpoint the goal API reported the incompatible BE04 goal
BLOCKED and unfinished, preserved without reset or false completion. That manual
assignment did not claim a second ACTIVE persistent goal. The later persistent-goal
reconciliation and current v8 assignment supersede that historical goal state.

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
BACKEND was explicitly returned after each frozen compile slice. All scoped native
clusters were cleaned up; no target or database is currently held. No MEDIA or
third target is used. The separate bounded avatar
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

## Initial source-bound candidate (historical)

`build_spatial_release.py` freezes exact published artist bytes, detects checkout
drift, validates the actual C06 MapManifest and geometry semantics, and emits
canonical map bytes plus source/resource pins and explicit readiness. It does not
publish an event-current map or produce an admitted bootstrap by implication.

Initial candidate map: `mcr_reference_scene_r3`, revision 1, content hash
`sha256:d455e77ddeadeaf120fefad4d2eaec6d92d4c0ccc7c091688b5471598776950a`.
Places retain VIRTUAL_CONCEPT / CONCEPT_ONLY and original UNREVIEWED provenance;
the new map provenance is also UNREVIEWED. The missing Tiled resource is
NOT_PROVIDED with null path/hash. No frontend URL is fabricated for backend output.

At that initial checkpoint readiness was NOT_ADMITTED: painted REVENGE and DANGER were unavailable; PARADE is
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

## Virtual-source qualification and revision 3

The original BE08, AR01 and FE19 requirements distinguish virtual concepts from
PHYSICAL meeting places. Each of the four source records (`...0002` lawn,
`...0003` stage, `...0004` gift corner, `...0005` pixel wall) is explicitly
VIRTUAL_CONCEPT / CONCEPT_ONLY, ORIGINAL_ART, UNREVIEWED and OPEN. None establishes
a physical destination; no physical-place reviewer or event layout is needed
for these four virtual-display records. The earlier generic provenance-blocker
wording described an overbroad implementation gate, not an external prerequisite.

The Product API root and spatial owner agreed the typed correction directly:
Product retains ORIGINAL_FIXTURE rejection and all physical-source review guards;
only an actual VIRTUAL_CONCEPT + CONCEPT_ONLY place may retain UNREVIEWED source
metadata. REJECTED, a physical place, an incompatible verification state and an
unreviewed top-level source asset remain denied. Full C06 schema/domain checks
still run first. No shared schema or physical-selection rule changed.

This owner performed actual engineering source qualification of the derived
virtual map: exact published source/Tiled/scenery/resource pins; C06 schemas and
geometry; integer projection and footprints; all-three budgets; unchanged source
places. A separate read-only Pillow integrity/full pixel-decode pass verified all
four unique runtime PNGs against the pinned dimensions. This is source engineering
review by the named backend role, not a fabricated human review, physical venue
verification, artist-fidelity/user acceptance or Product bootstrap admission.

Current map revision **3**:
`sha256:6898e0ee82da23b566ee0482d7dae013780cba7d19a22cc97a47eb0ad13cb920`.
Only derived-map provenance/revision changed from map2. Original geometry/source
and all four nested place values remain unchanged and UNREVIEWED. Map2 content
`30901851...de9f5` is preserved in published `60991c53` history. The exact reviewed
source scope, actor/time, map/geometry/resource source revisions and binding SHA
are in `backend/releases/reference-scene-v2/engineering-source-review.json`,
format `MCR_VIRTUAL_SPATIAL_SOURCE_QUALIFICATION` version1. Source-binding version2
and `from_source_bound_files` signature remain unchanged for the operator owner.

Six Python tests/builder-check and Rust formatting pass. The narrow Rust policy
change and map3 native rerun are NOT_RUN pending a fresh named slot. Existing
map2/avatar qualification above remains valid for its exact source. Current
readiness no longer imposes a generic physical-review blocker on virtual display;
operator bootstrap assembly and actual fidelity acceptance are still open.
Physical beacon/AR01/IN02 requirements remain separately required, with no claim
that this all-virtual source pack satisfies them.

## Required spatial-v3 / quality-v6 candidate — source checkpoint

This required fidelity follow-up uses the artist's published
`bc731ed868ec30b33ae5dd1e6f13e97e68ecabd9` and World's published
`48cf47e9b2c922de42673d1a29c1758a55fcf883`. World remains the sole public
runtime/Tiled writer. Artist owns the proposal/art, and this backend owner owns
only the new `backend/releases/reference-scene-v2/quality-v6/` candidate and
existing assigned source-binding implementation/tests/generator/documentation.
The default map3 artifacts and historical map2 content are preserved.

Generate/check the separate candidate with
`/tmp/mcr-fe14-format-venv/bin/python backend/scripts/mcr/build_spatial_release.py --candidate quality-v6 [--check]`.
Map identity is `mcr_reference_scene_r3`, revision **4**, content hash
`sha256:736afd273a067f6c40db76b8cc54412416fba88984a50d9a6b6738d869831fb8`.
Its new canonical geometry hash is
`sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f`.
The actual Tiled path is
`world/reference-scene-v2/runtime/spatial-v3/canonical-map-v4.json`, 24,058 bytes,
`sha256:60364d5f35bcc8ff82931b7e3a0724a4580880d4a481610e90475d73dcb6ceb9`.
The file contains the exact three new finite 48x81x16 grids. The old geometry
`20168a1d…` cannot be combined with new scenery/ground even though dimensions
remain identical. No independently rounded coordinates are introduced.

| Era | Runtime bytes | Runtime pixels |
| --- | ---: | ---: |
| REVENGE | 1,707,562 | 2,043,904 |
| PARADE | 1,727,221 | 2,043,904 |
| DANGER | 1,712,974 | 2,043,904 |

All three scenes bind their exact quality-v6 manifest, integer projection,
metadata, common source sheets/prompts, per-era source files and runtime images.
Engineering checks cover C06/schema/geometry, all 22 WORLD destinations and
footprints, six separate MAP representations, distinct atlases and per-scene
budgets. A read-only Pillow integrity plus full-decode pass verified the four
unique runtime PNGs against their exact pins. All four original nested places
remain unchanged VIRTUAL_CONCEPT/CONCEPT_ONLY/UNREVIEWED and physically ineligible.
The separate source-review receipt is engineering qualification only.

Source-binding format2 and the existing seven-parameter
`from_source_bound_files` constructor remain compatible. The runtime fence now
requires one pinned layout per scene and compares its geometry hash, unique
object set and original proposal footprints, plus exact integer destinations,
depth and atlas frame, against the supplied scenery. This prevents a binding
assembled from individually valid old/new source files from mixing generations.
Regressions exercise actual old-layout substitution and resealed layout drift.

Nine Python source tests, both exact builder checks, Rust formatting, planning
and diff checks pass (exit0). New Rust tests and both map3/map4 PostgreSQL runs
are **NOT_RUN**, pending a fresh named resource grant. No target or PostgreSQL
lease is held at this source checkpoint. Actual operator bundle/admission and
all-three frontend fidelity acceptance remain open; this candidate is
**NOT_ADMITTED**, with no full Product or multiuser claim.

### Persistent-goal reconciliation

The previous goal's actual objective was the bounded **BE04 canonical identity
adapter increment**: source-grounded design, implementation, relevant validation,
documentation, committed handoff and concrete review fixes. Its original worker
`9fb1c933` and integrated `eba858f2` satisfy that bounded scope. The source review
`docs/mcr/evidence/identity-integration.md` records 30 passing tests, matching
source/contract hashes, exact seven-path ownership and no remaining actionable
finding. The integrated commit is an ancestor of current master. The earlier
worker receipt's PENDING review was superseded by that actual integration review.

The old goal was therefore completed against its own increment criteria, and a
new unbudgeted spatial follow-up goal was created and tool-confirmed active.
This does **not** complete original task BE04 or its real Product admission
criteria. Current public participant/owner-grant composition remains with
Participant209 and the Product API root; their actual owner/admission evidence
must qualify those separate flows. Whole-task and release status are unchanged.

The map4 native case also publishes the old map3 first in the same isolated
database/event, adopts map4, then checks immutable map3 bytes/geometry remain
stored and stale map3 selections fail. The restart case checks exact map4 state.
This authored regression remains NOT_RUN until its frozen-binary qualification.

## Published source-only checkpoint

This publication contains the reproducible map3/map4 source candidates, existing
Python generator/tests and this document only. It deliberately does not deliver
the still-queued Rust provenance-policy/layout/native-test changes. Ten Python
checks and exact reproduction of both candidate directories pass. The native
runner requires the frozen test binary to report the exact actual map identity,
source and art revisions; older binaries without that observation fail closed.
Rust/native map3/map4 qualification remains **NOT_RUN** at this source checkpoint.
The previously published map2 content remains immutable in `60991c53` history;
the default source directory now describes map3, and quality-v6 describes map4.
Consumers may pin these actual source artifacts for separate fixtures without
claiming Product admission, backend runtime qualification or fidelity acceptance.
The backend owner retains the queued Rust implementation and qualification goal.

## Qualified map3/map4 follow-up

Exact tested source is `e3eb56d6cd094bdc4b06643f13fc0ea2a10e2998`.
Rust1.88 locked/offline/j2, dev/test debug0 and incremental0 compiled the service
library, bootstrap runtime config and bootstrap test targets (exit0, 24.50s).
Named MEDIA preflight found 65.3 GiB free and no target handles. Cargo also
reported the normal service executable, which stopped the first overly strict
snapshot selector. The initial release report was corrected immediately. A
named snapshot-only regrant with no intervening holder copied/hash-verified the
three requested test executables, excluding the normal binary; no rebuild or
source change was needed. The target was explicitly released with no handles
before any private PostgreSQL work. Exact JSON, source pins, frozen executables
and logs are external at `/tmp/mcr-spatial-e3eb56d6/`.

Pure tests: library **49**, runtime **13**, bootstrap **5**: **67 PASS** (exit0).
This includes the narrow truthful virtual-place policy and positive all-three
map4 source binding, actual valid-old-layout substitution rejection, and
resealed geometry/footprint/destination/frame/object-set rejection.
Thirteen inherited library native tests and two incumbent non-spatial bootstrap
native tests were not run by this slice. Avatar tests were not repeated because
its previously qualified hook bytes were unchanged.

Both `test_spatial_release.py --native --candidate baseline` and
`--candidate quality-v6`, using the exact frozen bootstrap executable and Cargo
metadata, passed **two tests each** against PostgreSQL16.14. Both runs reported
actual candidate identity/source/art/content/geometry observations, rather than
merely labeling the requested input. Map3 restart was 92082→92303, map4 restart
92460→92557, with marker/socket-owned cleanup PASS for both. Map4 qualification
published map3 first into the same isolated event/database, adopted map4,
verified immutable map3 bytes/geometry persisted, refused stale map3 selections,
and checked exact current map4 bytes after restart. All four virtual concepts
remain physically ineligible. No global or peer database was touched.

The portable `quality-v6/component-qualification.json` pins the exact compiled
Rust file hashes, compiler JSON/executables, native migration hashes, observed
map identities and results. The published source checkpoint's map, binding,
engineering review and readiness bytes remain unchanged.

### Per-era source qualification for derived descriptors

`quality-v6/scene-source-qualification.json` is an additive source-level review
for each exact REVENGE/PARADE/DANGER scenery JSON and its ground/atlas resources.
It pins the same source binding and engineering review, carries the truthful
source-level C06 provenance fields, and states consumer obligations explicitly.
The engineering source reviewer actually checked those bytes, C06/grid/integer
projection/footprints, PNG integrity/decoding, distinct eras and budgets.
Original artist files/provenance and all four virtual place records are preserved.
This packet supplies **no ThemeManifest or Bootstrap** and does not qualify
arbitrary theme/avatar fields. A new descriptor producer must separately validate
its complete C06 body, map/avatar bindings and content hash. FEroot owns its new
TEST_ONLY fixtures; Product API root owns actual operator/theme composition.
No frontend/shared guard was modified or waived by this backend patch.

The additive packet has an independent Python regression. Eleven Python tests,
both exact builder checks, formatting, planning and diff checks pass (exit0).
The unchanged Rust/source candidates retain their exact 67+4 qualification;
the later Python-only packet does not imply a new native run. Quality-v7 remains
a future source handoff and is not silently adopted. The component handoff is
INTEGRATED; actual operator configuration/admission, fidelity/user acceptance and
real multiuser qualification remain open. Readiness remains **NOT_ADMITTED**.

## Quality-v8 producer assignment and revision allocation

The separate avatar witness goal completed after independently verified master
publication `4d5c044fa1b09cfd794a519d963bcd63d22c26d8`. A new unbudgeted
spatial-v8 goal is active in the same isolated owned checkout on
`codex/mcr-be-spatial-v8`, actual base
`0779101878071e89a533f7e47c2dfdef1d1c82b1`.

At that immutable published base the tracked backend releases for map ID
`mcr_reference_scene_r3` contain only revisions3 and4. After this fresh source
collision check, this producer allocates **new server map revision5** to the
new quality-v8 candidate. This is a source identity reservation, not publication
to any database or Product admission. Revision4, geometry899, spatial-v3 and
quality-v6 remain byte for byte frozen; source proposal revision4 is not server
revision4. The future native test must also exercise actual immutable-store
collision handling and adoption from revision4.

Frozen inputs are artist `492160b15465818eca172f4017f3816d58c989a9` and frontend
`cf48c0fe`: normalized geometry
`sha256:0448d5086dc251c6a7aed8c136dbbbf751f95038ea1f7f98e3e40fe6e8067e6f`
was recomputed from the actual source JSON. Exact frontend-owned Tiled resource
`world/reference-scene-v2/runtime/spatial-v4/canonical-map.json` is24058 bytes,
SHA256 `c589b9b01c3e9f7162ccf7443782363bc9c132dd807b5fe15557686df584cf6f`.
The backend consumes those bytes; it does not author another serializer. The
frontend-owned 34-object WORLD and 39-object MAP composition/resource descriptors
remain exact source inputs. At this base the new runtime files exist only in
the frontend handoff commit, so complete current-checkout binding is not yet
qualified. Their publication/materialization must be coordinated without this
worker writing frontend paths.

Exact writable scope: new `backend/releases/reference-scene-v2/quality-v8/**`,
`backend/scripts/mcr/{build_spatial_release.py,test_spatial_release.py}`, spatial
portions of `backend/services/concert-service/src/bootstrap/{spatial,mod}.rs`,
spatial cases in `backend/services/concert-service/tests/{bootstrap,bootstrap_runtime_config}.rs`,
and this document. Root release/schema/registry/composition paths, frontend,
source originals, manifests, lockfiles, migrations, motion C06 families and
selected avatars are excluded. Validation and native publication are NOT_RUN
for this new goal. No build target or database is held.

### Revision5 reproducible source checkpoint

Published frontend inputs were integrated at `4af00fdb` and materialized here by
merge `48451d09`; their original `cf48c0fe` canonical Tiled bytes are unchanged.
This producer consumes them without frontend writes or a second serializer.
The new five-file `quality-v8/` candidate has content hash
`sha256:e22cda95d98f7f5a5ce2f4fba1e78e074248e8b4439e1b5fa2a07c9957f6e539`.
Its raw map SHA256 is
`df4c8d235b408488d0f81711c86f7aa1a838287f8eb26caa56fc28c02c9514fc`
(28854 bytes), raw source-binding SHA256 is
`8413534be5034085d4a0632740c4dc2fcc1e12347443485bfcdea3bc302ffc40`
(10166 bytes), and scene-source qualification SHA256 is
`283d4ed1153a4b0ad83d8b88caa1d78b555a0232d741b65bfe4f5a6438dce331`.

The generator verifies exactly72 newly blocked cells against artist revision
 evidence, with no blocked-cell removal. Terrain, overdraw, spawns, interactions,
four original concept-place records and old fractional footprints are unchanged.
All22 old runtime object destinations match the published **integer v6 layout**;
five illustrative floats in its older proposal are not substituted for those
already published integer runtime values. Exactly12 additional unbound solid
props yield34 WORLD placements in each of three scenes. MAP representations
remain frontend-only and create no backend place IDs or physical authority.
Each era uses two runtime PNGs and2,043,904 pixels: REVENGE1,735,321 bytes,
PARADE1,746,508, DANGER1,732,813. All remain under the existing C06 budgets.
PNG chunk integrity and complete RGB/RGBA pixel streams are decoded by the
retained generator with the standard library; no dependency or budget changes.

Seventeen Python tests pass, including actual old-generation substitution,
resealed geometry/layout evidence, damaged PNG refusal and the frozen executable
selector. That selector treats compiler executable paths as metadata after target
release; a hermetic regression forbids target stat/resolve calls and checks
snapshot drift, missing build-finished and failed Cargo metadata. All three exact
builder checks, Rust1.88 formatting, plan and diff checks pass (exit0). Validation
uses `/tmp/mcr-fe14-format-venv/bin/python`, retaining mandatory JSON-schema
URI/date-time formats; the system Python's missing format extras are not waived.

The new native cases are authored, **NOT_RUN**: revision4→5 adoption; valid
same-ID/revision conflicting bytes rejected without advancing the current event;
old revision4 bytes preserved; stale revision4 selection refused; current5 plus
old4 bytes retained across real postmaster restart. Production source binding is
already generic over34 objects, so this checkpoint changes no production Rust.
Fresh compile/native evidence requires a new named build/PG grant. Source state
is CONTRACT_READY, admission remains NOT_ADMITTED. There is no supplied new
ThemeManifest, Bootstrap, operator switch, art-fidelity acceptance, physical-place
verification, public multiuser proof, or Product/IN01 closure. Original artist
provenance is preserved; the separate engineering packet reviews only exact
source inputs and states complete-descriptor consumer obligations.

### Qualified revision 5 component handoff

The fresh exclusive BACKEND offer was accepted before access. Preflight found
no target handles or active compiler; an unrelated system PostgreSQL process
was left untouched. Rust 1.88 locked/offline, two jobs, dev/test debug 0 and
incremental 0 compiled exactly the service library and the two bootstrap targets.
A single package-only `contracts-build` clean repaired the stale shared source
path (20 files, 816.3 KiB); no broad clean or additional target was used. The first
build of `d5cb7da9` exited 101 with E0412 in the new test's unqualified `Value`.
The one-line qualification as `serde_json::Value`, commit
`4ab22d5fb8f2c30658f1e0e9c0f688e5f5092c4f`, compiled successfully in 3.03 s
(exit 0). The first attempt took 36.34 s; both receipts are retained. The final
compile's minimum free space was 20,318,744,576 bytes, above the 8 GiB floor.

Three exact compiler-selected test executables were copied outside the shared
target and verified against their original hashes. Fresh handle checks were
empty before and after freezing. BACKEND was explicitly released before pure
or PostgreSQL execution and was not accessed afterward. Full Cargo JSON,
source hashes, copied executables, logs and receipts are retained at
`/tmp/mcr-spatial-v8-d5cb7da9/`; its directory name refers to the initial source
checkpoint, while the actual qualified source is the full `4ab22d5f` commit
above. All 1,921 backend/contract input hashes remained unchanged after execution.

The copied service library passed 61 pure tests, runtime configuration 15 and
bootstrap 5: **81 PASS**, exit 0. Thirty-one inherited library native tests and
two incumbent non-spatial bootstrap native tests were not executed by this
spatial slice. The two spatial ignored bootstrap cases were each executed
separately for all three candidates through the retained native harness:

| Candidate | Server revision | Native cases | Postmaster restart | Cleanup |
|---|---:|---:|---|---|
| baseline | 3 | 2 PASS | 5490→5583 | PASS |
| quality-v6 | 4 | 2 PASS | 5813→5922 | PASS |
| quality-v8 | 5 | 2 PASS | 6400→6663 | PASS |

Each private PostgreSQL 16.14 run applied the actual service migrations and
reported the exact observed map/source/art/content/geometry identity. The
revision 5 transaction first published 4, adopted 5, repeated identical 5 without
changing immutable map bytes, rejected a fully valid conflicting same-ID/5
release without advancing the event head, retained 4's original bytes/geometry,
and refused stale 4 selections. Both 5's current state and 4's old canonical bytes
survived an actual postmaster restart. All four concept places remained
physically ineligible. Marker/inode/socket-owned cleanup passed for all clusters;
no peer or system database was attached or modified.

`quality-v8/component-qualification.json` carries the exact compile/source/test
hashes, three binary hashes, native migration checksums, candidate observations,
restart/cleanup receipts and explicit limits. Direct frontend parity also
passed: the 54015-byte `cf48c0fe` source descriptor
`frontend/src/world/reference-scene/spatial-v4/source-inputs.json`, SHA256
`0b686546056ba75591b1d5c32258bd73fd22ca0767686c12aff197d58f8f0beb`,
contains exactly the same three scenery bodies, canonical geometry and original
place records. All 34 WORLD objects agree. Six overview representations replace
one WORLD object and add five, yielding 39 MAP objects; they confer no collision
or place authority. A bounded source peer review found no actionable findings;
its independent decode passed, while its Python environment lacked mandatory
format extras. The owner's retained format-enabled environment passed all 17
source tests and exact generation checks without waiving those formats.

All five original v8 candidate outputs remain byte for byte identical to
`d5cb7da9`; map 3/map 4, spatial-v3/quality-v6, source archives, C06 contracts and
selected-avatar sources remain unchanged. Only the two already-owned spatial
test files changed; production Rust required no new 34-object special case.
This is component qualification, **NOT_ADMITTED**. It supplies no new operator
bundle/Bootstrap/ThemeManifest, art-fidelity acceptance, physical-place
verification, public multiuser proof or Product/IN01 closure. Future art/terrain
versions are not adopted by this handoff. The designated integrator's actual
master publication and independent ancestry/source verification remain required.

Final source checks passed again after native qualification: 17 Python tests,
all three exact generator checks, planning, changed-test-file Rust 1.88 formatting
and diff checks (exit 0). An expanded formatting check over all four potential
Rust seams exited 1 solely for pre-existing import/constructor wrapping in
unchanged `src/bootstrap/mod.rs`; that file's bytes remain exact. This is
recorded explicitly in the component receipt. No formatting or production
behavior changes were bundled to conceal that inherited result.

The compile command was `cargo +1.88.0 test --manifest-path backend/Cargo.toml
--locked --offline -j 2 -p concert-service --lib --test bootstrap_runtime_config
--test bootstrap --no-run --message-format=json-render-diagnostics`. Each native
run used `python backend/scripts/mcr/test_spatial_release.py --native --candidate`
with one of `baseline`, `quality-v6`, `quality-v8`, plus the frozen
`--build-metadata`/`--pinned-binaries` receipts and the PostgreSQL 16.14 `--pg-bin`.
All six exact selected test commands exited 0. Full portable command templates
and all source-check exit codes are in the component receipt; the two compiler
attempts retain their distinct failure/success outcomes.

## Quality-v10 successor: scope and revision reservation

The v8 goal completed after an independent audit of actual remote master
`babbc3f42eec8fa89b71f4c7d036f253c3c45237`: all eleven handoff paths match
`3742056c` byte for byte; five ordered integrated commits have equivalent stable
patch IDs, and old release/source/selected-avatar bytes are preserved. Its
component qualification does not confer Product, physical-place or art acceptance.

Root granted this sequential v10 assignment explicitly. After accepting it, the
same isolated backend checkout starts `codex/mcr-be-spatial-v10` from the master
above, with a new aligned unbudgeted goal. Exact scope is NEW
`backend/releases/reference-scene-v2/quality-v10/**`, additive v10-only handling
in the retained two spatial Python scripts, new spatial cases in the two
bootstrap test files, and this append-only v10 document section. No production
Rust, frontend, contracts, manifests, root composition or old release outputs
are writable. No build or PostgreSQL resource is held.

A fresh recursive scan of every tracked backend release JSON at that published
base found server revisions 3, 4, 5 for `mcr_reference_scene_r3`, with no existing 6
reservation or release. Under this accepted exclusive producer grant, this
committed checkpoint **allocates actual new server map revision 6**, before any
v10 generation. `quality-v10/revision-reservation.json` records the source/ref,
scan coverage, prior identities and exact input pins. This is a source identity
reservation; nothing has been published to a database, event-current pointer,
operator bundle or admitted bootstrap. The artist's proposed 6 and frontend's
minimum 6 were constraints, not prior authority for this allocation.

Frozen artist input is `06562ff1962be525ed929ef431a54bb589860373`, retaining
material source `c7b0cb013e5ab1d9f8cac08be1d0046c249e1cd4`. Frozen frontend input
is `473534296490bbcd4c63a4a0eb85ce7fdd57215a`: finite 48×81, 16-pixel Tiled resource
`world/reference-scene-v2/runtime/spatial-v5/canonical-map.json`, 24058 bytes,
SHA256 `e5837dfe08a8d2f1f7cdb4428b87965b4a7c6ecdb1458b8fe910e3f4c2112078`.
Its three row-major layers have the expected dimensions. Geometry is
`sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395`.
The original frontend binding remains `map_revision:null` and pending allocation
at that immutable source revision; it is not rewritten to forge historical
publication. The three source scenery bodies each contain 34 WORLD objects.
The expected delta is 560 terrain cells and zero collision cells, with old props,
places, spawns and interactions retained. These semantic deltas and all source
pins still require generator/native qualification; reservation alone proves none
of that runtime behavior. Synthetic frontend fixtures 7005 have no production
or canonical-allocation authority.

At reservation time v10 generation, Rust, PostgreSQL adoption/conflict/stale-map
and restart checks are NOT_RUN. Current v8/map5 proof is frozen. V10 remains
NOT_ADMITTED with no new ThemeManifest/Bootstrap or operator switch. Source
materialization proceeds through the frontend owner and designated publisher,
independently of its remaining runtime feature work. No backend-authored Tiled
serializer, implicit C06 family admission, new target or device gate is added.

### V10 source-bound candidate checkpoint

Published frontend sources at `29b1779e` were materialized by backend branch
merge `32a40daec2b8694165d440813aa5c8e107c2bc33`. This merges existing published
work; the backend producer authors no frontend path. The exact original
`47353429` Tiled, frontend binding and generated descriptor bytes all match the
checkout. The immutable reservation is commit
`390bb9a57c2ede32b814e8c36f8d1c85a10a99a2`, reservation SHA256
`771fabbcc0243cff645a2f7588fcaa4d68f056663b2b3ede5d86e2024d213317`.
The retained generator verifies those reservation bytes before v10 generation.

The new map ID remains `mcr_reference_scene_r3`, actual server revision 6,
content hash
`sha256:ac30a80c2c7dd1afdd1e125263403e0b0d4c57941047848a033ec186706c9985`.
Its raw MapManifest is 28854 bytes, SHA256
`34369f4a84e8b27c78bdb62b25bacca8f0db550ab468f76b8a23f8f5dcdf3ac8`;
source binding is 19471 bytes, SHA256
`6c2fa98e26be035e91774de11c0ebad5dd1f56d119b2cadeaf3fd99bb71c7aae`.
The separate per-era source qualification is 8028 bytes, SHA256
`6af6ae4374b1a9109cbe899ad16852bd087ac1a506f7db8b32e52c1be69cddcc`.
This source-level engineering review preserves original artist provenance and
all four original UNREVIEWED concept-place records. It supplies no complete
new ThemeManifest or Bootstrap and accepts no art fidelity or physical venue.

The generator proves exactly 560 row-major terrain changes against the pinned
artist evidence. Every other geometry field, including the entire collision
grid and ID, dimensions, overdraw, spawns, interactions and default spawn, is
unchanged from map 5. All 34 WORLD placements and their fractional footprints
are identical. Eight authored MAP representations replace three WORLD drawings
and add five, yielding 39 displayed MAP objects; they introduce no solid
footprints, backend place IDs or physical authority. The first source-only run
caught an incorrect assumption of 41 MAP objects; the actual frozen descriptor
established the three replacements, and the generator now enforces that value.

Each era retains two runtime PNGs and 2,043,904 pixels. Bytes are REVENGE
1,752,996, PARADE 1,764,183 and DANGER 1,750,488, all within the existing C06
2,097,152-byte/pixel limits. The four unique runtime PNGs are integrity checked
and completely decoded. Native/editable art sources are separately byte-pinned;
that does not assert a pixel decode or visual acceptance of every source file.
Each backend scene carries 24 source pins, below the retained limit of 32.

`frontend-source-compatibility.json` pins the exact 56822-byte frontend generated
descriptor, SHA256
`6a28b3578eb5c5caa317e2078a10166987143d3b05c43441442344db56944e9b`,
and the original 16314-byte source binding, SHA256
`86a5cf9891b77ed31054e4696ade8ae54092d5cbe1f7e8f442643be8b4192244`.
All 44 frontend source input pins and the exact three scenery/runtime descriptor
bodies agree with the artist source and backend binding. The original frontend
binding's null allocation remains historical source data, distinguished from
actual server reservation 6. Any later frontend allocation-metadata publication
must be frozen explicitly as a new input tuple; checkout drift is not waived.

All 22 Python tests pass, covering all old candidates plus new terrain/collision
and prop drift, missing terrain evidence, valid old map 5 Tiled resealing,
existing-revision aliases, actual source descriptor matching and admission limits.
All four exact generation checks pass, including byte-identical old map 3,
map 4 and map 5 outputs. Planning, changed Rust formatting and diff checks pass
(exit 0). Source archives, selected avatars, production Rust, contracts and
lockfiles remain unchanged. The new native cases are authored but NOT_RUN:
map 5→6 publication/adoption, immutable same-ID/revision conflict with no current
head advance, stale map 5 refusal, and exact map 6 plus old map 5 bytes retained
across a real postmaster restart. A fresh named build offer is still required;
no target or database is held. Candidate readiness remains NOT_ADMITTED.
