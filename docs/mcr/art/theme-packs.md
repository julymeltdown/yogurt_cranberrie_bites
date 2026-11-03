# AR04 — original world skin-pack source handoff

Recorded 2026-11-04, Asia/Seoul. Task status: **IN_PROGRESS** through Delivery
integration and final product qualification. Original source packs and their
bounded source-art validation are delivered; AR04, QA06 and IN01 acceptance are
not claimed. Source review is `UNREVIEWED` in the proposed C06 provenance.

## Ownership and ancestry

- Isolated checkout: `/Users/lhs/.codex/worktrees/1ff9/mcr`.
- Branch: `codex/mcr-art-AR04`.
- Clean detached starting base: `a3bdb38a83f5f220f6b934139da7cb0ad6bb9fae`.
- Source commit: `f711c6a4d15e794504026bd39dc77d20c5d21273`, direct child of that base.
- Evidence/documentation commit: the commit containing this document; its exact
  SHA is reported to Delivery with the final changed-path list. It follows the
  source commit serially. Only Delivery integrates/pushes `julymeltdown/mcr/master`.
- Exclusive paths: `frontend/public/world/themes/**`, this document, and
  `docs/mcr/art/evidence/themes/**`. No other lane, original, contract, registry,
  shared manifest, lockfile, style, root-composition or character path is changed.
- AR01→AR02 map owner: visible chat `01a1a231-0dc4-7b50-92bd-c0f0f56bd970`.
  Its starting map checkout/base are independent. Direct coordination resolved
  crossed proposals explicitly; **256×128 is the final agreed atlas**, superseding
  the unimplemented 128×64 proposal. Map validation against these source pixels
  is owned by that lane and is separate from the geometric art boards here.

## Original pixels and creative source

All 14 used cells per skin are newly authored as integer-grid pixel shapes,
retained as native editable rectangle layers in Tesseract `.tsrct` files and
checked-out JSON. No raster source, font or image is imported into those sources.
The three skins use the same geometry, with confined motif differences inside
the banner: an abstract petal/diamond for REVENGE, marching marks for PARADE,
and a lightning zigzag for DANGER. No characters, skull mascots, venue logos,
band lettering, screenshots, old UI or photographs were sampled or copied.

The supplied inspiration was inspected visually before authoring:
`docs/source/mcr_v2_en/reference/approved_mobile_world.png`, SHA-256
`7abdfbd492216b84317fe31161b65345af5a8e6fa8de7deaed239c49d3db23e1`.
It contains the three broad style concepts and the explicit label
`CONCEPT MAP - VERIFY ON SITE`. Its colors and mood informed the direction;
its pixels, characters, buildings and composition are not extracted. It is not
evidence of real venue geometry, event routes, operational permission or photo
rights. The image remains byte for byte unchanged.

Specification/contract source: `MCR_IMPLEMENTATION_PLAN_V2_EN.md`, especially
§§5.2/5.3/7.4 and AR04, at the stated base; active specification SHA-256
`e19732ac9c5c5d5397652bf047645da0640ec72c97f59869974680c4e7f69a36`.
The source ZIP is `MCR_Developer_Delivery_v2_EN.zip`, SHA-256
`4bf19451105a350acc7bc2f3255aee9514001fe8df904f71cb300c7c6f213964`, as
recorded in the existing provenance. C06 v1.0.0 `ThemeManifest`, texture metadata,
row-major indexing and provenance rules were read, together with the current
`world/theme/loader.ts`, `world/map/resources.ts`, `manifest.ts` and `tiled.ts`.
No schema change is needed for these texture resources.

Creation brief used for native authoring: “Create original compact pixel scenery
for three readable skins over one map: claret/ink/warm stone and deep green for
REVENGE; bone/silver/charcoal and cool sage for PARADE; sunlit green/cyan/yellow
for DANGER. Keep ground quiet, decoration sparse, tile identities and transparent
footprints fixed. Use integer pixels, no characters, copied imagery, real-route
claims, flashing effects or decorative left accent bars.” This was implemented
as the authored geometry in [author.py](evidence/themes/author.py); no remote
image-model generation or image editing was used.

License/provenance: original Codex-authored project artwork, with no external
asset, font, photo or stock license dependency. This is a creation record, not
third-party rights certification or a relicensing of the supplied reference.
The full proposed `ORIGINAL_ART` provenance for each skin is in the source
handoff JSON, explicitly `UNREVIEWED` with null reviewer/time.

## Fixed tile identities and resources

All textures are **256×128 RGBA PNG**, 32×32 cells, 8 columns × 4 rows, zero
margin and spacing. Tile IDs are 1-based in row-major order; 0 is empty.

| ID | Shared identity | Layer use |
|---|---|---|
| 1 | Lawn | terrain |
| 2 | Paving | terrain |
| 3 | Alternate paving | terrain |
| 4 | Gravel | terrain |
| 5 | Stage deck | terrain |
| 6 | Roof | terrain |
| 7 | Wall | terrain |
| 8 | Water | terrain |
| 9 | Shrub/canopy | overdraw |
| 10 | Flowers | overdraw |
| 11 | Banner | overdraw |
| 12 | Truss | overdraw |
| 13 | Landmark sign | overdraw |
| 14 | Lamp | overdraw |
| 15–32 | Reserved, fully transparent | unused |

IDs give visual meaning only. Explicit map collision data controls walkability;
no code may derive it from art or IDs. All decoration pixels remain inside their
single 32×32 cell. The full atlas alpha mask is byte-identical in all themes:
`sha256:efaad4cc17c37192b04edaa12ab3091b87573242fa22c472acb13dc769efd69c`.
No decoration changes room, spawn, interaction, place or avatar meaning.

| Skin | Runtime PNG | Byte SHA-256 | Bytes |
|---|---|---|---:|
| REVENGE | `world/themes/revenge.png` | `245049897f87ace9e82a7e2676654708ac3e4aa8a297d11dedbcb9275c064da6` | 4,975 |
| PARADE | `world/themes/parade.png` | `3ecb77e72c67e1befb1f345f45f12b7b6f931e3ce62c0a43c1b69de319db6c70` | 5,017 |
| DANGER | `world/themes/danger.png` | `1dcea83f14d38a5911f6c1cda6226c365475272cf89832d6a4957a7298343a21` | 4,909 |

The 14,901-byte PNG total is below the local source-art budget of 16 KiB per
texture / 48 KiB for all three. Each decodes to 131,072 RGBA bytes; all three
would occupy 393,216 raw bytes, excluding browser/GPU bookkeeping. These are
source resource measurements, not mobile frame-rate or total-app memory claims.
Editable `.tsrct` and checked-out JSON are review sources, not runtime allowlist
resources. Each rendered atlas uses 20 RGBA values including transparent black.

## Proposed binding inputs for Delivery

[binding-inputs.json](../../../frontend/public/world/themes/binding-inputs.json)
is explicitly `ar04_theme_source_inputs`, **not** a C06 `ThemeManifest`. Its
`texture`, three C06 palette keys and proposed provenance are the concrete
inputs for the final manifests. Full art palettes, source hashes and stable tile
semantics are included as local handoff metadata. No invented map/avatar hashes,
release fixture bindings or synthetic character fallback are present.

Delivery must bind all three themes to the same actual committed `map_id`,
`map_revision`, `map_content_hash`, `geometry_hash`, `collision_id`, exact
`place_ids` and `interaction_ids`, plus the selected product
`avatar_manifest_id`, version and content hash. Then compute each full C06
content hash and the composition-owned release pins/allowlist. Use the inert
`mcr-theme.png` Tiled image marker, or an explicitly permitted pinned path;
actual per-theme PNG resources are loaded through the existing verified loader.
Only the three PNG paths above are proposed runtime texture resources.

No final product `ThemeManifest` or root registry is authored in this scope.
The source `theme_revision: 1` is proposed, not an assertion about an existing
server release. Source JSON has no C06 content hash because it is not a release
document. Registry immutability and all cross-resource final hashes remain
Delivery obligations once real map/avatar sources are committed.

## Validation and review evidence

| Command / activity | Exit / result | Evidence and boundary |
|---|---|---|
| `tsrct --version` | 0; 0.4.0, `d7ccb4bc20fc69f2356c53b80336e52cff31dcc5` | Pinned skill CLI; package checksum verified before install |
| `python3 docs/mcr/art/evidence/themes/author.py --render` | 0 | Native commit + PNG preview for each source; 1,484 / 1,478 / 1,470 editable rects |
| `python3 docs/mcr/art/evidence/themes/verify.py` | 0; PASS | [pixel-validation.json](evidence/themes/pixel-validation.json): C06 texture/provenance/theme-ID source fragments; all authored pixels match rendered PNGs, opposing terrain edges, dimensions, reserved cells, masks, source/byte hashes, budgets, contrast and four negative probes |
| `python3 -m http.server 41864 --bind 127.0.0.1` | Local preview server | Served this checkout for review only; stopped after capture |
| `MCR_PLAYWRIGHT_MODULE=/Users/lhs/Desktop/mcr/frontend/node_modules/@playwright/test node docs/mcr/art/evidence/themes/browser-review.cjs` | 0; PASS | [browser-validation.json](evidence/themes/browser-validation.json): 9 native Chrome captures, 36 theme selections, unchanged art-board tile positions, no horizontal document clipping/page errors, 44px targets, nearest-neighbor 1:1 CSS pixels, 3 exact MIME/status/hash resource reads |
| `python3 scripts/mcr/check-plan.py` | 0; PASS | Planning provenance/exports and 112-task/61-IA accounting only; application checks NOT_RUN |
| `git diff --check` / staged equivalent | 0 | Owned-path whitespace check |

Preview tools were system Node **26.7.0** (standalone art-review script only),
Playwright **1.61.1** and native desktop Chrome **156.0.8078.80**,
at CSS widths 360, 390 and 430, height 844, DPR 1. The installed Playwright
dependency was read from its existing checkout as a tooling library; no files
there were edited and no application module was imported from it. Normal
reproduction uses this checkout's installed `frontend/node_modules/@playwright/test`
without the override. No central dependency or lockfile changes were made.
Pixel verification used Python **3.14.3** and Pillow **12.2.0** for read-only
pixel inspection and in-memory negative probes; final artwork was rendered by
Tesseract, not Pillow. C06 fragment checks use the repository's pinned contract
tools, jsonschema **4.26.0** and referencing **0.37.0**. They do not validate or
register a complete product release. No frontend Node-24 build is claimed by the
standalone preview. The verified Darwin arm64 CLI archive SHA-256 was
`f639b346e1339aca361cff5b893e8e1d260f91c91b7a7421d8e096629cc2f135`.

Contrast ratios for ink over lawn/paving/sign paper/accent/light are,
respectively: REVENGE **3.586/6.574/12.724/3.209/8.954**; PARADE
**3.807/7.298/12.338/10.104/8.464**; DANGER
**4.970/6.824/11.837/8.930/7.692**. These validate the specified outline/label
pairs, not every decorative pixel pair or avatar silhouette. Sign paper/ink
exceeds 4.5:1; the tested scenery outline pairs exceed 3:1. The initial darker
REVENGE accent was brightened before the source commit for the 3:1 outline goal.

All nine rendered boards were visually inspected: at 1:1 mobile scale the ground
patterns stay quiet, shrubs and metal silhouettes remain distinct, and the
three banner motifs are readable in the 2× tile strip. The same camera crop and
art-board tile layout are used across skins. Page wrapping varies with width;
the world canvas and source pixels do not stretch. No decorative left accent
bar or character fixture appears in these previews. These are labeled geometric
art boards, not a product venue map or app screenshot.

| Width | REVENGE | PARADE | DANGER |
|---|---|---|---|
| 360 | [PNG](evidence/themes/revenge-360.png) | [PNG](evidence/themes/parade-360.png) | [PNG](evidence/themes/danger-360.png) |
| 390 | [PNG](evidence/themes/revenge-390.png) | [PNG](evidence/themes/parade-390.png) | [PNG](evidence/themes/danger-390.png) |
| 430 | [PNG](evidence/themes/revenge-430.png) | [PNG](evidence/themes/parade-430.png) | [PNG](evidence/themes/danger-430.png) |

## Outstanding product qualification

Selected-avatar compatibility remains **NOT_RUN**: the separately owned selected
source pack and animated product manifest were not bound or substituted. Actual
map rendering and full C06 bundle/registry binding are separate AR02/Delivery
work. Desktop source-art previews do not establish iOS Safari/Android Chrome,
device DPR, frame rate, startup budgets, UI overlays, real two-account theme
switching, one-game/one-socket behavior or any final QA06/IN01/IN05 gate.

No physical venue, arrival, event route, place verification, map collision or
operational provenance is certified here. This patch makes the texture sources
reviewable and usable for those next gates; it does not change task acceptance
counts or authorize a release. The persistent goal remains active through the
committed handoff and concrete requested delivery fixes. Remote master ancestry
will be verified by Delivery, which has exclusive push authority.
