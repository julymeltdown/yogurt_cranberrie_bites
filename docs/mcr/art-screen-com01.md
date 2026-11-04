# ART-SCREEN-COM01 — Community background adoption

Status: **IN_PROGRESS** for the explicitly granted `MEET-FOCUS-VIS-01`
follow-up and final publication/readback. The original bounded Wall/background
owner disposition remains delivered in `35ce1d0a`. A new exact functional lease
adds only an inset offset for Meet's filter-summary focus ring. Source is ready;
the finite Root-only affected capture is **NOT_RUN**. The original full goal
still reports **BLOCKED** through actual tool readback and is retained unchanged;
new aligned concrete work proceeds without reset, false completion or an ACTIVE
claim. No whole QA06/QA07/FE21, live-flow, device or global accessibility
acceptance is implied.

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
and new pseudo-elements are prohibited. Completion requires implementation, validation, documented committed handoff,
publication and required fixes. The goal is blocked while the external evidence
below is unavailable; no outstanding requirement is waived.

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
| Wall surrounding body, current source-only revision | `/backgrounds/cute-wall-me-v1/wall-stationery.svg` | `18e6e4db2f2e6384c796453d6e63caedc11875e8` |

The three community SVGs are opaque 390×640 editable vector source. Current
Wall source is opaque 390×780; the original captured Wall was 390×180. The community surfaces use Root's granted `center`, `100% 100%`,
`no-repeat` mapping and matching solid fallbacks. Wall uses `center top` and
`100% auto` with its matching solid fill. Exact bytes and hashes are
in the [initial source evidence](evidence/art-screen-com01/source-preparation.json)
[community aspect-ratio fix evidence](evidence/art-screen-com01/asset-fit-fix.json),
and [current tall Wall source evidence](evidence/art-screen-com01/wall-tall-source.json).
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

## Retained original capture and external dependency

The original report and all eight owned Feed/Meet/Gifts/Wall PNGs are retained
in immutable Root Git artifact `16bf30c617995f87c89f94a13d9cc5f4c65d51b5`.
Direct `git show` reads independently matched every PNG's byte length and
SHA-256 against that report; the report hash remains
`59e0f2540a1fdc79f240230048385fda678c1029e9d3df3ab2a209320ae86da1`.
See [retention evidence](evidence/art-screen-com01/retained-native-01.json).
Temporary sparse-checkout materialization does not change these retained
bytes. No separate copy, reconstruction or additional capture was made here.

This receipt preserves the original pre-fix job and candidate; it does not
qualify the corrected seven-rule CSS, SVG fit correction or later shell paint.
The HTML-image comparison also leaves canvas/CSS-rendered character pixels
unproved. Root's single allocated workflow must supply corrected 390px results,
then 360/430px and computed contrast/focus/control evidence, with direct
character/media preservation checks. The owner will inspect that packet,
resolve concrete failures, commit the evidence and confirm Root publication.
No independent browser/build/output workflow is authorized in this worktree.
The same missing external evidence has persisted across multiple goal turns;
this bounded development goal is blocked rather than complete. Root's ongoing
integration work is not stopped by this status.

## Corrected native-02 owner disposition

The resumed worktree was restored from its archived attachment at exact
`cd6af94c06f91bfc92840ddf6c24cf200a16be51`, verified clean, then switched back to
its unclaimed `codex/mcr-art-screen-com01` branch. No Root packet, sparse pattern
or image was restored, copied or reconstructed by this developer. The canonical
Root packet was read in place. The stylesheet is unchanged.

- Immutable packet: `5186f11017185a040e4ccde1c59589e140563ef0`, under
  `docs/mcr/evidence/cozy-backgrounds-v1/native-02/` in Root's publication checkout.
- Frozen production/fixtures/assets: `8972b0b634035055eb023dbc2f786e4463b172e5`;
  runner `e237572719602da6a1a08b7fe0579b0e9c96da1f`;
  driver SHA-256 `58f553d9abfcfab1cac4e67ccf86c378493ec337d5c17f55749673957ba6dbde`.
- Raw report SHA-256:
  `7f34e10279212d5f5d01017cbd644f02a33bd8939393b30818326ab2a489395b`.
- Actual job `PANEL-CORRECTED-202611040733Z`, 2026-11-04
  16:38:12.062–16:40:02.112 KST, exit 0. All owned PNGs are width 360/390/430
  by height 844. The same candidate/fixture baseline policy remains as above.

All eight owned 390px before/after PNGs were directly inspected first, followed
by all eight 360px and all eight 430px PNGs, plus the supplied original collage.
The read-only [packet audit](evidence/art-screen-com01/verify-native-02.py)
independently checks their on-disk bytes against both immutable Git objects and
the report/manifest pins, paired controls/text/images/actual character records,
reported target bounds, overflow, source CSS and four asset pins. It exits 0;
[results](evidence/art-screen-com01/native-02-validation.json) preserve all 24
image pins and 12 pair summaries without duplicating images.

| Surface | Owner visual disposition at all three widths |
| --- | --- |
| Feed | PASS for captured background scope: full-width blush/ivory/lilac post/footer shapes and peach scallops across the lower body resolve both original defects |
| Meet | PASS for captured background scope: broad sage/peach upper field, quiet ivory middle and lilac lower clouds visibly soften the entire container |
| Gifts | PASS for captured background scope: full-width wrapping-paper rows and cloud-shaped lower body; truthful No photo placeholders and controls preserved |
| Wall | NEEDS_CORRECTION: top stationery is warm and the guard is truthful, but about 600px of flat lower cream is too sparse for the current playful brief |

The initial four-route pass receipt is superseded for Wall. Its asset owner
independently reported the same sparse lower field and is authoring a taller
390px-wide background with broad low-contrast lower cloud/bow shapes at the
same URL. Existing `center top`, `100% auto`, `no-repeat` and `#fff0de` mapping
remains suitable; no CSS ownership or source rights transfer occurred. New
source hashes/dimensions and a Root-owned actual 390px then 360/430px comparison
are required. `native-02` cannot qualify that future change.

Across the 12 owned pairs, all 189 recorded candidate controls have at least
44px width and height, with exact paired rect/font/color/state and text equality,
no reported page errors or horizontal overflow. Direct visible canvas RGBA,
dimensions and rectangles are identical: Feed has one canvas and Meet three
at every width. Gifts and guarded Wall explicitly have zero. All routes have
zero selected-static CSS spans and zero visible HTML images in these fixtures;
those zeros are not real-media or mural-editor qualification. The genuine
Wall guard and Meet unavailable-create state are unchanged.

The [paint inference](evidence/art-screen-com01/native-02-paint-inference.json)
recalculates the closed foreground/background bounds against exact current SVG
paints, whose colors are unchanged by the fit correction. Read-only RGB samples
from original candidate PNGs independently show ivory middle and peach/lilac
lower fields for the three Community routes. Wall's sampled lower points are
all `#fff0de`, corroborating the visual finding. Source-based foreground/focus
bounds remain an inference, not measured browser-computed text or ring contrast.
See [visual findings](evidence/art-screen-com01/owner-native-02.json).

The actual report's `metrics.paints.color` field contains `backgroundColor`,
not foreground text color. Own routes record only the first Tab's solid outline
style; screenshots precede that focus check. Me-specific focus/forced-colors
measurements cannot certify these four routes. The prior controls query omits
`details/summary` elements. Root received a bounded follow-up request for only
one representative real target per route at 390px, including the omitted
summaries, actual text/outline/backing measurements and forced-colors readability.
The same synthetic admission and truthful guarded Wall must remain. No worker
browser/native/build/test matrix was started. All core contexts already used
reduced motion; normal-motion equivalence and physical devices are not claimed.

Fresh `git ls-remote` observed published master
`39c68d1dcd85a618ec0c56287d8451c6ffe13dbd`. Both frozen production `8972b0b6`
and packet `5186f110` are its ancestors (`git merge-base --is-ancestor`, each
exit 0). Production CSS matches the local seven-rule source exactly, SHA-256
`d26dd8fbee928539f2689ccd9bb6269e4cb964c58b78e8cc86afbd894f228504`.
This proves publication of the inspected CSS and packet; it does not publish
the pending Wall correction, accessibility evidence or this new owner handoff.
Completion remains unfinished while those requirements are open.

## Closed tall Wall source consumption

Wall/Me owner delivered exact commit
`18e6e4db2f2e6384c796453d6e63caedc11875e8`, changing only their
`frontend/public/backgrounds/cute-wall-me-v1/wall-stationery.svg` and
`docs/mcr/art/cute-wall-me-backgrounds.md`. The handoff was read directly from
immutable Git. This developer restored only the archived exclusive checkout at
`f414d69586596b52d8c9b0d814514571f35b6ec8` and reclaimed the unclaimed original
branch, initially clean. The goal tool still reports **BLOCKED** with the same
full unfinished objective; no reset, budget or completion was introduced.

| Current source pin | Value |
| --- | --- |
| URL | `/backgrounds/cute-wall-me-v1/wall-stationery.svg` |
| Dimensions / viewBox | 390×780 / `0 0 390 780` |
| Bytes | 4,500 |
| SHA-256 | `22e2de672776682693bca71c57f501f2450bc9a28bc6314df44c26d6d193efc7` |
| Existing CSS | `center top`, `100% auto`, `no-repeat`, solid `#fff0de` |

The separate read-only
[source-consumption checker](evidence/art-screen-com01/verify-wall-tall-source.py)
exits 0 and records [closed results](evidence/art-screen-com01/wall-tall-source.json).
It independently verifies exact bytes/dimensions/viewBox, inert SVG tags and
resolved local references, all 25 handoff paints, retained original top geometry/
paints/definitions and child order, and new lower art after the original children.
Only the full cream/dot base paths extend from height 180 to 780; original top
ornament geometry is unchanged. New broad forms start at SVG y=258. No source
art is copied into this branch, and no CSS declaration changes.

Source contrast is explicitly an inference. The minimum individual paint ratio
for the existing closed default body assumption `#201d1d` remains 7.362:1;
the artist's `#211d19` assumption gives 7.364:1. A stricter per-channel envelope
`#c6a58b` bounds paint mixtures and still exceeds 4.5:1 for either dark assumption
and 3:1 for the existing closed focus-color assumption. These calculations do
not establish the runtime foreground, composed text/ring contrast, clipping,
forced-color behavior or ancestor opacity.

Using only the frozen native-02 body widths 342/372/412, the unchanged mapping
projects the new ribbon center to screenshot y=555.97/599.74/658.10 at viewport
360/390/430. Those positions lie inside the measured old body. This arithmetic
supports compatibility; it is not a new capture or proof that all new artwork
is visible and acceptable. Fresh Root rendering must establish that the lower
field meets the current broad playful requirement at every width.

The old `native-02` report, image/source pins, checker and Wall
NEEDS_CORRECTION disposition remain unchanged and apply to old 180px artwork
`1929fda18afe73000f79a44a7cb37c5504bee42d450bff343a562baa85e2e227` only.
No fresh visual pass is claimed for `22e2de67`. Root owns integration, the
bounded Wall 390px then 360/430px evidence, the already specified short own-route
accessibility follow-up and publication. Owner inspection/fixes and exact
published source/doc ancestry remain required. No browser, build, matrix,
packet copy/materialization or sparse operation was performed for this handoff.

## Final targeted Wall/Community packet `wall-com-03`

The current owner worktree was restored only from its archived attachment at
`58cd06e5d582271ae735b00bc4ed47485fb422bb`, verified clean, then switched to the
unclaimed original `codex/mcr-art-screen-com01` branch. No packet file, sparse
pattern, application asset or stylesheet was written. Community SL
`01a1a1fe-c5f1-7e60-87c2-43785b07f941` now owns coordination of this handoff;
Root153 retains sole publication/capture custody.

| Pin / outcome | Exact value |
| --- | --- |
| Published immutable packet | `1722a81734947764996590b94a63aa001501df28` |
| Canonical directory | `docs/mcr/evidence/cozy-backgrounds-v1/wall-com-03/` in Root's publisher checkout |
| Frozen production/fixtures | `cc20832a897f18cd83482dcd21b6c2fc1d226942` |
| Raw report SHA-256 | `8056cea82655a3ae95e6520952a351890a3eb921953262378ebf260ca89b4c5f` |
| Capture manifest SHA-256 | `4b1b8bf2d7a29678d072f6073c37e0a43ce646beb3a0cb99766275b2c952e61e` |
| Qualification SHA-256 | `f0ac612f201d43863086039a0284917f0cd9a6677fad5fe0b62a107d258c8301` |
| Actual job | `ROOT-WALL-COM-A11Y-202611040820Z`; 2026-11-04 17:17:02.026–17:17:31.969 KST |
| Raw browser command | **exit 1**, **OPEN_FAILURES**, preserved unchanged |
| Resource release | 17:17:32.004653 KST; guard reports 30.390451 seconds, owned link/temp absent and browser/server cleanup complete |

Both modes use exactly the same frozen production and controlled fixtures.
The baseline replaces **only** Wall SVG with the old 180px `8972b0b6` bytes.
Candidate Wall is exact tall source `18e6e4db` / SHA-256 `22e2de67…`. This is
actual default router/Auth/Phaser under controlled synthetic HTTP/C05/local
admission. Wall remains guarded; no editor was mounted to obtain the evidence.

The owner directly viewed all 25 original PNGs: Wall 390px before/after first,
then 360/430px, all four representative normal/forced focused views, ordinary
Community views and normal/forced text-mask views. The lower Wall field now
contains broad blush/sage/lilac clouds, a bow and music details, visibly warmer,
softer and more playful than both the prior sparse paper and supplied collage.
The quiet cream center and original guard remain readable. At 430px the natural
image continues below the existing viewport, while its broad lower clouds/bow
remain visible. No layout, crop, control, character or CSS geometry adjustment
is needed. Owner background disposition is **PASS for the bounded captured
scope**, with the inherited functional issue below explicitly retained.

The separate read-only
[retained-evidence audit](evidence/art-screen-com01/verify-wall-com-03.py)
exits 0. It independently verifies all 32 manifest artifacts against disk,
manifest and immutable Git; all 25 PNGs; 29 relevant production/fixture/CSS/
asset pins; paired Wall foreground/geometry/guard equality at each width;
actual Community character-record continuity from native-02; recorded target
bounds; normal text contrast raster arithmetic; forced alpha-composition and
actual unmasked glyph/backplate spots. Root's reported total of 725 pins is
retained as Root evidence rather than represented as this owner's full-source
audit. Physical nested CSS pins are clean-checkout evidence, not an individual
Vite load receipt. See [audit results](evidence/art-screen-com01/wall-com-03-validation.json)
and [owner disposition](evidence/art-screen-com01/owner-wall-com-03.json).

### Representative accessibility results and limits

| Route | Normal text minimum | Normal focus minimum | Forced ring minimum | Target size |
| --- | --- | --- | --- | --- |
| Wall guard / real header back control | 9.546:1 | 15.114:1 | 11.310:1 | 44×44 |
| Feed copy / options summary | 15.623:1 | 6.020:1 | 9.377:1 | 44×44 |
| Meet unavailable-member text and Refresh / filter summary | 5.499:1 | **1:1 clipped bottom/left** | **1:1 clipped bottom/left** | 301.546875×44 |
| Gifts footnote / filter summary | 7.464:1 | 6.586:1 | 10.678:1 | 360×44 |

Normal text ratios use the recorded actual opaque foreground and composed
backing PNG, with every pixel inside each selected text-range fragment box
recalculated independently. Ancestor opacity is 1, filter is none and blend is
normal for those targets. This is selected visible text evidence, not every
string/state or a global text audit. Normal/forced target geometry and recorded
application state restore exactly after the temporary measurement styles.

The raw forced-text check is invalid: forced colors overrides the temporary
transparent text-fill mask, leaving actual black glyphs in the purported
backing PNG. Comparing black foreground with those glyph pixels yields a false
1:1 backing result. The unmasked focused original PNGs independently contain
the qualification's recorded black glyph-core and white text-backplate spots
(21:1), with computed system-black text. Those per-fragment spots and readable
views are bounded evidence; the complete forced backing mask remains unqualified.
Art stays colored in this browser and is not claimed to disappear.

The raw forced-outline assertion ignores alpha in system outline
`rgba(5,0,73,0.8)`. Independent composition over each recorded backing, checked
against actual retained focused PNG pixels with one-channel tolerance, confirms
all four perimeter samples for Wall, Feed and Gifts. Their sampled minima are
shown above. That correction does not fix Meet's genuinely absent ring pixels.
No original failed command or report is relabeled as passed. Reduced-motion
and forced-colors media were active for the forced views; normal contexts also
request reduced motion. No all-motion, all-focus or physical-device claim is made.

### Inherited Meet finding routed outside the background lease

`COM01-INHERITED-MEET-SUMMARY-CLIP` remains **OPEN**. Actual normal and forced
images show top/right outline only on `.party-filters > summary`; the left and
bottom are clipped by `.meet-discovery` with unchanged `overflow:auto`. The
ancestor rectangle is `(15,115,360,394.5625)` and summary target is
`(15,465.5625,301.546875,44)`. The missing sampled pixels equal their backing,
with 1:1 contrast. This is a real focus limitation, separate from invalid forced
text masking and the alpha-comparison error.

Independent Git comparison confirms `party-discovery.css`, `shell.css` and
`shell-chrome.css` are byte-identical between `8972b0b6` and `cc20832a`, matching
the qualification's hashes. The exact seven-rule background CSS also remains
unchanged (`d26dd8fb…`). The finding is inherited, not a new background or tall
Wall regression. It was sent directly to Community's functional owner and Root
for separate functional ownership. No layout, focus geometry, title/header,
new selector or shared-file lease was acquired here. `TITLE-FOCUS-01` belongs
to ID01/UI ownership. This bounded background disposition does not close either
functional issue or create a new approval gate around it.

### Delivery boundary

All assigned background source fixes are implemented and validated in their
relevant retained packets. The source art and existing CSS require no further
owned fix from this packet. Native-02's old Wall pins and NEEDS_CORRECTION remain
historical and unchanged; new evidence separately qualifies the tall correction.
Real media, live services, device behavior, mural-editor pixels, every IA entry
and whole-project acceptance remain outside this bounded owner disposition.
The full original goal remains unfinished until this exact committed handoff
is published by Root and its owned bytes/ancestry are independently read back.

## Explicit `MEET-FOCUS-VIS-01` functional follow-up

Community SL received and recorded Root's exact functional lease before writes.
This is a separately authorized QA07 focus-visibility follow-up on the FE21 Meet
surface, retaining the QA06-owned background disposition; it is not an additional
original task or implicit layout authority. Existing `party-discovery.css` may
receive only:

```css
.party-browse .party-filters > summary:focus-visible { outline-offset: -3px; }
```

The new selector is explicitly granted only for this functional fix. Existing
COM documentation/evidence paths remain writable. No `.meet-discovery` overflow,
outline color/style/width, target rectangle, typography, text, padding, scroll,
layout, avatar, art/background, header/title, shared CSS or other feature may
change. The optional new test subtree is unused: no CSS-mirror unit test, build,
lint, browser, native matrix or output job is introduced.

Before writes, actual `get_goal` returned thread identifier
`01a1a533-50a9-73a1-8d4f-a16bf0b69aee`, `createdAt=1793772982`, status
**BLOCKED**, and the original complete COM01 objective. The tool exposes no
separate goal ID. No objective/reset/status/budget mutation was performed.
Actual checkout was clean on the sole claimed `codex/mcr-art-screen-com01`
worktree at `35ce1d0a8861ed4ba522d7dfdd25020b41c86a01`; common integration base
was `7c25078f118da53dd9a524d0a58e707cca190f0f`. The exact acceptance, readback and
scope were sent to Community and Root before implementation.

Read sources included the original specification's mobile/44px/360-390-430/
reduced-motion requirements (§5.1), FE21/QA06/QA07 criteria, current project
assessment/execution/working agreements, actual owner report `wall-com-03`, and
existing feature CSS. The retained actual summary target is
`(15,465.5625,301.546875,44)`, with a solid 3px `#a3212b` outline at +2px offset.
A -3px offset puts the ring's outer edge on the same border box and all 3px of
paint inward. The inner rectangle is 295.546875×38. This confirms the source
geometry needed to take the exact grant, without claiming rendered proof.

The patch is one appended rule; all 9,041 original local CSS bytes remain
unchanged. Local source SHA-256 changes from
`264b4c5304ada1419b46f73d3cdb0168505234a0d308289560967a495d801350`
to `366664ac1c5a4a538173209656e59064c55612b3db880d968e40c481226a7c91`.
The local branch predates already delivered peer Meet typography: frozen
production CSS is 9,066 bytes, SHA-256
`0f4b94ffb12cf8890d6e7c4e258e69b68833287dca0e8ad275a69d0700ebdea8`.
Root must preserve that current production preimage and integrate only the
append-only rule; this handoff does not replace the entire production file.
[Source-bound evidence](evidence/art-screen-com01/meet-focus-vis-01-source.json)
records exact preimages, scope, arithmetic and unfinished validation.

Source boundary and diff checks exit 0. Runtime remains **NOT_RUN** for this new
rule. Root/21d alone coordinate the finite affected capture: Meet focused and
unfocused at 360/390/430 plus 390px forced colors. The owner must inspect actual
retained images and measurements for all four ring sides/contrast, unchanged
rectangles/fonts/text/scroll/layout, characters/backgrounds and control states,
resolve concrete failures inside this lease, then complete final committed
disposition and independent remote byte/ancestry readback. No source-only
calculation closes the inherited finding or original full goal.
