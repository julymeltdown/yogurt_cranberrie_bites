# AR03 selected-source runtime preparation

Status: **IN_PROGRESS**. The current checkpoint delivers six selected-source
transparent front-idle figures, a shared static atlas, a runnable art fixture,
and source/pixel/network evidence. C06 composition, layer choices and animation
remain unfinished. Earlier source-only checkpoints below are historical.

Original assignment base: `f0e44bd1bba943b02319f9d9a42433e253e8de6f` on
`codex/mcr-art-runtime`. Selected authority: `mcr_user_characters_20261103`,
revision 3. The original twenty selected JPEG hashes and byte lengths were
verified unchanged by the report below. `docs/source/` and the source catalog
are unchanged. All review rendering and tooling here is newly authored; no
upstream implementation or visual library was copied.

## Current six-character static release

The human's source-faithful reconstruction instruction, relayed by Main on
2026-11-04, authorizes deterministic provided-pixel cuts and background masks.
The earlier method-authorization blocker is resolved. The recorded goal still
shows its historical BLOCKED state; available goal tools cannot resume it.
Required unfinished work has not been falsely marked complete.

This patch starts at `37e0e0369443c3365c94d8dcaef99eaec06a4038` and introduces
only assigned art assets, preparation tools and evidence. All twenty original
files still match source-pack revision 3 hashes and lengths. The source photo
is `photo_01`, SHA-256
`67448360482a0dd68172d6089378a002190142bb8bd804818c89a86370154311`.
All six complete designs retain their source-baked skin, face, hair and clothes.

The public `world/avatars/derived-v1/static-release.json` follows
`mcr-selected-static-v1`: pack/revision, `front_idle_only`, review attribution,
one atlas and six explicit frames. Its IDs `photo01-front-01` through
`photo01-front-06` identify public static illustrations, never backend inventory
or saved `AvatarConfig` items. It does not change the C06 contract. Frontend
consumer ownership is separate; the frontend owner's filename compatibility
finding was fixed before handoff by using a hyphen before each content hash.

| Resource | Dimensions | Bytes | Role |
|---|---|---|---|
| `static-release.json` | Six frame records | 1,898 | Default metadata |
| `photo01-front-idle-64-e124c88815b22220.png` | 384×64 RGBA | 36,030 | One default texture, six 64×64 cells |
| `photo01-front-idle-256-6258b9f9ee51aed7.png` | 1536×256 RGBA | 368,812 | Optional separate high-resolution texture; not fetched by the default fixture |
| Six `photo01-row*_col*-<hash>.png` cutouts | Native source crops | Per-file lengths/hashes in provenance | Source fidelity and future preparation; not fetched by default |

Default atlas SHA-256:
`e124c88815b22220df2f2dc59e2f26b3ceb845e57b77674fd8e55383116d7efe`.
The [package provenance](../../../frontend/src/entities/avatar/derived-assets/static-package.json)
and [extraction evidence](evidence/static-extraction.json) record every source
rectangle, crop origin, native foot, registration transform and derivative hash.
Initial art payload is 37,928 bytes before HTTP overhead/compression. No source
JPEG, native crop or high-resolution atlas is needed by that default fixture.

`mask-presets.mjs` removes only exterior-connected near-neutral pixels with
minimum RGB 118 and channel spread at most 18, then discards detached JPEG
ringing. Native alpha is binary and retained RGB is unchanged. Uniform nearest
sampling registers a foot at `(32,60)` in each cell; transparent padding is
explicit. An initial wider color tolerance lost pale hair and was rejected
before this release. Its obsolete trial outputs are excluded from delivery.
No redraw, tinting, stretching or synthetic missing frames are used.

Independent decoding of the actual exported PNGs verifies zero retained RGB
mismatches, zero lost dark-outline pixels, intact protected eye/collar/pale-hair
regions and exact nearest-sample atlas registration for all six characters.
A first overly broad white-pixel assertion also counted bright exterior JPEG
ringing. The [validation report](evidence/static-validation.json) records those
92 removed exterior light samples separately and checks reviewed interior
feature regions, rather than claiming every bright background pixel is art.
Retained original light-pixel counts are 538/389/194/286/251/130. A fine beige
JPEG fringe remains visible at native resolution on dark backgrounds; source
RGB fidelity is preserved rather than silently repainting that fringe.

The [native light](evidence/static-native-light.png),
[native dark](evidence/static-native-dark.png), native checker and 390px light,
dark and checker screenshots were visually inspected. All six identities,
pale hair, eyes, collars, ornaments and complete figures are present. The art
fixture shows 64px/72px/192px versions and the common foot marker. Browser
checks at 360/390/430px pass: 18 sprite views, no overflow/page errors, one
descriptor and one atlas request across four surfaces and repeated display
changes. The loopback fixture server gives hashed textures immutable one-year
caching and verifies ETag revalidation with HTTP 304. These are isolated art
fixture results; deployment cache policy and application integration are owned
and validated separately.

Reproduce with Node 24.22.2 and the existing installed Playwright dependency:

```sh
MCR_ART_REVIEWED_AT=2026-11-03T22:38:33Z \
MCR_ART_PLAYWRIGHT=/absolute/installed/frontend/node_modules/@playwright/test/index.mjs \
  node frontend/src/entities/avatar/derived-assets/build-presets.mjs
MCR_ART_PLAYWRIGHT=/absolute/installed/frontend/node_modules/@playwright/test/index.mjs \
  node frontend/src/entities/avatar/derived-assets/verify-static.mjs
node frontend/src/entities/avatar/derived-assets/serve.mjs
# http://127.0.0.1:4193/src/entities/avatar/derived-assets/static-review.html
```

The build and independent verification commands exited 0 with Chrome
156.0.8078.80. Scoped Oxlint on the owned `.mjs` files, `git diff --check`, and
`python3 scripts/mcr/check-plan.py` also exited 0; the last command verifies
planning/source-archive integrity only. No new dependencies, renderer imports, manifest/schema changes,
private API calls or original-image modifications are introduced. Five-layer
composition, selectable skin, four directions/four walking frames, actual
world/profile parity, real mobile devices and multiuser acceptance remain
**NOT_RUN**. AR03 is not accepted by this static release.

## Delivered review geometry

[`source-layout.json`](../../../frontend/src/entities/avatar/derived-assets/source-layout.json)
records six spatial regions on `photo_01` at original 1280×853 coordinates.
These are source-sheet review windows, not validated alpha bounds or persisted
item IDs. Each region contains an initial visually estimated foot point.
[`review.html`](../../../frontend/src/entities/avatar/derived-assets/review.html)
displays those exact original pixels through CSS viewports in 64px and 72px
cells, plus a 3× inspection cell, with the shared local `(32,60)` foot marker.
The beige JPEG background remains visible. No new raster cutout is produced by
the review fixture. Nearest-neighbor scaling preserves the source design for
inspection; these contact-sheet window sizes are not a final pixel-grid design.
Ready status waits for all eighteen displayed images to decode.

The read-only source analysis in `source-registration.json` now measures dark
outline bounds inside every window using a maximum-RGB-channel threshold of
112. No measured dark outline touches a review-window boundary. A shoe-center
estimate from the bottom eight dark-pixel rows differs from the descriptor's
initial visual estimate by at most 5 source pixels horizontally and 2 vertically
(under one pixel at the 64px review scale). The measured locations remain
estimates, not validated alpha bounds, approved masks or actual foot landmarks.
No source pixel is changed and no raster is generated by this analysis.

Surface buttons use the current FE06 cream colors: REVENGE `#eddbd4`, PARADE
`#333239`, DANGER `#d5e6e1`. They keep image URLs, crop geometry, pose and source
skin colors unchanged. This demonstrates stable review selection only; opaque
JPEG backgrounds prevent a transparent-edge/contrast acceptance claim.

## Extraction attempt and precise limitation

The built-in imagegen background-extraction edit was attempted against the
inspected original. Its exact [prompt](evidence/rejected/prompt.txt) and
[rejected PNG](evidence/rejected/imagegen-photo01-r1.png) are retained only in
documentation evidence. The output visibly redraws the hair/fringe contour,
face/eye spacing and collar/tie arrangement instead of retaining source pixels.
It also changes dimensions to 1254×1254. The visual fidelity review rejects it;
the presence of alpha does not make it an approved derivative. It is absent
from `frontend/public/`, the source catalog, every runtime manifest and every
fallback registry.

The read-only Chrome decode reports 909,228 fully transparent pixels and
661,520 partially transparent pixels, with no alpha-255 white sample. Nonzero
alpha reaches x=0 and y=1239. Those counts are measurements of this rejected
candidate, not acceptable edge registration. Recoloring or making every white
pixel transparent would lose intentional highlights/collars and is prohibited.

At the historical source-only checkpoint, deterministic extraction awaited a
human exception to the imagegen editing default. That exception has now been
received and the six cutouts above supersede this blocker. The rejected redraw
remains evidence only and never enters product assets.

## FE12 coordination and composition constraints

FE12 owner `01a1a153-1227-7e72-801f-a2a6b91981e1` confirmed C06 v1 consumes
five independent layers (`body`, `eyes`, `hair`, `outfit`, `accessory`), 64×64
untrimmed cells, foot `(32,60)`, one 320×256 region per item, rows
`down,left,right,up`, and columns idle plus four walk frames. Delivery owner
`01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08` explicitly retained C06 unchanged and
classified front-idle artwork as review/preview only. No production binding or
synthetic backend item ID is approved by this checkpoint.

Direct inspection of the original base sheets confirms these mismatches:

| Source | Actual visible content | Required preparation |
|---|---|---|
| `photo_01` | Six fully composed, front-idle characters | Exact alpha extraction and registration; cannot split by pretending hidden pixels exist |
| `photo_02` | Eight skin variants with eyes, mouth, base clothes and limbs baked in | Separate editable face/clothing or retain an explicitly reviewed whole-body variant; no whole-image tint |
| `photo_04` | Sixteen front hair shapes, including intentional pale panels, clips and ornaments | Isolate each shape; fit landmarks; deliberately preserve pale pixels and separate decorations only where reviewed |
| `photo_05` | Sixteen hair backs with silhouettes and colors that do not establish a one-to-one front pairing | Pair by reviewed design, not sheet position; these do not supply side/back body frames |
| `photo_06` | Sixteen outfits with attached pale hands, lower hems, trousers, jewelry and neck skin in some entries | Deliberate garment/hand/neck masks and baked tone variants; resolve overlap with body and separate bottoms |

Candidate future layer order for review is back hair, body/limbs, bottoms,
top/outfit, face/eyes, front hair, then explicit attachment groups. This is a
proposal only: some front hair occludes eyes, and neckwear/headwear/handhelds
need individual attachment ordering. The selected additional sheets have
different proportions; they must be fitted individually. This checkpoint
does not implement that order or silently extend the five-slot contract.

## Concrete static-preview proposal for later review

Keep the local `selected_source_static_review` descriptor separate from
`AvatarConfig` and `AvatarManifest`. Its present fields pin pack/revision,
original source ID/path/hash, original-space region/estimated foot, display cell
and foot anchor, `front_idle_only`, source-baked skin policy and explicit missing
states. `wire_binding` and `inventory_ids` stay null, and
`production_eligible` stays false.

After an exact cutout exists, a reviewed preview descriptor can additionally
record derivative path/hash, actual alpha bounds, original-to-derivative
transform, checked foot registration and color-preservation evidence. A static
preview renderer must use only the supported front-idle pose, label unavailable
directions/walk states explicitly, preserve the baked face/hand colors and use
the same derivative across themes. That descriptor must remain outside backend
inventory and C06 production entry. Any product static behavior or versioned
layer contract needs an explicit owned implementation decision with actual
assets and producer/consumer checks. This proposal grants no such decision.

No four-direction animation was supplied. Mirroring, stretching, repeating a
front image or treating back hair as a complete rear character would not meet
the four-direction idle/four-walk-frame requirement. No frame aliases or fake
walk rows are delivered. Six hairstyle/four-eye/six-outfit/four-accessory and
selectable-skin production targets remain outstanding, as do actual device and
world/profile parity checks.

## Reproduction and evidence

### Thirty-two hair reference registrations

The World lead assigned the next source-only slice while the extraction-method
question remained pending: `photo_04` front hair and `photo_05` back hair.
[`hair-layout.json`](../../../frontend/src/entities/avatar/derived-assets/hair-layout.json)
records sixteen shapes from each original, exact source hashes, original-space
review rectangles, measured principal dark-outline bounds and unverified
cranium-center estimates. Every front/back pair, composition order and wire
item ID is explicitly null. Alpha and body fit are unprepared/unverified;
direction frames are unknown and walking frames are not provided.

The read-only measurement uses four-neighbor connected dark pixels
(maximum RGB channel 112) and keeps the sixteen principal components above
5,000 pixels on each sheet. Review windows add eight original pixels around
those bounds. This is geometry analysis, not background removal, a pixel mask,
or a claim that every faint/isolated edge pixel is included. The estimated
attachment is the outline's horizontal center at 45% of its height. It has not
been fitted to a face, skull, body or C06 cell. The visible attachment marker at
the center of a 128×128 review window is unrelated to the C06 foot anchor.

The measured outlines cross nominal 320×240 contact-sheet grid boundaries for
five front-hair references (`row1_col1`, `row1_col4`, `row2_col1`, `row2_col4`,
`row4_col4`) and three back-hair references (`row3_col3`, `row3_col4`,
`row4_col2`). That directly confirms the catalog's grid hints cannot be used as
validated sprite rectangles. Per-choice regions preserve the visible supplied
shapes in the review fixture; regular grid crops could truncate them.

[`hair-review.html`](../../../frontend/src/entities/avatar/derived-assets/hair-review.html)
shows each original reference at 64px and 128px review-window sizes. The front
and back sheets are selectable independently, with a persistent warning that
same-position references are unpaired. White JPEG background/face openings and
intentional pale hair and ornaments remain opaque and unchanged. It offers no
composition, recoloring, layer choices or backend save operation. The six
complete-character review remains separate and unchanged by this slice.

Native Chrome 156.0.8078.80 checks pass at 360, 390 and 430px: all 32 spatial
references/64 original-image views decode; each selected sheet shows sixteen
shapes; all three review surface switches preserve original image URLs and
geometry; no horizontal overflow or page errors. Original hash checks, source
bounds, strictly interior estimated attachment points and null pairing/item
bindings pass. See the [machine report](evidence/hair-registration.json),
[front 390px capture](evidence/hair-photo_04-390.png) and
[back 390px capture](evidence/hair-photo_05-390.png). Both full 390px captures
were visually inspected: all sixteen references on each sheet are present,
including pale panels and ornaments, with their estimated attachment markers.
The report is source-only evidence; transparent pixels, actual body fit,
production rendering, animation and mobile-device acceptance remain NOT_RUN.

Regenerate only the metadata, then validate and capture the review:

```sh
MCR_ART_PLAYWRIGHT=/absolute/installed/frontend/node_modules/@playwright/test/index.mjs \
  node frontend/src/entities/avatar/derived-assets/measure-hair.mjs
MCR_ART_PLAYWRIGHT=/absolute/installed/frontend/node_modules/@playwright/test/index.mjs \
  node frontend/src/entities/avatar/derived-assets/verify-hair.mjs
# http://127.0.0.1:4193/src/entities/avatar/derived-assets/hair-review.html
```

Both commands were executed with pinned Node 24.22.2 and the installed
Playwright path below and exited 0. Installed Oxlint 1.72.0 on
`frontend/src/entities/avatar/derived-assets/*.mjs` and `git diff --check` also
exit 0. This is scoped tooling lint, not application-wide lint/build. The scripts
write source metadata and browser evidence only; no source or cutout raster is
edited or generated. The same assigned writable paths and production exclusions
apply. FE12 runtime handoff `c2149dae63d21324179be3dca7df4bc79a003881` confirms the
unchanged C06 compositor and reviewed baked-tone binding; this hair registration
does not satisfy that production entry's missing compatible atlas requirement.

From the assigned repository root, with the project's pinned Node 24.22.2:

```sh
node frontend/src/entities/avatar/derived-assets/serve.mjs
# http://127.0.0.1:4193/src/entities/avatar/derived-assets/review.html

MCR_ART_PLAYWRIGHT=/absolute/installed/frontend/node_modules/@playwright/test/index.mjs \
  node frontend/src/entities/avatar/derived-assets/verify.mjs
python3 scripts/mcr/check-plan.py
git diff --check
```

The dependency-free server mounts only explicit review resources and binds to
loopback. The optional browser check reuses an installed locked Playwright via
the explicit environment path; it adds no dependency or lockfile. The original
hash check runs even without Playwright. `verify.mjs` reads image pixels for
analysis but does not write or transform source/derived raster artwork.

Executed with `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node`
and existing `/Users/lhs/Desktop/mcr/frontend/node_modules/@playwright/test/index.mjs`:

| Check | Exit/result | Evidence |
|---|---|---|
| `verify.mjs` with installed Playwright, native Chrome 156.0.8078.80 | 0; twenty original hashes/lengths pass; six regions/18 decoded views at 360/390/430; no horizontal overflow/page errors; three surface switches retain source URLs/geometry; six source dark-outline bounds remain inside their review windows | [Machine report](evidence/source-registration.json) |
| 390px visual inspection | Six complete source designs and foot markers visible; JPEG beige background visible as labeled | [390px capture](evidence/source-registration-390.png) |
| 360px and 430px browser captures | Captured; no real mobile device claim | [360px](evidence/source-registration-360.png), [430px](evidence/source-registration-430.png) |
| `python3 scripts/mcr/check-plan.py` | 0, PASS; planning/source archive checks only | No implementation acceptance inferred |
| `git diff --check` | 0 | No whitespace errors |

Application-wide build/lint, C06 production composition,
real devices, load, multiuser integration and AR03 acceptance: **NOT_RUN**.
No backend imports or external network dependencies are added. Exact changed
paths and commit ancestry are sent directly to delivery after committing; only
delivery publishes master. The unbudgeted asset objective remains unfinished
because layer derivation, animation and production compatibility are unfinished;
the historical recorded goal status is explained in the current checkpoint.
