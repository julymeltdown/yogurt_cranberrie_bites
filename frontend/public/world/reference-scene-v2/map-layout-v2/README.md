# MAP layout v2: measured label registration

Status: **CONTRACT_READY metadata; new rendering NOT_RUN.** This is an additive
source correction, not a replacement for the published v10 capture packet or a
visual acceptance decision. No raster, font, native project or browser output is
generated here. The frozen editable projects remain the prior composition.

## Concrete finding and correction

The original target's six cream text-panel components are measured in its
248×420 MAP crop, then normalized to the existing 768×1296 coordinates. The old
MAIN STAGE text panel is about87px left/54px above its target center; MEETUP ZONE
is107px too low. `target-measurements.json` records all six comparisons, source
bounds, the pixel-selection rule and limitations. No target pixels enter product
artwork.

| Label | Old nominal anchor | Proposed nominal anchor |
|---|---:|---:|
| MAIN STAGE |335,147|422,201|
| PLAZA |370,461|327,529|
| MERCH |118,750|177,700|
| FOOD |658,758|652,685|
| MEETUP ZONE |372,1024.836257|371,918|
| GIVEAWAY BOOTH |540,1241.157895|518,1159|

The integer rectangles place their measured text-panel centers within0.484
source pixels of those target component centers. This measures registration
only: it does not certify the complete label edges, typography, perspective,
architecture, route topology, WORLD composition or whole-scene fidelity.

The corrected PLAZA rectangle would cover two old garden beds. The coupled
`landscape-presentation.json` moves only those unbound original atlas
illustrations: southwest to240,468,30×20; southeast to405,539,36×23. All1,428
destination pixel centers remain inside the existing semantic PLAZA parcel.
Native visibility and foot/occlusion checks for these placements remain NOT_RUN.

All six label strings, existing/null interaction bindings, rectangle sizes,
font/license, vector icons, era atlases and the other eight garden beds remain
unchanged. There is no additional texture, WORLD object, solid, place, interaction,
canonical geometry revision or selected-state assertion. The exclusive geometry
binding remains `sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395`.

## Explicit consumer handoff

Adopt **both** new sidecars together, pinned by exact bytes/hash. The new label
presentation is also hash-pinned by the planting sidecar; reject an old/mixed
label presentation even though the canonical geometry hash is unchanged.
The new label
anchors and rectangles deliberately override the old display anchors outside
canonical geometry; update the consumer's generated MAP display metadata too.
Use these values for painted hit areas, annotation obstacles and zoom avoidance.
Keep all four current catalog links, authorization, full accessible labels,
closed/unavailable semantics and actual selected-location state.

Preserve the published WORLD camera/props, runtime Tiled bytes, canonical identity,
collision, source review and production denial. This source folder grants no
runtime/application/shared-file ownership. The former v10 owner has returned
its implementation leases; a new exact consumer assignment must be recorded
before anyone edits those paths. No king/SL review or approval gate is requested.

Required follow-up is a matching editable/native composition and actual
360/390/430 MAP comparison in all eras, including garden visibility, label/marker
and short-viewport caption/zoom clearance, interaction/focus and failed-adoption
behavior. The original frozen v10 captures cannot certify this proposed layout.
No artist browser slot or output-heavy generation is assumed.

## Evidence and commands

`consumer-review.json` records the independently verified frozen candidate
`d5c0c9d188f68acb749d7ed2940706f34b2a8be1`, published checkpoint
`12a5d394eeabc89e1c7b0012fb51038f737c9a7f`,49 capture artifacts and105 represented
and published source-pin checks. Its three-width renderer parity and16 root
cases resolve the earlier consumer defects for that input. They do not accept
the new layout or the full reference.

Reproduction (small JSON output only):

```sh
python3 -B frontend/public/world/reference-scene-v2/map-layout-v2/build.py
python3 -B frontend/public/world/reference-scene-v2/map-layout-v2/verify.py
```

Both exit0. `verification.json` records structural bounds, six independent
target-component matches, unchanged drawing/identity inputs, disjoint label and
planting rectangles and destination-pixel parcel containment. There is no new
critic panel/score, source REVIEWED status, physical venue assertion, admission,
live-server, device or multiuser result. Full fidelity remains **IN_PROGRESS**.

Dependencies are the already available Python3.14.3 and Pillow12.2.0 for read-only
target-pixel analysis. No package install or manifest/lockfile change is needed.
