# Wall and Me background artwork — 2026-11-04

Status: **IN_PROGRESS**. Editable source and application adoption are present in
candidate `c8a731e5`; the actual same-state 390px Wall/Me pairs were inspected.
The original three-SVG pack is published on `origin/master` at `c8a731e5`.
The later background-only Me focus-contrast fix below is source-ready and still
requires integration, fresh rendered evidence and publication.
360/430px and additional accessibility checks, the later Me outer-paper capture,
and publication of final documentation remain required. This document does not
accept FE32, FE34, IN04, or mobile release gates.

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
| `wall-stationery.svg` | 390×180 / `0 0 390 180` | 2639 | `1929fda18afe73000f79a44a7cb37c5504bee42d450bff343a562baa85e2e227` |
| `me-scrapbook.svg` | 390×240 / `0 0 390 240` | 2728 | `98c351c141e372d8d7e0d6879556105c551fbdf7478d66afe76d360a59a7aae1` |
| `me-night-ribbon.svg` (focus-background revision) | 390×180 / `0 0 390 180` | 2298 | `29ad1cb9d04101399e2483fef9de6a3d710c6bf7f08cb1cb4501fb746152932d` |

| Asset | Background role and proposed use, subject to Root's exact grant |
| --- | --- |
| Wall stationery | Broad cream `#fff0de`, peach scallops `#f3c9bd`, lilac `#e5d8e8` and sage clouds `#dce3cd`. COM01's guarded Wall panel-body surround only; never the canvas, tool stack, pixel grid or swatches. Prefer matching solid background color plus a single image sized to the surface width. The tile has an intentional scallop/wave edge; it is **not seamless**. Do not stretch it into unrelated UI or repeat it densely. The matching solid fill covers tall exposed paper without returning to the old dark field. |
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
with `8ff3a58f`: exact bytes match. Wall and Me paper still match the current
asset-interface hashes above. This first capture used original header SHA-256
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
