# Cute community backgrounds — FE27 / FE21 / FE24

Status: **INTEGRATED contribution; owner visual PASS** for F01 Feed, M01 Meet,
and G01 Gifts at 360, 390, and 430px. Both modes of all 18 native-02 PNGs were
inspected in place. The short-row fit and sparse lower-panel coverage defects
are resolved; no additional asset or CSS change is indicated for these screens.
This is background-contribution evidence, not whole-task acceptance. All seven
SVGs are published; this evidence-document update awaits Root publication.

## Ownership and provenance

- Author chat: `01a1a534-4fcf-7291-abe3-a97ba2fcafcd`.
- Branch: `codex/mcr-art-cute-community-v1`; base
  `bfa0193939c3eebd6c80ade85083c6530c6875e1`.
- Original checkout was archived; the exclusive artist checkout was recreated at
  `/Users/lhs/Desktop/mcr-worktrees/art-cute-community-v1` on the same branch.
- Exclusive writes remain `frontend/public/backgrounds/cute-community-v1/**`
  and this document. COM01 owns the CSS adoption; Root153 owns composition,
  packet materialization and master publication. No artist browser, native,
  build, export, packet-copy or shared-checkout mutation was performed.
- The supplied ten-screen JPEG was re-inspected read-only, SHA-256
  `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
  No pixels or assets were copied from it.
- Seven original, editable, code-native SVG graphics; no third-party source,
  generated raster input, copied legacy art, embedded font, script, external
  resource or new runtime dependency. Characters, crops, renderer, text,
  controls, routes and layout remain outside this patch.

## Consumer interface and source validation

All URLs are same-origin under `/backgrounds/cute-community-v1/`, MIME
`image/svg+xml`. Exact byte sizes and SHA-256 pins are in
[manifest.json](../../../frontend/public/backgrounds/cute-community-v1/manifest.json).

| File | Intrinsic size / viewBox | Alpha | Delivery use |
|---|---|---|---|
| `feed-backdrop.svg` | 390 × 640 / `0 0 390 640` | Opaque | Loaded on Feed posts, footer and existing panel body. |
| `meet-backdrop.svg` | 390 × 640 / `0 0 390 640` | Opaque | Loaded on the existing Meet container. |
| `gifts-backdrop.svg` | 390 × 640 / `0 0 390 640` | Opaque | Loaded on Gifts rows and existing panel body. |
| `feed-corner.svg` | 160 × 96 / `0 0 160 96` | Transparent | Optional stationery/music ornament; not currently loaded. |
| `meet-corner.svg` | 160 × 96 / `0 0 160 96` | Transparent | Optional ribbon/heart ornament; not currently loaded. |
| `gifts-corner.svg` | 160 × 96 / `0 0 160 96` | Transparent | Optional parcel/ribbon ornament; not currently loaded. |
| `paper-tile.svg` | 64 × 64 / `0 0 64 64` | Opaque | Optional repeatable paper field; not currently loaded. |

Broad fields supply the dominant softer backdrop, rather than relying on small
corners. The three broad SVG roots use `preserveAspectRatio="none"`; the existing
consumer paints center / no-repeat / `100% 100%`. COM01's adopted leaf is
`frontend/src/shared/styles/cozy-community-backgrounds-v1.css`. Its Feed/Gifts
body rules use exact existing `data-mcr-panel` hooks. No extra element space,
character filter, parent opacity or decorative left rail was introduced.
The separate Wall rule in that leaf is outside this contribution.

Optional ornaments remain CSS backgrounds with no faces, people, avatar
silhouettes, product labels, stock claims, venue guidance or interaction
semantics. They must not overlap text, replace product photos/icons, repeat on
every row, or resize controls. Omit them when no empty region exists.

Source qualification passes: XML parsing, closed element/attribute allowlist,
matching dimensions/viewBoxes, and each SVG below 4 KiB. Only `svg`, `title`,
`desc`, `path`, `g`, and `circle` elements occur, with no event handlers,
resource hrefs, embedded rasters, or external/data URLs. Rechecked all seven
local byte/SHA pins against production and published Git objects; all match.

## Immutable native-02 evidence

- Packet commit: `5186f11017185a040e4ccde1c59589e140563ef0`.
- [Raw report](../evidence/cozy-backgrounds-v1/native-02/report.json), SHA-256
  `7f34e10279212d5f5d01017cbd644f02a33bd8939393b30818326ab2a489395b`.
- [Capture packet](../evidence/cozy-backgrounds-v1/native-02/capture-packet.json).
- Production/fixture/assets: `8972b0b634035055eb023dbc2f786e4463b172e5`.
- Runner: `e237572719602da6a1a08b7fe0579b0e9c96da1f`.
- Driver SHA-256:
  `58f553d9abfcfab1cac4e67ccf86c378493ec337d5c17f55749673957ba6dbde`.
- Job: `PANEL-CORRECTED-202611040733Z`, Chrome `156.0.8078.80`,
  `2026-11-04T07:38:12.062Z`–`2026-11-04T07:40:02.112Z`, exit 0.
- Comparison: same production and fixture, restoring only four background
  integration files to `7c25078f118da53dd9a524d0a58e707cca190f0f` for baseline.
  Meet typography is identical in both modes; this is not a whole-app checkout
  comparison against that base.
- Root packet technical result: `PASS_48_PAIRED_CORE_CASES_60_SCREENSHOTS`.
  This owner's acceptance covers only the 18 community PNGs below. The immutable
  report's `CAPTURED_PENDING_OWNER_VISUAL_INSPECTION` status remains unchanged;
  the scoped owner disposition is recorded here.

Every relevant PNG was viewed directly and independently matched against its
report pin, capture-packet pin and immutable Git blob. No evidence copy was made.
Files are `baseline-{screen}-{width}.png` / `candidate-{screen}-{width}.png`
under `docs/mcr/evidence/cozy-backgrounds-v1/native-02/`.

| Screen / width | Baseline PNG SHA-256 | Candidate PNG SHA-256 |
|---|---|---|
| Feed 360px | `1da1d96bc4548abf168979e5173e97454632fe4cdd46a98e56fe784d01412f50` | `78aaa9e8c6c28a5c44b7e6cd0a0fc94d7b55c887b0a5b6e37068ce2bca901fec` |
| Feed 390px | `e4d99d561d4c3b2ee5c52a60682920190885fea104efe286faf22e3d7f183851` | `2c4eee679c691499411c12791645188c8ebf597be655769f118fd7af191cefb5` |
| Feed 430px | `dc32ed36232d2adad0960c472ae3e9dd9ed7842f6edf2d97d107f679ee8635ba` | `2b334e3063b1b9261f504e338058b1c5f28c6a90762c1ca5ffb1c05d619a888f` |
| Meet 360px | `1c3a6ebab1efb7a189e2efbdec4dc37acd05c13265687c7daac6429cdb3f1f5d` | `5f307926d9038797e26d44c4e36c03a810b4b278ed14073803f57f2b5244d61d` |
| Meet 390px | `2d2fa482226c4b39f4c8ea958376957372c84c25c303d2819e37ab4a36a1e9cc` | `85f2d76ba60ab3a84af36b70aecf2b57d0057d29979d40987ac37801160a5be7` |
| Meet 430px | `563735f9f747f94ee915b49f275f089f5cc872fa8add3a70c8f1f09267288b45` | `d17be3e62d357b53691b51efccb9cc51a9cb231f3a977b382e0122ce2ac15b37` |
| Gifts 360px | `82058f0f287de34de6daea8ed534ea0e447a7e3d4b34eac8cc5283bdbbd77774` | `c2d74293250334b904a0bfdf3483deaf3b35e1fa6ed8e909fb5edf8863a30886` |
| Gifts 390px | `a9c8c7dff188c9262d132caec9302f40e2c3426ba4dd271447ed1df7cdd51f02` | `9214f16c9b2b1b665e35dd84a9ddac9a4f695063c8b85e792b7b00b206abf83f` |
| Gifts 430px | `25f5e111cddd206ec53b3aa22ce47d1fb6b302e7b4afe665b1a038045988bc5d` | `a1a34340585d0836e25ac130223b625119972cfff00c2ef6a49d1626a2656d6f` |

## Visual and preservation findings

- **Feed:** blush/lilac upper field and broad peach/cloud lower field cover the
  formerly distressed sparse lower body. Short rows fill across their width;
  no central letterbox strip remains. Quiet ivory keeps existing text readable.
- **Meet:** peach/sage upper field, quiet ivory middle, and broad lilac lower
  cloud remain coherent at all three widths. Existing character framing and
  black/red controls are preserved.
- **Gifts:** full-width lilac/blush rows and peach cloud lower body replace the
  former strip and exposed lower paper. Existing row boundaries, typography and
  controls remain intact.
- Against the supplied reference and each actual baseline, these are dominant
  warm, soft fields at a glance. No ornamental rail was added. Existing black
  parade/footer silhouettes are unchanged. COM01 independently agrees that the
  three screens' row-fit and lower-body coverage defects are resolved.

For all nine candidate cases, the raw report records no errors or horizontal
overflow and true `controlsPreserved`, `imagesPreserved`, `textPreserved` and
`characterPixelsPreserved`. Directly recomputed paired equality for controls,
text, images and character records also passes. Feed has 16 queried controls,
Meet 21 and Gifts 19 at every width. Every queried rectangle has width and height
at least 44px (minimum 44px). This covers the report's controls query only, not
`details`/`summary`, all possible targets, offscreen usability or unseen states.

Direct character evidence records one Feed canvas and three Meet canvases,
each 64 × 64 source pixels, with unchanged rectangles and RGBA SHA-256
`a4a6b1ceac075574b988f94f45fe839efc3e14692db73f8fdcfd8131ba3681d5`.
Feed renders at 46 × 46; Meet at 64 × 64. Gifts has zero character canvases;
all three routes have zero selected-static CSS spans. Those zeros describe the
captured scene; empty image arrays alone are not character-preservation proof.
No character asset or renderer was changed by this contribution.

Contrast arithmetic uses conservative darkest unblended opaque field paint
`#efd6cd`: Feed ink `#201b18` **12.320:1**, Meet ink `#211b18` **12.287:1**,
Meet hint `#67594b` **4.882:1**, Gifts note `#5a4d43` **5.886:1**. These are
source calculations. `metrics.paints.color` is background color, not text color.
Candidate first-tab records say only BUTTON / solid outline at each width;
they do not prove computed focus color, width, offset, contrast or every focus
state. Browser-computed text/focus contrast and exhaustive keyboard/forced-color
checks remain unverified for these routes. Me's dedicated focus/forced-colors
captures cannot certify Feed/Meet/Gifts. Existing disabled/access/stock states
were preserved in the paired snapshot, without exhaustive interaction coverage.

The packet uses the actual default router with controlled DEV QA06 synthetic
HTTP/C05/local admission. It is desktop Chrome evidence, not live-service,
physical-device, Profile-save, Me504, guarded Wall editor or full FE acceptance.
Reported browser/server closure and temporary cleanup are all true. No separate
artist capture or expanded test matrix was launched.

## Delivery and historical correction

Source publication was freshly checked at `origin/master`
`39c68d1dcd85a618ec0c56287d8451c6ffe13dbd`: all seven SVG bytes equal this
manifest and production `8972b0b6`; packet `5186f110` is an ancestor. This proves
asset and evidence publication. Root remains the sole integrator/publisher of
this new manifest/document disposition; its publication receipt is pending.

Original assets: `2a5c67e5d269db818431b28dcef1217b9782b71f`.
The initial native-01 job `PANEL-COZY-202611040632Z`, candidate
`c8a731e5c7f03c3fbcbd0581c6e872429806fa22`, report SHA-256
`59e0f2540a1fdc79f240230048385fda678c1029e9d3df3ab2a209320ae86da1`,
exposed narrow central Feed/Gifts row strips and old paper in the sparse lower
bodies. Those historical captures were inspected but did not accept the fix.

Fit correction `124f3ed8` added only `preserveAspectRatio="none"` to the three
broad SVG roots; removing that attribute recovers each original file exactly.
COM01's separately authorized body coverage correction `a37e65cc` extended only
background paint to the existing exact Feed/Gifts body hooks. Native-02 now
verifies both fixes at all three widths. Earlier pending/NOT_RUN statements
were historical checkpoints and are superseded by this scoped evidence.

Only this document and the pack manifest change in this disposition update;
no SVG pixels, application source, character source or packet files change.
`git diff --check` and exact changed-path enumeration pass. Historical
`python3 scripts/mcr/check-plan.py` exit 0 was planning provenance only, not
application acceptance; no new build is needed for this evidence-only update.
