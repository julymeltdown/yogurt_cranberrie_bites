# ART-SCREEN-COM01 — Community background adoption

Status: **IN_PROGRESS**. Background source adoption and the first actual 390px
comparison are implemented. Feed/Gifts' SVG aspect-ratio fix is source-verified
and requires fresh rendered validation.
Responsive checks, complete rendered contrast, publication and final visual
acceptance remain open.

## Authority and baseline

Root153 granted the following new paths to developer
`01a1a533-50a9-73a1-8d4f-a16bf0b69aee` before writes:

- `frontend/src/shared/styles/cozy-community-backgrounds-v1.css`
- `docs/mcr/art-screen-com01.md`
- `docs/mcr/evidence/art-screen-com01/**`

The exclusive worktree is `/Users/lhs/.codex/worktrees/06af/mcr`; branch
`codex/mcr-art-screen-com01` starts at
`7c25078f118da53dd9a524d0a58e707cca190f0f`. All three granted paths were absent
from that baseline. Root retains stylesheet imports, shell composition, the
Wall route attribute and master publication. Social and Community functional
leads recorded no conflicting claim to this new stylesheet.

Allowed declarations are `background-color`, `background-image`,
`background-position`, `background-size`, `background-repeat` and
`background-blend-mode`. Exact allowed surfaces:

| IA | Selector | Boundary |
| --- | --- | --- |
| F01 | `.feed-browse .feed-post` | Post paper only; exclude portraits and media |
| F01 | `.feed-browse .feed-footer` | Footer paper only; preserve all controls |
| F01 | `.shell-panel[data-mcr-panel='feed'] > .shell-panel-body` | Exact `/feed` list route; surrounding blank paper only |
| M01 | `.party-browse` | Container background only; existing Meet typography belongs to its owner |
| G01 | `.gifts-reference .gift-reference-row` | Row background only; exclude item images and actions |
| G01 | `.shell-panel[data-mcr-panel='gifts'] > .shell-panel-body` | Exact `/gifts` list route; surrounding blank paper only |
| R01 | `.shell-panel[data-mcr-panel='wall'] > .shell-panel-body` | Surrounding paper only; Root supplies the semantic attribute |

Root explicitly permits a `:root` specificity prefix on these exact selectors
so lazy feature stylesheets cannot overwrite the background paint. This adds
no new target surface. No `!important` declarations are used.

No foreground, type, layout, control, disabled-state, character, crop, avatar,
media, mural canvas, pixel, palette, editor-tool or shared-token changes are
authorized. No existing source file is writable here. Decorative left rails
and new pseudo-elements are prohibited. The persistent goal remains active
through implementation, validation, documented committed handoff, publication
and required fixes.

## Art direction and source provenance

The human's current direction requires backgrounds that look visibly cuter
than both the existing application and supplied collage at a glance: broad,
warm, soft, playful paper and shapes, with quiet text regions. Tiny ornaments
over an otherwise unchanged dark distressed field do not meet this criterion.
Characters, authentic content, guards and the ten-screen structure stay intact.

The human reference is the original 1280×853 JPEG, 482737 bytes, SHA-256
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`,
preserved at
`docs/mcr/evidence/visual/qa06/human-reference-20261104/reference-human.jpg`.
It is context for structure and the superseded dark treatment, not permission
to copy its character or mural pixels.

Community artist `01a1a534-4fcf-7291-abe3-a97ba2fcafcd` owns
`frontend/public/backgrounds/cute-community-v1/**`. Wall/Me artist
`01a1a534-4fd0-7ba1-8fe8-4cc95674080f` owns
`frontend/public/backgrounds/cute-wall-me-v1/**`. This developer consumes their
approved closed assets and does not author or modify those files. Root confirmed
the exact broad URL mapping and permits direct use of the closed owned pack
without a further per-URL permission flow. Source commits and usage:

| Surface | URL | Source commit |
| --- | --- | --- |
| Feed post/footer | `/backgrounds/cute-community-v1/feed-backdrop.svg` | `124f3ed8217acfff33ffcc7904c6ff000e33b013` |
| Meet container | `/backgrounds/cute-community-v1/meet-backdrop.svg` | `124f3ed8217acfff33ffcc7904c6ff000e33b013` |
| Gifts rows | `/backgrounds/cute-community-v1/gifts-backdrop.svg` | `124f3ed8217acfff33ffcc7904c6ff000e33b013` |
| Wall surrounding body | `/backgrounds/cute-wall-me-v1/wall-stationery.svg` | `8ff3a58f556bf2811ad1415cd31c5ba1953ccd58` |

The three community SVGs are opaque 390×640 editable vector source; Wall is
opaque 390×180. The community surfaces use Root's granted `center`, `100% 100%`,
`no-repeat` mapping and matching solid fallbacks. Wall uses `center top` and
`100% auto` with its matching solid fill. Exact bytes and hashes are
in the [initial source evidence](evidence/art-screen-com01/source-preparation.json)
and [current aspect-ratio fix evidence](evidence/art-screen-com01/asset-fit-fix.json).
No old artwork, fonts or dependencies are copied into this task. Asset files
are integrated separately by Root and are absent from this patch.

Wall preserves its intrinsic aspect ratio over a matching tall solid fill,
as its artist recommends and Root explicitly grants. The combined 390px capture
must still expose any hidden ornament or inadequate dominant treatment; source
readiness does not establish acceptable visual adoption.

## Historical captures and their limits

The immutable index at baseline `bfa0193939c3eebd6c80ade85083c6530c6875e1`
is `docs/mcr/evidence/visual/qa06/human-reference-20261104/comparison.json`.
These captures provide historical context; they are not same-candidate before
evidence for the new stylesheet:

| Screen | Capture under `docs/mcr/evidence/visual/qa06/` | Source and qualification |
| --- | --- | --- |
| Feed | `postfix-12a5-repairs-tail/feed-parade-390-h631.png` | 390×631, source `12a5d394eeabc89e1c7b0012fb51038f737c9a7f`; synthetic HTTP/C05, one text-only post |
| Meet | `postfix-12a5-repairs-tail/meet-parade-390-h637.png` | 390×637, same source; three synthetic parties, truthful unavailable member images/create capability |
| Gifts | `fidelity-v3/equal-gifts-390.png` | 390×608; application `980fc4d85020f297b36fee8195e1ca92079ff33f` with corrected fixture harness `59f88f9c1d8d3b1aa4568581b8c496981208e371` |
| Wall | `fidelity-v3/equal-wall-390.png` | 390×639, same older cohort; actual unavailable guard, not a qualified editor |

The original matrix-v3 390px Feed/Meet/Gifts/Wall captures were directly
inspected during read-only preparation. Their old source, synthetic content
and unavailable Wall cannot establish current integration acceptance.
The four newer historical captures listed above were also directly inspected;
the Wall screenshot still proves only its unavailable state.

## Required validation and delivery

1. Pin delivered asset bytes and inspect the closed SVG source for external
   resources, scripts, text/characters and expected quiet regions.
2. Check the stylesheet's selectors and declarations against the exact grant;
   preserve all original character/feature source bytes.
3. Under coordinator21d's explicit exclusive slot, Root captures actual mounted
   components before/after at 390px in identical states. The latest scheduling
   correction allows ready panel/entry comparisons independently of the
   World/Map pack after the active artist releases its output slot. No worker
   starts an individual browser job. Capture 360/430px only after inspecting
   the 390px result; an initial staged packet does not waive those checks.
4. Verify broad visual improvement, computed contrast, visible focus, unchanged
   typography/control bounds, no horizontal overflow, truthful unavailable and
   disabled states, and no change to avatars, media or mural pixels. Keep
   desktop viewport evidence distinct from actual mobile-device acceptance.
5. Commit only the granted paths; hand off exact base, SHA, changed paths,
   commands/exit codes, evidence and limits to Root. Verify published ancestry
   after Root publication and address concrete fixes before goal completion.

This developer starts no separate browser, native, build or export workflow.
Root's one allocated shared capture has now exercised the semantic Wall hook
and imports in the actual default router. Real multiuser flows, physical
devices, and release gates remain separate obligations.

## Committed handoff and source checks

Source handoff `8824401dec061102c9af33ba302855e48563170a` contains only the
new stylesheet, this document and source evidence. Root and both asset authors
received the handoff. Remote master was rechecked after handoff and was still
`7c25078f118da53dd9a524d0a58e707cca190f0f`; publication ancestry is not yet
proved.

| Check | Result |
| --- | --- |
| `python3 scripts/mcr/check-plan.py` | Exit 0; planning provenance/structure only |
| Exact selector/property/URL boundary and source asset hashes/dimensions | Exit 0; five granted surface rules, four distinct pinned SVGs |
| SVG XML and closed-resource audit | Exit 0; no script, raster, filter, animation, text rendering or external resources |
| `git diff --cached --check` before source commit | Exit 0 |
| Source-paint contrast bound | Exit 0; [evidence](evidence/art-screen-com01/source-contrast.json) |
| Actual browser-computed contrast, focus and geometry | NOT_RUN; combined Root capture pending |

The conservative source bound treats every SVG hex paint as fully opaque,
including faint ornaments. Minimum body-text ratios are Feed 12.32:1,
Meet 12.287:1, Gifts 12.089:1 with the default ink token, and Wall 7.362:1.
Meet's existing hint ink has a 4.882:1 bound; the lowest focus-color bound is
Wall 3.286:1. This calculation does not establish actual theme inheritance,
rendered text contrast, control contrast, visual acceptance or device behavior.

## Actual first 390px owner inspection

Root's shared job `PANEL-COZY-202611040632Z` completed with exit 0 on
`c8a731e5c7f03c3fbcbd0581c6e872429806fa22`. The capture ran from
2026-11-04 15:33:32.440 to 15:34:13.328 KST in desktop Chrome 156.0.8078.80.
The same candidate/fixtures were used for both modes; the baseline restored
only four paint-integration files to exact `7c25078f` bytes. Meet typography
was identical in both modes. This is deterministic QA06 synthetic HTTP/C05 and
local admission through the actual default router, not live service or device
qualification.

All eight owned PNGs were opened and visually inspected. Their hashes/bytes,
the report hash, observed findings and remaining checks are recorded in
[owner evidence](evidence/art-screen-com01/owner-390-native-01.json).

- Feed and Gifts need a concrete fix: their short row surfaces show a narrow
  central vertical strip of artwork over the matching pink/lilac fallback.
  SVG's default `preserveAspectRatio` letterboxes the 390×640 source within the
  short `100% 100%` background viewport. The intended broad cloud shapes do not
  span those rows. This was routed directly to the asset owner and Root;
  no asset edits or additional browser job were made here.
- Meet shows a visibly broader, softer cream/peach/lilac container background
  while preserving the three party rows and truthful unavailable create state.
- Wall shows the warm scalloped stationery around the same unavailable guard.
  No mural editor was mounted or substituted to produce this view.

The driver recorded no page errors or horizontal overflow on these four routes.
For each pair, controls retained rect/font/color/state, text remained exact and
the visible image source/rect/RGBA comparison passed. Browser/server cleanup and
temporary removal passed. These observations do not prove complete focus,
44px target, computed contrast, responsive, physical-device or live-flow gates.
360/430px were not run in this first finite job. The later shared213 fourth
paint is also outside this packet's candidate and needs its own relevant proof.

## Community asset fit correction

The asset owner delivered `124f3ed8217acfff33ffcc7904c6ff000e33b013` after
independently inspecting the same first capture. It adds
`preserveAspectRatio="none"` to the three broad backdrop SVG roots. Direct Git
byte comparison proves that removing this one attribute recovers each original
SVG byte for byte: geometry, colors, opacity, dimensions and resources are
unchanged. The manifest's new hashes/bytes/viewBoxes match the actual files.
This deliberately scales only decorative background shapes to the granted
paint surface; no character, layout or object geometry is changed.

The CSS and URLs remain unchanged. Root integrates the separate asset commit;
this branch owns only the updated provenance and evidence. Existing source
contrast bounds remain applicable because paint colors are byte-identical.
The old `native-01` images remain pre-fix evidence. Fresh 390px inspection,
360/430px rendering and computed accessibility checks are still required.

## Dominant list-route paper correction

The first rendered packet also exposed large untouched paper regions below the
single Feed post and below Gifts' available content. The card-only initial
scope could not establish the requested dominant at-a-glance change across
those screens. Root explicitly extended the grant to the two exact list-route
body selectors above before writes. They consume the same fixed Feed/Gifts
backdrops, `100% 100%`, `center`, `no-repeat`, with their matching opaque fills.
All original row/footer rules remain. Detail, new-post/giveaway, reservation and
pickup routes do not receive these body backgrounds.

Root owns adding `feed` and `gifts` values to the existing main's semantic
attribute only for exact `/feed` and `/gifts`, while retaining Wall and the
other assigned developer's Customize scope. This changes no DOM structure,
layout, list/scroll geometry, type, controls or character pixels. The seven-rule
source patch is validated separately; the old five-rule first packet cannot
prove the new body coverage. One Root follow-up must inspect fixed 390px first,
then 360/430px, computed contrast/focus/44px behavior and preservation evidence.
