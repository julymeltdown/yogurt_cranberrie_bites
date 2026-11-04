# Wall and Me background artwork — 2026-11-04

Status: **IN_PROGRESS**. Editable source is ready; application adoption, actual
same-state 390px before/after, 360/430px checks and master publication remain
required. This document does not accept FE32, FE34, IN04, or mobile release gates.

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
| `me-night-ribbon.svg` | 390×180 / `0 0 390 180` | 1895 | `1519b1b125190be901e1119105eafd057532ff98d1f091cf450f0f31f72b62bc` |

| Asset | Background role and proposed use, subject to Root's exact grant |
| --- | --- |
| Wall stationery | Broad cream `#fff0de`, peach scallops `#f3c9bd`, lilac `#e5d8e8` and sage clouds `#dce3cd`. COM01's guarded Wall panel-body surround only; never the canvas, tool stack, pixel grid or swatches. Prefer matching solid background color plus a single image sized to the surface width. The tile has an intentional scallop/wave edge; it is **not seamless**. Do not stretch it into unrelated UI or repeat it densely. The matching solid fill covers tall exposed paper without returning to the old dark field. |
| Me scrapbook | Broad blush cream `#fff0e7`, rose scallop `#efc3c9`, lilac `#ead9ea`, sage `#dce5d0`. ID01 may propose `.me-hub--reference .me-bio` and `.me-activity-navigation` background paint; no Me body grant is implied. Keep existing opaque controls. At small bio heights the broad warm fill and upper scallop remain visible while lower clouds may be clipped. |
| Me night ribbon | Warm berry `#573344`, broad mauve clouds `#6c4355` / `#75495d`, quilted hearts and a large soft bow. ID01's proposed `.me-hub--reference .me-profile-header` background only, preserving its existing light text. The existing opaque avatar container and every character pixel remain unchanged. It is not a portrait frame or character background asset. |

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
| WCAG relative-luminance calculation | Dark `#211d19` on rose `#efc3c9`: 10.62:1; on peach `#f3c9bd`: 11.08:1; on lilac `#e5d8e8`: 12.21:1. Light `#f5ecdc` on the brightest berry ornament color `#875a69`: 4.85:1; on berry base: 9.13:1. Opacity blends those ornaments toward the darker base. These are source color checks, not a complete rendered accessibility audit. |
| `git diff --check` | Exit 0. |
| Browser decode / actual application capture / width checks | **NOT_RUN**; wait for the one consumer-owned capture set, not a parallel fixture rendering. |
| Master publication / delivered ancestry | **NOT_RUN**; Root153 is the publishing owner. |

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

Neither consumer source commit proves publication, image decoding, same-state
geometry/pixel equivalence, control usability or the requested at-a-glance
visual improvement. Those obligations remain open for the combined candidate.
