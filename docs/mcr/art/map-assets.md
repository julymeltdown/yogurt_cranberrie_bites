# AR02 original virtual courtyard assets

Status: **CONTRACT_READY** geometry candidate, **MOCK_READY** asset review.
No product release, physical venue qualification, AR01 acceptance, QA06 or IN01
acceptance is claimed. Base: `a25b85592b906209f4a9d20f9c7279ced8abab02`;
AR01 source handoff: `03e5f1bdb72c74af1bdfcae5a7392e910b84c799`.
Checkout: `/Users/lhs/.codex/worktrees/a519/mcr`, branch `codex/mcr-art-AR01-AR02`.

## Source and authorship

[AR01 qualification](../venue-provenance.md) and its
[source pack](../../../frontend/public/world/places/source-pack.json) establish
the names and broad diagram-relative Studio-above-Park relationship, plus lawn
character. AR02 uses that limited context in an original virtual courtyard.
All cell extents, paths, boundaries, planting footprints and activity locations
are newly authored. No source photo/map pixels, old rendering, old characters,
unreviewed physical route, or synthetic5×5 fixture has been copied or promoted.

The approved concept image supplies visual inspiration for virtual stage,
gift-corner and mural landmarks. Its warning remains applicable. No stage,
gift pickup point, physical mural, real gate or temporary fence is asserted.
The physical candidate UUIDs are excluded from product metadata because their
closure is unknown; C06 cannot truthfully encode that unknown as OPEN/CLOSED.
Every published candidate place here is explicitly VIRTUAL_CONCEPT/CONCEPT_ONLY
and has a distinct UUID. No physical-candidate alias is issued.

New geometry, metadata and review harness code are authored for MCR, with no
copied third-party code dependencies. The AR04 texture author owns the three
separate PNGs and their source/license evidence. No raster source photo reuse
permission is assumed. Supplied originals and `docs/source/` remain unchanged.

## Delivered format and integration

Files live under `frontend/public/world/maps/`:

| File | Consumer meaning |
| --- | --- |
| `courtyard-tiled.json` | Finite orthogonal32px Tiled-compatible JSON,36×28 cells, exactly terrain/collision/overdraw layers |
| `geometry.json` | C06 normalized terrain/collision/overdraw,3 spawns,5 interactions, one collision identity |
| `virtual-places.json` | Five distinct virtual place records; no real meeting destinations |
| `landmarks.json` | Exact place/interaction/cell associations and supported-vs-authored source trace |
| `binding-inputs.json` | Local hash/dimension/identity inputs, explicitly not a MapManifest or release registry |
| `build.py` | Deterministic original geometry exporter; no HTTP fetching or central manifest changes |
| `verify.py`, `verify-loader.mjs` | C06 and actual FE14 parser checks, including concrete invalid mutations |
| `review.html`, `serve-review.py`, `browser-check.mjs` | Isolated native Canvas asset review using actual AR04 PNGs |

The export is authored by script, not claimed to have been saved in the Tiled
desktop editor. `version: "1.10"` is the frozen parser's format identifier;
`tiledversion: "AR02-authored-v1"` identifies this authored export truthfully.
Opening in a Tiled desktop editor is NOT_RUN. The current application parser
accepts exactly three tile layers and rejects object layers, so spawn/interaction
and landmark data are normalized/sidecar records, not extra runtime Tiled layers.
The collision tile layer stores logical0/1 values; it must not be displayed as
art. Its1 values are blocked,0 walkable; outside-map is blocked. Positive terrain
and overdraw IDs address an AR04 texture, and never grant collision authority.

The fixed texture marker `mcr-theme.png` is inert. The runtime binds the selected
hash-verified theme PNG instead of discovering/fetching an extra image from the
Tiled file. Final map/theme manifests and product pins remain Delivery-owned.
Use one immutable map/geometry/collision identity and exact place/interaction
sets across all three skins. Do not promote the review harness to product entry.

## Shared tile semantics

AR02 and AR04 directly agreed on32×32px cells,256×128px PNGs,8 columns×4 rows,
no spacing/margin and1-based row-major tile IDs. The final256×128 agreement
supersedes crossed earlier128×64 proposals.0 is empty. The frozen identities are:

| ID | Meaning | Collision rule |
| --- | --- | --- |
| 1 | Lawn | Explicit grid only |
| 2 | Paving | Explicit grid only |
| 3 | Alternate paving | Explicit grid only |
| 4 | Gravel | Explicit grid only |
| 5 | Stage deck | Explicit grid only |
| 6 | Roof | Explicit grid only |
| 7 | Wall | Explicit grid only |
| 8 | Water | Explicit grid only; unused in this map |
| 9 | Canopy/shrub | Explicit planting footprint only |
| 10 | Flower | Decorative overdraw |
| 11 | Banner | Reserved decoration; unused |
| 12 | Truss | Decorative overdraw |
| 13 | Sign | Reserved decoration; unused |
| 14 | Lamp | Decorative overdraw |
| 15–32 | Reserved | Transparent/unused |

Layer order is terrain, character foot depth (future product composition),
overdraw. This review includes no character sprites or avatar fallback. The map
offers a connected walkable lawn/courtyard, blocked studio/stage/wall/planting
footprints and a blocked virtual boundary. None is an operational site claim.

## Validation

Commands ran in the isolated checkout; successful commands exit0:

```sh
python3 -m venv /tmp/mcr-ar02-validation
/tmp/mcr-ar02-validation/bin/pip install -r scripts/mcr/requirements-contracts.txt
python3 frontend/public/world/maps/build.py
/tmp/mcr-ar02-validation/bin/python frontend/public/world/maps/verify.py
node frontend/public/world/maps/verify-loader.mjs
python3 scripts/mcr/check-plan.py
python3 frontend/public/world/maps/serve-review.py --theme-root /Users/lhs/.codex/worktrees/1ff9/mcr/frontend/public/world/themes
MCR_AR04_THEME_ROOT=/Users/lhs/.codex/worktrees/1ff9/mcr/frontend/public/world/themes MCR_AR04_CHECKOUT=/Users/lhs/.codex/worktrees/1ff9/mcr node frontend/public/world/maps/browser-check.mjs
```

`verify.py` uses the actual C06 schema/validator on an in-memory unpublished
candidate; it does not write a final MapManifest. It verifies canonical geometry
hash, resource bytes, source-pack hash, exact exported grid parity, source/virtual
identity separation, all five landmark references, all three spawn-to-interaction
paths, boundary/structure blocking and connectivity of all713 walkable cells.
There are1008 total cells and295 blocked cells. Invalid collision drift, blocked
spawn, unknown place, severed route and nonbinary collision are rejected.

`verify-loader.mjs` executes the actual FE14 `validateTiled` and strict JSON
parser transpiled into temporary scratch files, recording source hashes. It
accepts this export and rejects collision drift, an extra object layer, external
tileset discovery, wrong texture dimensions and flipped GIDs. No app source is
modified. Node26.7.0 and TypeScript6.0.3 were the available local execution tools;
this is not qualification of the selected Node24 product toolchain. Python3.14.3
uses the repository's pinned C06 plugin requirements in an isolated temporary
environment. Library modules are read from the existing frontend installation;
no manifest/lockfile or second installation is created in this worktree.

The browser harness uses installed Chrome and Playwright1.61.1 with actual AR04
PNG resources read from that writer's checkout. It checks360/390/430px widths
and all three themes, all five landmark selections, no horizontal overflow,
44px controls,16px labels, nearest-neighbor1:1 close views and actual textured
pixels. The overview is a scaled navigation review, and its numbered legend
retains readable DOM text. Browser version, exact resource hashes and screenshots
are recorded in [browser evidence](evidence/maps/browser-validation.json).
The exact captured PNG bytes were subsequently matched against committed AR04
source `f711c6a4d15e794504026bd39dc77d20c5d21273` using `git show` and hashes.
The report preserves the earlier checkout HEAD observed during capture separately;
that earlier HEAD is not misrepresented as the commit containing the textures.
Geometry results are in [geometry evidence](evidence/maps/geometry-validation.json)
and native parser results in [loader evidence](evidence/maps/loader-validation.json).

The first C06 validation correctly rejected a dotted resource basename that
violated its path pattern; the export now uses `courtyard-tiled.json`. Successful
results above describe the final resource. Resource regeneration is checked for
byte-for-byte reproducibility before handoff.

Visual inspection of the360px Revenge,390px Parade and430px Danger captures
found readable DOM labels, distinguishable lawn/paving/structure areas, matching
landmark positions and no horizontal clipping of controls. This is bounded
desktop asset-review evidence; the other six captures are retained for review.

## Remaining integration and external gates

The BE12 code's cell-center, top-left, row-major0/1 convention was inspected;
the encoded convention is exhaustively checked for these1008 cells. Actual Rust
runtime parity is NOT_RUN: this worker has no Cargo/build lease and creates no
backend target. Delivery/QA must run the final map through the real server and
actual Phaser composition on the same product pins. No frontend rendering source,
server source, schema, lockfile, root or release registry is changed here.

Operator verification, physical route/closure/accessibility qualification, photo
rights, final release binding, actual mobile hardware, QA06, load/real multiuser
and IN01 remain NOT_RUN. These are bounded honest gaps, not scope cuts. This
asset increment does not accept AR01, AR02 or an integration parent. The goal
stays active while required external inputs and concrete delivery fixes remain.

Only Delivery integrates/pushes master. The containing commit's ancestry and
exact changed-path list are sent directly to Delivery and World SL; this branch
does not push or alter acceptance registries.
