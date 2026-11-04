# MAP planting illustration sidecar v1

Status: **CONTRACT_READY source; overall reference fidelity IN_PROGRESS.**

This addresses the sparse central gardens visible in the v10 MAP comparison.
It adds ten small, independently editable planting illustrations around Plaza,
Meetup and the lower lawn. They reuse the original `shrub_bed` frame from each
frozen era atlas. No new image generation, reference/character/legacy pixel use,
recoloring, alpha resealing, texture or font is introduced.

The sidecar binds exclusively to spatial-v5 geometry
`sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395`.
All published source files stay unchanged. In particular, this does not modify
v10, its canonical geometry/projection, WORLD objects, collision/terrain, places,
spawns, interaction cells, cameras, eight MAP representations or six labels.
It is a separate MAP illustration input, not a C06 schema extension.

## Exact source and draw contract

`presentation.json` pins the published v10 source lineage, geometry/projection,
label presentation and per-era scene/layout/native atlas. Each variant also pins
the original generated source atlas, its existing transformation provenance,
source rectangle, nearest packing transform and actual packed frame. The three
eras retain their own artwork. Every original source pixel and alpha is retained
through that recorded lineage; native projects package the unchanged source pair.

Require the exact active geometry hash and matching era atlas hash before using
this sidecar. Read the frame from the pinned layout. Combine its ten `decorations`
with the existing MAP props and eight representations in stable ascending
`depth_y` order. Paint the original label presentation after scenery, followed
by actual runtime markers. Scale all coordinates with the768×1296 scene.
WORLD ignores this entire sidecar. No new PNG/font resource is loaded.

Every decoration has a unique `map_garden_` drawing ID, null interaction and
`MAP_ONLY` view. These are not place, destination or WORLD-object identities.
Do not synthesize collision, solid footprints, hit targets, physical eligibility,
selected state or routes from them. The first five rectangles sit fully within
the semantic green Plaza overlay; the other five are wholly on existing canonical
lawn. They avoid every label rectangle and keep8px from all six named feet.
Visible sprite bases are less than1px above their authored integer draw depths.

The Plaza overlay remains a cartographic gathering-zone illustration, not a claim
that the underlying shared WORLD paving changed to physical lawn. Existing
source-review and release states are not changed by these illustrations.

## Actual verification

`verify.py` checks normalized geometry and frozen pins, all ten bindings/bounds,
every covered pixel's semantic-Plaza/lawn membership, label/foot clearance, exact
original atlas crops and runtime resource hashes/budgets. It checks all three
saved native ZIP compositions and exact packaged ground/native-atlas/font bytes.
Removing the ten new image layers reproduces all93 frozen v10 layers exactly.

For all three native MAP PNGs, **every pixel outside the ten illustration
rectangles equals v10**. Each actual planting changes at least85percent of its
resized original alpha-support pixels by more than5 per channel. This direct
visibility check caught two initially canopy-hidden beds; both were repositioned
before the final exports. It is not a reference-match or critic score.

`check.mjs` passes36 actual Chrome source-composition cases at360/390/430px,
cycling all three eras twice with the planting and frozen-base modes. All216
label checks and216 added-decoration foot samples pass; the exact geometry,
font loading/advance fit, one canvas and no errors/overflow are checked. The foot
checks concern only the added MAP illustrations, not full moving-avatar occlusion.
Nine target/candidate captures are saved. All three final native PNGs and PARADE
comparisons at all three widths were visually inspected. Native viewer-open
returned queued; actual CLI exports provide render evidence. Plan/archive and
scoped whitespace checks pass. No dependency or tool was installed/upgraded.

Each native variant is768×1296 and has two PNGs plus the existing OFL font.
Runtime scenery stays at2,043,904 pixels; combined PNG bytes remain
REVENGE1,752,996 / PARADE1,764,183 / DANGER1,750,488, below existing limits.
No change to the ground PNG or scenery atlas is hidden in this sidecar.

## Delivery and limits

The execution base is `93ef0d895dcc4b247b727e2cccba0bb84576b262`.
`v10-publication.json` records independent verification of v10's mapped published
commits,107 exact payload paths and105 closed Git-byte pins at its named snapshot.
It is historical proof for v10, not proof that this sidecar is published/adopted.

The designated Root153 publisher receives the committed exact-path handoff.
Consumer adoption requires its explicit pinned patch and relevant app/native
checks. Current map5/v8 and v9 bindings remain frozen. This sidecar changes no
canonical geometry and requires no further map revision beyond the actual
backend/consumer binding required for ef52 itself. Art does not allocate, reserve
or admit a server revision. Never infer a revision number from this sidecar.

There is no fourth critic panel or new8.5 score. Historical WORLD6.5/MAP6.0 scores
apply to the earlier frozen checkpoint. Full reference fidelity and concrete
consumer-composition findings remain active. Native/Canvas2D source checks do not
prove application/Phaser/server/source-review/release/device/multiuser/physical
acceptance. No runtime, application, backend, shared, character or archive path
is written by this source patch.

```sh
python3 frontend/public/world/reference-scene-v2/map-landscape-v1/build.py
python3 frontend/public/world/reference-scene-v2/map-landscape-v1/verify.py
# Serve the repository on localhost8896; reuse an existing installed package:
MCR_PLAYWRIGHT_PACKAGE=/absolute/path/to/installed/frontend/package.json node frontend/public/world/reference-scene-v2/map-landscape-v1/check.mjs
```
