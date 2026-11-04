# Wall and Me background artwork — 2026-11-04

Status: **INTEGRATED; owned background visual validation complete**. Me passes
this asset owner's scoped inspection in immutable `native-02` at 360/390/430px,
including four focused header links and forced colors. The tall Wall correction
passes direct owner inspection in published `wall-com-03` at all three widths,
including the relevant captured normal/forced-color guard and focus states.
The tested SVG bytes are published. Final inspection documentation publication
is the remaining handoff step. Raw `wall-com-03` exit 1 / `OPEN_FAILURES` remains
unchanged: invalid forced-color measurement assertions are qualified below,
while a real inherited Meet focus-clipping finding stays open with its owner.
This document does not accept FE32, FE34, IN04, or mobile release gates.

## Assignment and ownership

- Tasks: FE32 / R01 Wall, FE34 / U01 Me, background artwork only.
- Source baseline: `bfa01939`, isolated worktree
  `/Users/lhs/.codex/worktrees/12a1/mcr`, branch
  `codex/mcr-art-cute-wall-me-v1`.
- Editable asset handoff: `8ff3a58f556bf2811ad1415cd31c5ba1953ccd58`.
- This author owns only new
  `frontend/public/backgrounds/cute-wall-me-v1/**` and this document.
- ID01 (`01a1a533-50a8-7fb2-b3ed-96b941b96bbf`) is the Me CSS consumer;
  COM01 (`01a1a533-50a9-73a1-8d4f-a16bf0b69aee`) is the Wall CSS consumer.
  Exact selector, URL and property grants belong to Root153. Existing functional
  CSS remains read-only. Root153 owns imports and master publication.
- One capture set is shared with those consumers under Resource SL21d's named
  frontend slot, after the required World/Map 390px cue. No duplicate capture
  matrix, native job or build is started by this asset author.

## Source and provenance

All three SVG files are newly authored editable vector source for this task.
They contain geometric paths, circles, repeated paper dots and local SVG uses;
there are no copied upstream assets, raster images, character silhouettes,
fonts, scripts, filters, animations, external URLs or dependencies. They are
background decoration and convey no application state, entitlement, place,
identity, control or interaction. Each has an opaque base rectangle; ornament
opacity is internal, not a transparent compositing layer over character pixels.

The supplied collage was inspected without modification at
`/tmp/codex-remote-attachments/01a1a385-9f32-73c2-8715-ff54bd1e0497/120DC5EC-1683-42AB-AF36-0DA8EB43CDE0/1-Photo-1.jpg`.
SHA-256: `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
It provides comparison context only; no pixels or shapes were traced/copied.
The user's newer instruction requires visibly cuter dominant backgrounds,
rather than tiny additions to the old distressed dark field. Broad peach
scallops, cream/blush paper, lilac/sage clouds and warm berry quilt replace that
field only at the consumer's explicitly granted background surfaces.

No selected character source, derivative, atlas, crop, framing, sprite, renderer,
canvas, mural cell, palette swatch, control, text, layout, route or behavior is
changed by this patch. `docs/source/` is untouched. No contract version,
migration, manifest or lockfile changes are needed.

## Asset interface

All URLs start with `/backgrounds/cute-wall-me-v1/`. The SVG itself is the
editable source and the served output; there is no separate raster export.

| Filename | Intrinsic size / viewBox | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `wall-stationery.svg` (tall-paper revision) | 390×780 / `0 0 390 780` | 4500 | `22e2de672776682693bca71c57f501f2450bc9a28bc6314df44c26d6d193efc7` |
| `me-scrapbook.svg` | 390×240 / `0 0 390 240` | 2728 | `98c351c141e372d8d7e0d6879556105c551fbdf7478d66afe76d360a59a7aae1` |
| `me-night-ribbon.svg` (focus-background revision) | 390×180 / `0 0 390 180` | 2298 | `29ad1cb9d04101399e2483fef9de6a3d710c6bf7f08cb1cb4501fb746152932d` |

| Asset | Background role and proposed use, subject to Root's exact grant |
| --- | --- |
| Wall stationery | Broad cream `#fff0de`, peach scallops `#f3c9bd`, lilac `#e5d8e8`, sage `#dce3cd`, broad blush/sage middle shapes and large lower clouds/ribbon. Guarded Wall panel-body surround only; never the canvas, tool stack, pixel grid or swatches. Existing `center top`, `100% auto`, `no-repeat` and matching solid fallback stay valid. This is one tall composition, not a seamless repeated tile. Original top ornament geometry/paints remain unchanged; new forms occupy the formerly sparse lower area. |
| Me scrapbook | Broad blush cream `#fff0e7`, rose scallop `#efc3c9`, lilac `#ead9ea`, sage `#dce5d0`. ID01 may propose `.me-hub--reference .me-bio` and `.me-activity-navigation` background paint; no Me body grant is implied. Keep existing opaque controls. At small bio heights the broad warm fill and upper scallop remain visible while lower clouds may be clipped. |
| Me night ribbon | Warm berry upper paper `#573344`, mauve clouds, quilted hearts and a large soft bow; an opaque lower paper shelf `#fff0e7` with only light blush/lilac/sage paints protects the unchanged red focus outline. Same header background selector only. Requested failed-image fallback is former header `#1c1a18`, preserving baseline outline and light-text contrast. The existing opaque avatar container and every character pixel remain unchanged. It is not a portrait frame or character background asset. |

Use these as CSS backgrounds, not DOM images, pseudo-elements, buttons, masks,
full-screen screenshots or avatar textures. Leave the existing painting order,
dimensions, padding, borders, touch targets, focus outlines and overflow rules
alone. Nothing should acquire pointer events, an accessibility role, or a name.
Do not apply an opacity/filter to a parent containing foreground content.
There is no decorative left rail or accent border in the pack.

The broad center remains visually quiet: roughly x=80–300 / y=40–115 for Wall,
x=85–290 / y=40–150 for Me paper, and the full middle text area of the berry
header. Ornament/background fills support dark `#211d19` text on light paper;
the berry background is for existing light header text only. No color change is
authorized for text or controls. Asset loss should reveal the matching solid
background color, with all content and controls still working.

## Validation and limitations

Source checks on 2026-11-04:

| Command / inspection | Result |
| --- | --- |
| `python3 scripts/mcr/check-plan.py` | Exit 0; planning provenance/export checks PASS. Application checks remain separate. |
| Python stdlib XML parse + dimensions/bytes/SHA-256 inspection | Exit 0 for all three SVGs. |
| XML audit for scripts, raster images, foreign objects, animations, filters, text nodes, external hrefs and event attributes | Exit 0; none present. SVG title/description are metadata only. |
| Original-pack paint-palette and local-reference audit | Exit 0 for `8ff3a58f`. All SVG use/pattern references resolve and IDs are unique within each file. Its 9 berry paint colors meet at least 4.85:1 against `#f5ecdc`; all 21 Me paper colors meet at least 8.49:1 against `#211d19`; all 21 Wall colors meet at least 7.36:1 against `#211d19`. The later revised header has separate upper text and lower focus paint zones, audited below. These source-color checks do not replace rendered validation. |
| WCAG relative-luminance calculation | Dark `#211d19` on rose `#efc3c9`: 10.62:1; on peach `#f3c9bd`: 11.08:1; on lilac `#e5d8e8`: 12.21:1. Light `#f5ecdc` on the brightest berry ornament color `#875a69`: 4.85:1; on berry base: 9.13:1. Opacity blends those ornaments toward the darker base. These are source color checks, not a complete rendered accessibility audit. |
| `git diff --check` | Exit 0. |
| First-candidate protected source comparison | Exit 0; `git diff --name-only 7c25078f c8a731e5 -- frontend/src/entities/avatar frontend/public/world/avatars frontend/src/world/avatar frontend/src/features/avatar frontend/src/features/me/ui/MePages.tsx frontend/src/features/me/ui/me.css frontend/src/features/mural/editor frontend/src/features/mural/sync` returns no changed paths. This is source preservation, not proof of every rendered frame or width. |
| Browser decode / actual application capture / width checks | Actual 390px default-router capture inspected below; 360/430px and additional accessibility checks remain **NOT_RUN**. No duplicate workflow. |
| Original-pack publication / delivered ancestry | `git fetch origin master`, exit 0: `origin/master` at `c8a731e5c7f03c3fbcbd0581c6e872429806fa22`. The delivered original-pack commit `0ac3ef9f90cca8496fbde083c15046753a2130ac` is a verified ancestor; independent `git show` comparisons match all three original `8ff3a58f` SVGs and their recorded hashes. The later header focus-background revision is not covered by that publication check. Root153 remains publishing owner for subsequent paint/evidence/doc changes. |

In-app adoption remains necessary. A small background allocation may hide large
ornaments behind opaque functional content; that must be exposed in native
390px evidence rather than claiming that source artwork alone establishes the
at-a-glance requirement. Wall's immutable drawing canvas will still dominate its
screen. The actual guarded public Wall state must be captured truthfully; a
fixture editor must not be substituted for the public root guard. Me must retain
the actual approved full-character display or honest unavailable state.

The consumer evidence must record exact tested commits/asset hashes, same-state
390px before/after, 360/430px overflow and control/focus checks, reduced-motion
and forced-color behavior, and unchanged character/canvas rendering. This asset
pack adds no motion and should disappear naturally in forced colors if the user
agent suppresses background images. Physical-device, live multiuser and release
qualification remain outside this bounded asset source check.

## Committed consumer source handoffs

The asset author inspected these committed CSS declarations through `git show`
on 2026-11-04; this establishes source adoption only, not live application use.

| Consumer commit | Exact adoption of this pack |
| --- | --- |
| COM01 `8824401dec061102c9af33ba302855e48563170a` | New `frontend/src/shared/styles/cozy-community-backgrounds-v1.css`: `:root .shell-panel[data-mcr-panel='wall'] > .shell-panel-body` uses `wall-stationery.svg`, fallback `#fff0de`, `center top`, `100% auto`, `no-repeat`. No mural/editor/character selector. Root's existing-main route attribute and stylesheet import are still required. |
| ID01 `e44b5d1c38f05b03a74904c94004d5fc983b2493` | New `frontend/src/shared/styles/cozy-identity-backgrounds-v1.css`: `:root .me-hub--reference .me-profile-header` uses `me-night-ribbon.svg` / `#573344`; `.me-bio` and `.me-activity-navigation` under the same Me root use `me-scrapbook.svg` / `#fff0e7`. These three rules set background paint only, with `center`, `100% 100%`, `no-repeat`. No avatar selector. Root's stylesheet import is still required. |

Neither consumer source commit alone proves publication, image decoding,
same-state geometry/pixel equivalence, control usability or the requested
at-a-glance visual improvement. The separate evidence below records the
subsequent publication and 390px findings, with remaining obligations explicit.

## Actual shared 390px evidence

Root's finite job `PANEL-COZY-202611040632Z` ran from
`2026-11-04T06:33:32.440Z` to `2026-11-04T06:34:13.328Z`, exit 0, Chrome
`156.0.8078.80`, at 390×844. The immutable packet is
[`native-01/report.json`](../evidence/cozy-backgrounds-v1/native-01/report.json),
SHA-256 `59e0f2540a1fdc79f240230048385fda678c1029e9d3df3ab2a209320ae86da1`.
Its capture candidate is `c8a731e5c7f03c3fbcbd0581c6e872429806fa22`.
The original report and PNGs are retained in immutable Git commit
`16bf30c617995f87c89f94a13d9cc5f4c65d51b5`. After a reported sparse-checkout
retention issue, the asset author independently compared the canonical publisher
report and all four Wall/Me PNGs against that commit and the original hashes
below: exact bytes match. No rerun, reconstructed image or substitute cohort is
used. The canonical physical packet is under
`/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish/docs/mcr/evidence/cozy-backgrounds-v1/native-01/`.
The asset author independently compared all three original candidate SVG blobs
with `8ff3a58f`: exact bytes match. Me paper still matches the current
asset-interface hash above. This first capture used original Wall SHA-256
`1929fda18afe73000f79a44a7cb37c5504bee42d450bff343a562baa85e2e227`
and original header SHA-256
`1519b1b125190be901e1119105eafd057532ff98d1f091cf450f0f31f72b62bc`,
not the later focus-background revision.

This is the actual default router/Auth/Phaser rendered through the QA06 DEV
synthetic HTTP/C05/local-admission harness. It is not a live service or a
physical device. Both phases used the same candidate, fixtures and Meet text;
only four paint integration files were restored to exact `7c25078f` bytes for
the baseline: `app/shell.css`, `app/shell-chrome/shell-chrome.css`,
`shared/styles/base.css`, and `app/WorldShell.tsx` under `frontend/src/`.
The later fourth shared-paint cohort was not included in this packet.

The asset author directly viewed the four PNGs below and recomputed their
SHA-256 hashes against the report. No extra browser, capture or image export
was run.

| Owned route / phase | Evidence | SHA-256 |
| --- | --- | --- |
| Wall baseline | [390px PNG](../evidence/cozy-backgrounds-v1/native-01/baseline-wall-390.png) | `6ee53f544d8b1619e23c2fda7dba413afb19b9ce6b7c4509fdf1419904bb4bea` |
| Wall candidate | [390px PNG](../evidence/cozy-backgrounds-v1/native-01/candidate-wall-390.png) | `bd085e3bbab0b848a77f3c7e3ac76207aa7b37c64e336cd4289edc2f42397b40` |
| Me baseline | [390px PNG](../evidence/cozy-backgrounds-v1/native-01/baseline-me-390.png) | `82a4dc4db9e98e776cd67a1184f9e9abad0d6aacb94e8f27e08c93954c93cb54` |
| Me candidate | [390px PNG](../evidence/cozy-backgrounds-v1/native-01/candidate-me-390.png) | `d3f97daa18096a42033c1be905a7f30d41b32ba25f561f8255da65ff3a6a8bbb` |

Owner visual finding: Wall's former textured paper is visibly replaced by warm
cream, broad peach scallops and lilac/sage cloud shapes. The exact unavailable
guard remains readable; no editor is shown or implied. Me's former black header
is visibly replaced by berry quilt, bow, cloud and scallop backgrounds. Existing
light nickname/era text and dark bio text remain legible, and the selected full
character remains on its original opaque field. The lower Me activity paper and
marching silhouettes remain original under the current ownership/grants. No
owned asset correction was established at 390px; this does not accept the full
page or the untested widths/states.

For both owned pairs, the report records no overflow, no runtime errors,
identical body text and identical visible control rectangles, fonts, colors and
disabled states. The recorded paint URLs correspond to all three owned assets.
One Tab step records a solid focus outline; it is not a complete keyboard/focus
audit. The reported `imagesPreserved` is vacuous here because both routes have
an empty `document.images` list. It must not be cited as independent proof of
the CSS/canvas character's RGBA preservation.

To cover the actual visible Me character in this packet, a read-only Pillow
comparison of the two immutable Me PNGs found exact RGBA equality for the
140×140 opaque portrait interior at screenshot box `(23,73,163,213)` (Pillow
right/bottom exclusive). This box contains the complete visible character.
RGBA SHA-256 in both phases:
`aefbbb51305c5112db0e54a20c902ae5e32db689916c9f4d93993191409f09cf`.
It does not certify fractional outer frame edges, all source characters, other
widths or hidden world-canvas pixels. Root was informed of the image-list gap
for its bounded follow-up. No source pack, renderer or framing changes are
introduced to obtain this evidence.

Remaining: actual 360/430px responsive captures, additional 44px/focus and
reduced-motion/forced-color checks, any resulting owned fixes, later Me paint
capture, and final documentation publication. Goal remains active.

## Later Me outer-paper placement

After inspecting the first packet's large unchanged activity-paper region,
Root granted ID01 the outer route body paint. ID01 source `8ba3c7a6` adds
`:root .shell-panel[data-mcr-panel='me'] > .shell-panel-body` using the same
`me-scrapbook.svg`, `#fff0e7`, `100% auto`, `no-repeat`; Root owns the route
attribute. The asset author flagged a concrete composition issue: with the
first packet's body y=57, header y=63–271 and navigation y=271–324, a top-aligned
390×240 image at width 372 ends near y=286, hiding almost all large cloud shapes
behind the opaque header/navigation.

Root explicitly granted bottom placement. ID01 committed the one-property
correction in `d1e756e02016adf68a0396cad26c8e490ffd4667`, setting the outer Me
paper's background position to `center bottom`. The asset bytes, character
rendering, foreground layout and control geometry remain unchanged. This is a
source correction resulting from actual 390px evidence; it is not included in
`native-01` and still needs the next coordinated rendered packet.

## Background-only Me focus contrast correction

The actual first packet plus unchanged source establishes a material regression
around the four Me header icon links. Their inherited outline remains 3px
`#a1212c` with 3px offset. Source contrast against former `#1c1a18` is
2.288:1, against berry `#573344` 1.41:1, and against cloud `#75495d` 1.04:1.
ID01 inspected the real PNG strip y=215–217 at each icon: baseline pixels are
`#1c1a18`, while the candidate has berry/mauve backing. Root explicitly assigned
the asset author a background-only correction; no focus style, control color,
character, layout or framing change is authorized or introduced.

The actual candidate header rectangle is `(15,63,360,208.390625)`. The four
48×44px links start at x=81/141/201/261, y=220.390625. Including the full 6px
outline-plus-offset extent, the corrected SVG's explicit
`preserveAspectRatio="none"` maps their outer boxes to:

| Link | SVG outer focus box (left, top, right, bottom) |
| --- | --- |
| Edit profile | `(65,130.7655,130,179.1362)` |
| Dress up | `(130,130.7655,195,179.1362)` |
| Manage private social links | `(195,130.7655,260,179.1362)` |
| Sharing requests | `(260,130.7655,325,179.1362)` |

The revised SVG retains its intrinsic dimensions/viewBox and original upper
berry/quilt/bow. Broad mauve clouds move upward. The last full-width opaque
rectangle covers y=120–180 with `#fff0e7`; the only subsequent paint is clipped
to that shelf and uses `#ead9ea`, `#dce5d0`, `#efc3c9` and `#fff0e7`.
No berry paint, stroke, opacity or external resource follows that opaque shelf.
Thus all four measured outline perimeters are fully on the light paper zone,
with over 10 SVG units of clearance from its upper edge.

Read-only source validation exits 0: XML parses; all local references resolve;
no active/raster/external content; painter order and clipped shelf palette are
closed; all four measured focus boxes fit inside y=120–180. Individual shelf
colors contrast with the unchanged outline by 4.81–6.81:1. A conservative
per-channel minimum envelope `#dcc3c9` also exceeds the former 2.288:1 bound,
covering antialiased mixtures among those light paints. The failed-image
fallback must be former `#1c1a18`, retaining 2.288:1; ID01 owns that existing
background-color declaration and its closed-source pin update. This does not
claim the historical outline passed a 3:1 requirement.

The actual fixed rendering, every focused icon, responsive widths and relevant
text states still require Root's coordinated follow-up. Static geometry/color
proof is a source handoff, not visual acceptance. No browser, capture, PNG
restoration or sparse-checkout operation was started by this author.

## Corrected shared packet `native-02`

The asset author restored only their unclaimed isolated source checkout on
`codex/mcr-art-cute-wall-me-v1` at preserved `856c7170`; Root's publisher and
packet files were read in place. No packet copy/materialization, sparse repair,
browser or native matrix was started.

The immutable packet commit is `5186f11017185a040e4ccde1c59589e140563ef0`.
Canonical packet:
`/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish/docs/mcr/evidence/cozy-backgrounds-v1/native-02/`.
The author directly verified the owned PNGs and report against both immutable
Git blobs and the capture manifest, then viewed all twelve Wall/Me core PNGs,
all eight baseline/candidate Me focused-link PNGs and the forced-colors PNG.

| Pin | Value |
| --- | --- |
| Production and fixtures | `8972b0b634035055eb023dbc2f786e4463b172e5` |
| Runner | `e237572719602da6a1a08b7fe0579b0e9c96da1f` |
| Driver SHA-256 | `58f553d9abfcfab1cac4e67ccf86c378493ec337d5c17f55749673957ba6dbde` |
| [Raw report](../evidence/cozy-backgrounds-v1/native-02/report.json) SHA-256 | `7f34e10279212d5f5d01017cbd644f02a33bd8939393b30818326ab2a489395b` |
| [Capture manifest](../evidence/cozy-backgrounds-v1/native-02/capture-packet.json) SHA-256 | `ba34f05a25a30b42475fa13c1349576bdc0480a69eb4aeec1d444b7d91dc16fe` |
| Job / actual run | `PANEL-CORRECTED-202611040733Z`; `2026-11-04T07:38:12.062Z`–`07:40:02.112Z`; exit 0 |
| Browser / scope | Chrome `156.0.8078.80`; controlled QA06 synthetic HTTP/C05/local admission, actual default router/Auth/Phaser |

The baseline restores only the same four paint integration files to `7c25078f`;
all other frozen production/fixture source is identical in both phases. This
packet tests header hash `29ad1cb9…`, Me paper `98c351c1…` and **old 180px Wall**
`1929fda1…`. It does not test the new 780px Wall source `22e2de67…`.

| Route / width | Baseline PNG SHA-256 | Candidate PNG SHA-256 |
| --- | --- | --- |
| Wall 360 | `411911482b29ba80255f7c8f344c845499a8aca6f48abec811e63c049b3b0ff6` | `c9d97fc5a42910a8e590032867eedd161d0f8fc41dd30da929033b18398875ef` |
| Wall 390 | `6ee53f544d8b1619e23c2fda7dba413afb19b9ce6b7c4509fdf1419904bb4bea` | `ac3b1d2230e5076ea27e4d875fa97a35130527ffeebec38bf314f0dfbcd3d93f` |
| Wall 430 | `05eada7abfd4e543bdd353fdad9d6908cd311c7cbc3755e2af52d4ce39da6b56` | `871ec6c72b9086d92b906f1554491785b4b371ca802cd24b15a06d8202c70864` |
| Me 360 | `9655d2c0c4cdb96833e75cea9eb2703a9ba960566ce001bb944f1584d04b9421` | `8d738d11653d7a3393a50309d5ceadf60866b73cb4129516ceae3e350d317e86` |
| Me 390 | `82a4dc4db9e98e776cd67a1184f9e9abad0d6aacb94e8f27e08c93954c93cb54` | `24fc709693f07591cacd1b2f67f2afbb61e20d00d765ebf182c168359fc67b13` |
| Me 430 | `abf3cd505d07da0861f5a0e41ce4b490cb79aa642ef49a19db53f09cf34a663e` | `66d2dd7768c1ff2c635972283e1282691c22754dabeee1225664650bdcf15ba0` |

Me visual disposition: **PASS for this owned background scope and captured
states**. The upper berry quilt/bow, warm light focus shelf, blush outer paper,
large lower ribbon/clouds and retained character form a visibly warmer, softer,
more playful background than the old grunge/dark UI and the supplied collage.
This is a broad surface change, not a few tiny accents. At each width the
nickname, era, bio, navigation and unavailable-activity text remain legible.
The selected full character stays on its original opaque field. The source
does not change its crop, renderer, framing or proportions.

The report now measures the actual visible Me canvas, not an empty HTML-image
array: one canvas, intrinsic 128×128, RGBA SHA-256
`5ebf042e3a925cb16bdc690e168e8f4aa28e3faefbb75bc2006f50ccd4317a9d`
in both phases at all three widths, with exact matching rectangles/dimensions
per pair. At 360px its pre-existing CSS rectangle is 117.59375×128; that remains
unchanged and is not a new framing adjustment. Wall explicitly has zero
character canvases/spans in its guarded state; this is not editor-pixel proof.
Both owned routes have identical body text, recorded control geometry/type/
colors/states, no recorded control under 44×44, no horizontal overflow and no
runtime errors. These statements cover the driver's recorded controls rather
than an unbounded accessibility audit.

The four actual candidate focus PNGs show clear unchanged red outlines, width
3px, offset 3px. Four perimeter samples per icon give minima respectively:
Edit profile 5.637:1, Dress up 6.234:1, Manage private social links 5.836:1,
Sharing requests 5.536:1, versus 2.288:1 for every baseline icon. This confirms
the background correction improves the sampled real outline backing without
changing focus geometry. It is not a claim that the old baseline passed 3:1.
The tested CSS also restores failed-image fallback `#1c1a18`. A failed-image
runtime case is not independently captured here.

The actual forced-colors/reduced-motion Me PNG keeps system foreground text,
control borders and the focused fourth-link outline readable. SVG background
art remains colored in this browser; it is not claimed to disappear. The
recorded forced-colors and reduced-motion media are active. The ordinary
contexts also request reduced motion, and the owned SVGs contain no animation.
Focus and forced-color artifact hashes are retained in the pinned manifest.

Wall visual disposition: **NEEDS_CORRECTION** despite preserved truthful guard,
legible text and warm palette. At 360/390/430px the 180px image occupies only the
upper part of the 780px body; roughly 590–620px below remains plain cream. That
is too sparse for the latest dominant-playful background instruction. COM01
independently reported the same gap; neither owner treats technical invariance
as visual acceptance.

Fresh publication check: `git fetch origin master`, exit 0, finds
`39c68d1dcd85a618ec0c56287d8451c6ffe13dbd`. Both frozen production `8972b0b6`
and packet `5186f110` are verified ancestors. Remote Me SVG bytes equal the
current tested header/paper hashes. Remote Wall remains the old `1929fda1…`.
The new tall Wall revision and this final inspection documentation still need
Root's integration/publication.

## Tall Wall correction from actual responsive evidence

Only `wall-stationery.svg` changes. Its width stays 390; intrinsic height and
viewBox height become 780. Base cream and paper dots extend down the composition.
All original top pattern definitions and ornament geometry/paints are retained;
new art begins below y=258. Broad blush and sage middle forms, large lower
lilac/sage clouds, a larger ribbon and soft music details occupy the formerly
sparse field. No new foreground, character, state, label, control or venue cue
is introduced. The original top guard text region is unchanged.

COM01's existing `center top`, `100% auto`, `no-repeat`, `#fff0de` background
mapping needs no CSS change, only closed-source dimension/hash metadata. With
the actual captured body widths 342/372/412 and y=57, the new lower-ribbon
center maps to screenshot y=555.97/599.74/658.10, inside every 780px body. The
composition returns to its matching cream base at the bottom; it is not tiled.

Source checks exit 0: XML/dimension/reference/inert-content audit, original top
geometry/paint comparison excluding insignificant XML tail whitespace, and
complete palette luminance audit. Minimum unblended source contrast remains
7.364:1 for dark `#211d19` against the darkest paper paint `#c9a58b`; all new
large forms are lighter. `git diff --check` exits 0. This proves source bounds,
not the final rendering.

Remaining required evidence: Root's bounded fresh Wall 390px before/after and
360/430px captures (including relevant focus/contrast and forced-color guard
readability), owner inspection/fixes, and final source/doc publication ancestry.
Me's unchanged tested assets do not need a duplicate matrix. Goal stays active.

## Tall Wall owner disposition: `wall-com-03`

This section resolves the preceding tall-Wall handoff's pending evidence. The
earlier `native-02` 180px Wall remains historically **NEEDS_CORRECTION**; its raw
packet is not rewritten. The source correction is owner commit
`18e6e4db2f2e6384c796453d6e63caedc11875e8`, imported by Root in delivered commit
`70a444bf`. Root retained and published the new packet in
`1722a81734947764996590b94a63aa001501df28`.

| Pin | Value |
| --- | --- |
| Frozen candidate | `cc20832a897f18cd83482dcd21b6c2fc1d226942` |
| Controlled baseline | Same candidate, fixture and foreground source; **only** `wall-stationery.svg` restored to `8972b0b634035055eb023dbc2f786e4463b172e5` |
| Tested Wall SHA-256 / bytes | `22e2de672776682693bca71c57f501f2450bc9a28bc6314df44c26d6d193efc7` / 4500 |
| Driver SHA-256 | `84b23c4d4de54d7126447cc6a282b042db216d72a3311eb4b30febc965e7db6f` |
| [Raw report](../evidence/cozy-backgrounds-v1/wall-com-03/report.json) SHA-256 | `8056cea82655a3ae95e6520952a351890a3eb921953262378ebf260ca89b4c5f` |
| [Capture manifest](../evidence/cozy-backgrounds-v1/wall-com-03/capture-packet.json) SHA-256 | `4b1b8bf2d7a29678d072f6073c37e0a43ce646beb3a0cb99766275b2c952e61e` |
| [Qualification](../evidence/cozy-backgrounds-v1/wall-com-03/qualification.json) SHA-256 | `f0ac612f201d43863086039a0284917f0cd9a6677fad5fe0b62a107d258c8301` |
| Job / actual run | `ROOT-WALL-COM-A11Y-202611040820Z`; `2026-11-04T08:17:02.026Z`–`08:17:31.969Z`; Chrome `156.0.8078.80` |
| Immutable raw result | Exit **1**, `OPEN_FAILURES`; 30.390 seconds, resources released at `08:17:32.004653Z` |

Canonical files were read in place at
`/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish/docs/mcr/evidence/cozy-backgrounds-v1/wall-com-03/`.
The author started no browser, native job, test matrix, packet copy, restoration
or materialization. The original active goal and clean isolated source checkout
at `18e6e4db` were verified before inspection.

| Width | Baseline PNG SHA-256 | Tall Wall PNG SHA-256 |
| --- | --- | --- |
| 390 | `ac3b1d2230e5076ea27e4d875fa97a35130527ffeebec38bf314f0dfbcd3d93f` | `ec075ce0ceb6a029a5b469fe2db8ba48b7668a7c83bd8a5845fd7019a8417a9f` |
| 360 | `c9d97fc5a42910a8e590032867eedd161d0f8fc41dd30da929033b18398875ef` | `d5281ac77ad5b53a32fc3025cfc819865e1d78259ec90cb996fc2946de43e203` |
| 430 | `871ec6c72b9086d92b906f1554491785b4b371ca802cd24b15a06d8202c70864` | `9bb0bf3f6ac6790306f12a39be4db4fcfec262788fd8f267c94880624522c357` |

**Visual disposition: PASS for the owned background change and captured
guarded-Wall states.** The author directly viewed the 390px pair, then both
360/430px pairs, the Wall normal/forced focus images and both text-backing
images, and compared the supplied source collage again. Large blush and sage
middle forms and the lower lilac/sage clouds and ribbon now occupy the sparse
field. Warm paper, scallops, soft music details and broad pastel forms are
visibly cuter than the former dark/distressed reference and more substantial
than the rejected short composition. At 430px the existing viewport clips the
bottom wave, while the lower clouds/ribbon remain visible; at 360px the whole
composition ends above a matching cream field. Both are coherent uses of the
unchanged `100% auto`/top/no-repeat mapping. The guard remains legible and does
not imply an admitted editor or successful service connection. No source fix
is required after this inspection.

Independent read-only Python checks against immutable Git objects and retained
PNGs exit **0**:

- All 725 recorded source/asset/physical-CSS pins match their Git revision bytes;
  all 32 artifact pins match both retained bytes and commit `1722a817`. These
  physical CSS pins do not certify individual Vite nested-import load receipts.
- At all three widths, raw baseline/candidate metrics and recorded shell state
  are exactly equal: text, control type/label/geometry/font/color/disabled state,
  body rectangle and guard/admission state. There is no horizontal overflow or
  recorded runtime error. All recorded controls are at least 44×44. The visible
  non-World character list is explicitly empty in both phases; Wall provides
  **no character/editor pixel proof**. Me's earlier actual canvas/RGBA proof is
  unchanged and remains attached to `native-02`, not this Wall packet.
- The header raster above y=57 is byte-identical per pair. Unchanged top SVG
  ornament geometry is a source claim, not top-background raster identity:
  broader pixel-equality assertions were rejected by the readback, so no such
  equality is claimed for the deliberately repainted body.
- Recomputing every sampled normal guard text-range backing pixel from the
  retained PNG gives **9.545801856993844:1** over 6927 pixels. The text-fill mask
  visibly removes the normal glyphs. This applies to the captured 390px guard
  ranges, with direct readability inspection at 360/430px.
- All four actual normal back-button outline samples match the recorded RGB,
  with minimum **15.114013729961503:1**. The captured target is 44×44, outline
  2px, offset -3px, focused and `:focus-visible`. Focus was set by the driver;
  this is not proof of a complete keyboard traversal.
- In forced colors, the four actual Wall ring samples are `[55,51,109]`, the
  correct composition of `rgba(5,0,73,.8)` over white, giving
  **11.309774587047812:1**. Comparing these with uncomposited `[5,0,73]` caused
  the raw false failure. Independent raster sampling reproduces the pixels and
  alpha calculation. Two unmasked text fragments each have an actual black
  glyph-core spot and white system-backplate spot (21:1); the guard is visibly
  readable. These are limited spots, **not a complete forced text-backing mask**.

The raw forced text test is **not qualified**: forced-color system text overrides
the temporary transparent text fill, so the supposed backing PNG still has
black glyphs. Its reported 1:1 is glyph-versus-glyph, not invisible text. The
actual Wall forced text-backing and focus PNGs are byte-identical
(`9bac7f25c85b284ef1eab1c2b4506c93e9d413597e6f58726e6f8808496d5997`),
which independently demonstrates the ineffective mask. The normal focus PNG
is `4d829241f3f8af62f8250442cb74f70f236120b8af9955a8a2ec22d1d222dd29`.
No raw file, raw exit code or failed assertion is relabeled as passing. Forced
colors and reduced motion are recorded active; colored SVG backgrounds remain
visible behind the system foreground. The SVG source contains no motion.

The author also directly viewed Meet's normal and forced focus PNGs to check
the reported distinction. Its summary outline loses the bottom and left edges;
that is a **real inherited finding**, not an alpha/mask false failure. Read-only
Git byte/hash comparison confirms `party-discovery.css`, `shell.css` and
`shell-chrome.css` are unchanged from `8972b0b6`; the existing
`.party-browse .meet-discovery { min-height: 0; overflow-y: auto; }` contains the
outline. It remains open for the foreground implementation owner. No Meet
layout/focus declaration, shared CSS, title-focus lease or checker is changed
by this asset author, and this separate finding is not presented as resolved.

The scope is actual production router/Auth/Phaser under controlled QA06
synthetic HTTP/C05/local admission. It does not establish live service,
physical device, admitted Wall editor, whole-screen target fidelity, complete
accessibility compliance or full FE32/FE34 acceptance. Me's scoped `native-02`
visual/focus/character disposition remains valid for its exact tested source;
later header/title work belongs to its separately assigned owner.

Fresh publication readback: `git fetch origin master` and
`git ls-remote origin refs/heads/master`, both exit **0**, report
`1722a81734947764996590b94a63aa001501df28`. Frozen candidate `cc20832a` and the
retained packet are verified ancestors. All three remote SVG bytes match this
author's final source hashes in the interface table. This inspection document
is handed to Root for publication; final documentation ancestry is checked
after its import, without an SL/QA approval gate.
