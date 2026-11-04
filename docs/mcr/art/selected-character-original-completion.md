# Original AR03 selected-character disposition

Status: **INTEGRATED**. The original AR03 artwork criteria are satisfied by
existing qualified, published deliveries. This continuation found no uncovered
asset, packing, anchor or category requirement and creates no replacement art.
The owner disposition is **ACCEPTED for the original artwork criteria**; live
services, physical devices, full reference parity and release acceptance are
separate obligations. This new disposition is awaiting Root publication.

## Authority, custody and source

The original specification, §6 avatar target and task AR03, requires six presets,
six hairstyles, four eye choices, six outfits, four accessories, selectable skin,
64×64 cells, foot `(32,60)`, four directions and idle/four walk phases. Acceptance
requires no clipping, missing frames or theme-dependent identity changes at
actual mobile scale. FE12 supplies the shared compositor/cache/thumbnail behavior.
No new live-service or device gate is added to that artwork task here.

The original clean `art-runtime` worktree was at `7e07f7328f079991a9aedb7ad83a4b682decb543`.
That branch remains preserved. The continuation uses the same registered worktree,
branch `codex/mcr-art-C06-AR03-source-completion-20261105`, from freshly fetched
published base `827877828684fdd5e6cda56a85fd11f6f526daf7`. No new worktree or chat
was created. Root153 explicitly granted these new, unoccupied prefixes after
the former motion writer returned its asset lease:

- `frontend/src/entities/avatar/derived-assets/selected-character-original-completion/**`
- `frontend/public/world/avatars/derived-v1/selected-character-original-completion/**`
- This document and `docs/mcr/art/evidence/selected-character-original-completion/**`

Only this document and the new evidence report are used. Source originals,
catalog, prior releases, contracts, generated types, application renderers,
manifests, lockfiles, AR04/ART06 and peer worktrees are unchanged. World owns
functional allocation; Root153 publishes directly, without king/SL review or ACK.
The older proposed `c06-source-completion` prefixes were never used.

Current authority remains `mcr_user_characters_20261103`, revision 3, twenty
selected original JPEGs. The source guidance and catalog's historical
`NOT_PREPARED` fields predate later deliveries; those fields do not prove a
current missing-art requirement. Historical six-static delivery alone also
does not satisfy directional or layered requirements.

The current reviewed successor is
`mcr_selected_motion_reviewed_20261104`, revision 2, contract `2.0.0`.
Its manifest content hash is
`sha256:7f2891c81ffa1d5850107118898a7ef9377b222ab8639b3f2c9daf7908cf9018`.
The exact manifest file SHA-256 is
`1342f2b640a8135c2f128713fe1f59adc0b6da8b925a0ac84808b2d8d1ec884d`.
Its shared 1280×1856 atlas contains 576 physical cells, is 470,010 bytes, and has
SHA-256 `1cd8f05af8d76d58243e2f6e143961953b4d4291fdd553df8c8d33aabd700dbd`.
The candidate descriptor remains ineligible; admission comes through the separate
reviewed successor, complete review/resource pins and canonical Auth authority.

## Original criteria and retained evidence

| Original criterion | Responsible-owner disposition | Concrete evidence |
|---|---|---|
| Six complete presets | Satisfied | Reviewed manifest has six closed preset IDs and 120 distinct physical cells. |
| Required layer inventory and skin choices | Satisfied | Eight body/skin variants, six hair, four eyes, six outfits and four accessories. The versioned composition defines cuff masks, semantic skin palettes, body clipping at y42, attachment order and invisible rear eyes. |
| Fixed cells, anchor and all direction/phases | Satisfied | Manifest geometry is 64×64, `(32,60)`, down/left/right/up and idle/four walks. Source front/rear map explicitly to down/up. The compositor assembles 320×256 output sheets; the renderer selects facing/movement frames from the incumbent scene clock. |
| No missing frames or clipping at mobile scale | Satisfied using retained qualification | Art direction evidence covers all 120 frames at 56/64/72px, 360/390/430 widths and three themes. Wardrobe evidence covers 1,176 pairwise compositions; independent seam evidence covers 4,800 cases. Computed coverage is distinct from the pinned owner's visual assessment. |
| Identity independent of theme | Satisfied using retained qualification | Frozen direction/wardrobe reports assert theme identity invariance; the visual receipt binds exact source/atlas/mobile captures and discloses source JPEG fringes. |
| FE12 shared composition and product consumer | Delivered client behavior | Current reviewed registry, Account owner/editor, Profile and World dispatch use the reviewed production owner and shared runtime. Root's actual-main 360/390/430 receipt and later 390 hard-reload/save/natural self-peer frame receipt remain explicitly controlled HTTP/C05 proofs. |

The [source-bound report](evidence/selected-character-original-completion/criterion-disposition.json)
pins the actual manifest, schemas, source catalog, artwork resources, code and
retained reports. It lists the exact criterion-to-evidence paths. In particular:

- `art/evidence/motion-candidates-v1/direction-validation.json` and
  `wardrobe-validation.json` retain the art/mobile results.
- `contracts/avatar-motion-v2/review/visual-review.json` and `review/seams.json`
  bind the actual reviewed pixels, composition/anchor checks and limits.
- `evidence/frontend-combined-adoption/reviewed-round01/reviewed/2026-11-04T15-46-44-984Z-native.json`
  retains Root's actual-main three-width client qualification.
- `evidence/reviewed-persistence-motion/handoff.json` and its pinned native report
  retain Save/hard reload, Profile parity, natural self/peer walking and cleanup.

These relative evidence paths are under `docs/mcr/`. The earlier art publication
receipts `2fb80680`/`145d0e21` and later reviewed/runtime publications are retained
in their original documents. Their old pending/admission statements must be read
in checkpoint order, not used to erase newer results.

Supplied front pixels/source articulation account for 30 preset cells; 90 cells
use previously authored, reviewed directional reconstruction. Existing wardrobe
body/hair/outfit/accessory reconstruction, supplied eye cutouts and selected skin
palettes retain their explicit provenance. This continuation neither generates
art nor reclassifies authored pixels as supplied photographs. It does not claim
a new license grant or independently established third-party rights.

## Verification and remaining ownership

A source-only Node 24.22.2 inspection exited 0: 38 unique resource hashes/lengths
match current reviewed pins; all twenty catalog entries agree with the reviewed
original-source records; the reviewed schema's full const equals the manifest;
the canonical manifest hash matches; every direction has five in-range indices;
all 120 preset indices are unique; required category counts and cell/anchor
geometry match. Twenty-three source/receipt files are pinned in the report.
This is source binding, not a new image/native qualification or a replacement
for the retained visual judgments. No unchanged image suite, capture or build
was repeated.

| This continuation's command | Exit | Scope |
|---|---|---|
| Node 24.22.2 `--input-type=module` source inspection | 0 | Inline read-only canonical/hash/category/frame inspection; results and exact resource pins in the report |
| `python3 scripts/mcr/check-plan.py` | 0, PASS | Planning/source-archive integrity only |
| `git diff --cached --check` | 0 | Scoped documentation whitespace |
| `git diff --name-only --` protected source/contract/runtime paths | 0, empty | No tracked changes in originals, catalog, `docs/source`, contracts, runtime, renderer or App paths |

Comparing current code with Root's qualified `5da64426` shows the art resources,
motion runtime/compositor, renderer and reviewed registry unchanged. Two outer
composition files changed: Profile gained post-media rendering and World gained
a mural factory. Their reviewed-avatar dispatch branches remain intact. This
static comparison is not a fresh execution of those new unrelated features.

No demonstrated asset input blocker, new consumer failure or required central
C06/accessor hunk remains from this intake. Consequently there is no additional
atlas, metadata adapter, test or duplicate packing implementation to publish.
An evidence-driven later defect still goes directly to its implementation owner.

Actual iOS Safari/Android Chrome and applicable full-reference duties remain
with restored QA06 owner `1ab7` under UI533. Current pack/atlas/review and reusable
capture pins were sent directly. Live backend persistence, two-account behavior
and release acceptance retain their functional/integration owners; controlled
HTTP/C05 reports do not certify them. These are not invented missing-art gates.

The aligned original goal still reports its historical **BLOCKED** authorization
metadata. It was not reset or replaced, and authorization no longer prevents
work. Publication of this scoped disposition remains pending with Root153;
no false goal completion is claimed before the delivery obligation is resolved.
