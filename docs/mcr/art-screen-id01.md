# ART-SCREEN-ID01 — identity background adoption

Status: **INTEGRATED**. The four-screen background implementation and scoped
visual checks pass on frozen production `8972b0b6` at 360/390/430px. The final
owned evidence handoff and its publication receipt remain to be delivered.
This does not certify full IA, live-service, device or release acceptance.

## Ownership and baseline

The sole CSS adoption owner is chat `01a1a533-50a8-7fb2-b3ed-96b941b96bbf`,
working in `/Users/lhs/.codex/worktrees/2251/mcr` on
`codex/mcr-art-screen-id01`. The initial read-only base was
`bfa0193939c3eebd6c80ade85083c6530c6875e1`. A fresh fetch confirmed publication
`7c25078f118da53dd9a524d0a58e707cca190f0f`, the implementation branch base.
The new source, document, and evidence prefixes were all absent before work.

Root153 granted these exact paths before any source write:

- `frontend/src/shared/styles/cozy-identity-backgrounds-v1.css`
- `docs/mcr/art-screen-id01.md`
- `docs/mcr/evidence/art-screen-id01/**`

The initial source grant covers background paint on A01
`.public-entry--poster`, its `.public-poster` and `.public-header`; A02
`.auth-sign-in .concert-window` and its existing `::before`; and U01
`.me-hub--reference .me-profile-header`, `.me-bio`, and
`.me-activity-navigation`. Background color, image, position, size, repeat,
and blend declarations are the allowed initial properties.
Root subsequently granted the exact decorative payload selectors below before
they were changed. This is a narrow extension beyond the initial grant.
World subsequently recorded functional no-claim and Root granted U02 before
source writes: only direct `.concert-window__body` and
`.concert-window__footer` children of the direct `.concert-window` under
`.mcr-motion-editor`, `.mcr-whole-editor`, and `.mcr-avatar-editor`. This is
surrounding paper only; preview/figure/choice/canvas selectors remain excluded.
Root also approved the exact proposed Entry and Me asset URLs for background
image adoption after a closed source handoff and matching provenance pins.
Those closed handoffs have now been inspected and adopted.

Root alone owns imports, root composition, and master publication. The
implementation owner chooses validation, fixes failures, and supplies a
committed handoff. There is no SL or king technical review gate.

## Asset sources and preserved behavior

Entry assets are authored separately by chat
`01a1a534-4fcf-7291-abe3-a9af7b895a45` under
`frontend/public/backgrounds/cute-entry-v1/`. Me assets are authored by chat
`01a1a534-4fd0-7ba1-8fe8-4cc95674080f` under
`frontend/public/backgrounds/cute-wall-me-v1/`. This owner has no asset write
rights. Entry source revision is
`9a962ea6f8c20fe59846cbcf4f468b902e3f280e`; current Me source revision is
`77d8e2608a28286bcce18df000eeea4069c826d1`, superseding the original
`8ff3a58f556bf2811ad1415cd31c5ba1953ccd58` header art. The Entry manifest and exact Git
asset bytes were read before adoption. All seven used asset hashes and byte
sizes are recorded in `evidence/art-screen-id01/source-validation.json`.
They are original inert SVGs without raster/character/font/script/filter or
external resource dependencies. Assets themselves are integrated by Root;
this worker's commit contains CSS and its own documents/evidence only.

| Surface | URL after `/backgrounds/` | Changed paint |
| --- | --- | --- |
| A01 poster | `cute-entry-v1/landing-garden-night.svg` | Broad rose field, large moon/cloud backdrop; fixed cream wordmark/copy and crowd remain |
| A02 window | `cute-entry-v1/sign-in-paper.svg` | Vanilla paper with broad pastel side fields |
| A02 existing top/bottom decorative SVG boxes | `cute-entry-v1/sign-in-clouds-top.svg`, `cute-entry-v1/sign-in-garden-bottom.svg` | Background paint replaces only old cemetery image payload paint |
| U02 direct window body/footer in all three granted wrappers | `cute-entry-v1/customize-paper.svg` | Outer paper only; all preview/figure/choice/canvas descendants keep their original surfaces |
| U01 profile header | `cute-wall-me-v1/me-night-ribbon.svg` | Berry upper field and opaque cream lower shelf behind unchanged header links; original opaque portrait surface remains |
| U01 bio and navigation surround | `cute-wall-me-v1/me-scrapbook.svg` | Blush paper and soft broad ornament; opaque controls remain |

Every adopted image uses the granted `center`, `100% 100%`, `no-repeat`
paint properties and a matching solid fallback. The `:root` selector prefix
raises specificity without expanding matching surfaces, so later lazy feature
CSS with the original selectors cannot replace this paint. Root explicitly
allowed this specificity treatment. No `!important` is used.

Only `.public-entry--poster .landing-web > g`,
`.public-entry--poster .landing-scene > g`, and
`.auth-sign-in .auth-cemetery > image` receive `opacity: 0`, under Root's exact
additional grant. The SVG containers remain in their original flow, geometry,
z-index, focusability and hit-testing state. The wordmark, printed copy, entry
print, characters and every ancestor opacity remain unchanged. Source proof is
`PosterArtwork.tsx`'s separate `ReferenceLayer` SVGs and `Cemetery.tsx`'s sole
decorative image: both are `aria-hidden`, `focusable="false"`, with existing
`pointer-events: none`. These selectors are not generalized to other SVGs.

The current direction requires backgrounds that look warmer, softer, and
playful at a glance in actual screenshots. Small ornaments over the previous
dominant gothic surface do not establish that result. Background-only changes
must preserve all words, controls, typography, layout, route guards, true data,
focus outlines, touch targets, and character pixels, crops, framing, atlases,
renderers, and configuration. No decorative left accent line is allowed.

## Historical reference evidence

The exact human JPEG is preserved at
`docs/mcr/evidence/visual/qa06/human-reference-20261104/reference-human.jpg`,
SHA-256 `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
It is a reference, not a replacement UI or the revised background palette goal.
The frozen index at baseline `bfa01939` is
`docs/mcr/evidence/visual/qa06/human-reference-20261104/comparison.json`.

| Screen | Existing artifact under `docs/mcr/evidence/visual/qa06/` | Context |
| --- | --- | --- |
| A01 | `fidelity-v3/equal-landing-390.png` | 390×684; application `980fc4d8`, fixture `59f88f9c` |
| A02 | `fidelity-v3/equal-login-390.png` | 390×746; same older application and fixture cohort |
| U02 | `fidelity-v3/equal-customize-390.png` | 390×698 full older frame before portrait-responsive source |
| U02 | `human-reference-20261104/customize-portrait.png` | 350×685 component inside a 390×698 viewport; source `14d61ee7`; not a full-root 390px frame |
| U01 | `postfix-12a5-repairs-tail/me-parade-390-h595.png` | 390×595; source `12a5d394`; authorized activity unavailable |

These are historical, source-specific browser fixtures. They do not prove a
fresh current-master before state, a live service journey, physical device
behavior, or visual acceptance. The implementation owner has directly viewed
the human JPEG, A01/A02/U01 frames, and the later U02 component. Source drift
from the initial base through `7c25078f` includes the shared titlebar's
`border-bottom` change from solid to double; these historical captures must
not be relabeled as a new baseline capture.

## Validation and delivery

Repository status, full SHAs, prefix absence, relevant selectors, and the
decorative SVG source boundaries were inspected read-only with exit code 0.
The new CSS now contains twenty granted selectors and consumes all seven
closed assets. The static selector/property/URL mapping, asset byte/hash, and
inert SVG checks pass with exit code 0:

```sh
python3 docs/mcr/evidence/art-screen-id01/validate-source.py
git diff --check
```

`evidence/art-screen-id01/source-validation.json` pins the implemented source.
The earlier `source-preparation.json` remains a historical fallback-palette
checkpoint, not the final composed palette or acceptance evidence. The artists'
source color checks do not prove actual rendered contrast. In particular,
the landing's light moon/cloud band must avoid unchanged cream lettering;
the source author reports normalized wordmark y=27.2–242.2, light zone starting
y=258, and copy y=485.8–572.9 on dark lower paper. Rendered composition must
confirm those boundaries after CSS adoption.
No browser, build, export, or native job has been launched by this owner.
No dependency or package resolution changed.

Required remaining evidence:

- Closed asset handoff revisions and hashes, adopted source, and source scope checks.
- Root-owned import/composition and a coordinator21d workflow allocation after
  the first actual World/Map 390px cue.
- Same-state actual 390px before/after captures, then 360/430px captures.
- Rendered contrast, preserved geometry and character pixels, reachable 44px
  controls, focus, and relevant unavailable/pending/error states.
- Scoped committed handoff, required fixes, Root publication, and fresh remote
  verification of the actual delivered source.

## First integrated 390px evidence and required correction

The initial CSS handoff is `e44b5d1c38f05b03a74904c94004d5fc983b2493`.
Fresh fetch and independent remote-head lookup verified master publication
`c8a731e5c7f03c3fbcbd0581c6e872429806fa22`: this CSS is byte-identical,
all seven closed assets match exact bytes/hashes, and the Root-owned import
is present. This verifies publication of the first source increment.

Root's allocated `PANEL-COZY-202611040632Z` workflow produced the actual
390×844 eight-route before/after packet at
`docs/mcr/evidence/cozy-backgrounds-v1/native-01/`, from 06:33:32.440 to
06:34:13.328 UTC on 2026-11-04. The owner directly viewed all eight identity
PNGs and verified their report byte counts and SHA-256 values. The report
uses candidate `c8a731e5` for both cohorts, restoring only four paint
integration files to `7c25078f` for baseline; other candidate source and
Meet typography stay identical. It is actual default-router rendering with
QA06 synthetic HTTP/C05, not a live service or physical device check.

The owner's findings and exact artifact/report pins are recorded in
`evidence/art-screen-id01/owner-native-01.json`:

- Landing has a clear dominant rose/cloud/moon change. Printed wordmark and
  copy remain on darker fields; the selected crowd is visually unchanged.
- Sign in has broad pastel paper and garden scenery with readable live form
  labels and honest unavailable-provider copy.
- Me has a berry header and softer bio paper. The opaque approved portrait
  and unavailable activity state remain intact.
- Customize is **not visually closed**: the large blank region below the
  controls remains the original grain paper. Its actual `.shell-panel-body`
  is 372×780 at x=9, y=57; the initially granted editor body/footer rules do
  not paint this outer region.

Root granted the exact correction before writes:
`:root .shell-panel[data-mcr-panel='customize'] > .shell-panel-body`, only
background color/image/position/size/repeat using the same closed
`customize-paper.svg`. This worker added only that CSS rule. Root owns the
existing route-marker expression extension for `/me/avatar`; there is no new
wrapper, preview paint, geometry, control, character, or layout change. This
correction still requires fresh rendered evidence after integration.

Root then granted the equivalent exact Me outer-paper rule before writes:
`:root .shell-panel[data-mcr-panel='me'] > .shell-panel-body`. It uses only
background paint with the same pinned `me-scrapbook.svg`, `center bottom`,
`100% auto`, `no-repeat`, and solid `#fff0e7`. Root owns the existing
route-marker expression extension for exactly `/me`. This replaces the old
blank lower paper while keeping the original opaque portrait, all character
pixels, controls, layout, and footer unchanged. It also needs fresh rendered
evidence; the first packet does not include this rule.

The initial Me outer-paper placement was `center top`. The asset owner mapped
its 390×240 image to the actual 372×780 shell paper: it ends around y=286,
almost entirely behind the opaque header/navigation. Root granted the exact
one-property correction to `center bottom` before it was changed, exposing the
large cloud/scallop shapes in the lower blank area. Asset bytes, sizing and all
foreground geometry stay unchanged. Actual follow-up rendering still has to
confirm that placement.

The first packet reports unchanged control geometry/type/color/state and body
text, no errors, no horizontal overflow, and a visible focus outline for each
owned route. An independent calculation over its recorded controls finds all
8 Landing, 9 Sign in, 15 Customize, and 15 Me controls at least 44×44px in both
cohorts. This does not replace additional auth-state or full focus-return checks.

An important evidence limit: **all four routes have empty `metrics.images`
arrays**. The driver's `document.images` comparison does not cover the landing
CSS-background spans or editor/profile canvases. Its `imagesPreserved: true`
must not be presented as quantitative character RGBA proof. Character-source
freeze and direct visual comparison remain valid narrower evidence. Independent
read-only screenshot comparisons additionally show byte-identical RGBA in the
140×140 opaque Me portrait interior and the 348×322 Customize preview interior;
their coordinates and hashes are in the owner evidence JSON. These two crops do
not certify landing spans, antialiased edges, other widths, or corrected paint.
The owner
requested actual canvas RGBA/dimensions/rectangles and landing span resource/
position/size preservation in Root's next single allocated workflow.

The original 16 PNGs and report are retained in immutable commit
`16bf30c617995f87c89f94a13d9cc5f4c65d51b5`. Sparse checkout temporarily
dematerialized the PNGs from the shared working directory. Root restored the
exact original blobs and corrected the inclusion pattern. The owner then
independently verified all 16 physical PNGs against that commit, all eight owned
PNG pins, and the original report hash at the same recorded paths. This was
artifact restoration, with no replacement capture or changed cohort identity.

Read-only contrast spot checks on the retained candidate PNGs are recorded in
`evidence/art-screen-id01/native01-contrast-spots.json`, including exact glyph
and backing RGB values and pixel coordinates. Selected Me profile-name, bio
and activity-label samples measure 9.129:1, 15.042:1 and 10.043:1; Customize's
Random button and Characters heading measure 14.248:1 and 16.212:1. Disabled
Save measures **4.497:1**, which is not rounded into a 4.5:1 pass. It is disabled
in this captured state; the enabled state remains unqualified. These selected
samples do not certify all glyphs, antialiasing, focus, other states, corrected
outer paint or other widths. No foreground or asset change follows from them.

Browser/server closure and temp removal are recorded. A later shared fourth
paint correction is outside this packet's cohort. Required remaining work is
the integrated Customize correction, actual 360/430px evidence, rendered
contrast, additional relevant auth/focus states, quantitative character
invariance, any resulting fixes, final documentation and fresh remote proof.
The full four-screen persistent goal remains **ACTIVE**.

## Me header focus backing correction

The retained native01 report places the header at `(15,63,360,208.390625)` and
the four 48×44 icon links at x=81/141/201/261, y=220.390625. Existing
`me.css` gives these links a 3px `#a1212c` focus outline with 3px offset.
Read-only inspection of the original screenshots finds the future outline's
y=215..217 backing strip uniformly `#1c1a18` in baseline, but berry/cloud
colors in candidate (the fourth link strip is uniformly `#75495d`). This
confirms the new background reaches the actual focus footprint; the original
packet did not focus those links and cannot certify their rendered outlines.

The asset owner closed the background-only correction at
`77d8e2608a28286bcce18df000eeea4069c826d1`, documented at
`856c7170c2066975ffbf8910a2c04b0cc0d48a81`. The same header URL now has
2,298 bytes and SHA-256
`29ad1cb9d04101399e2483fef9de6a3d710c6bf7f08cb1cb4501fb746152932d`.
It explicitly stretches its 390×180 viewBox with `preserveAspectRatio="none"`
and paints an opaque cream shelf across y=120..180, followed only by clipped
light ornament. The four outline boxes map to source y=130.7655..179.1362,
inside that shelf. The author's conservative light-paint color envelope gives
4.583:1 source contrast against the unchanged ring. This is source evidence;
actual focused captures at all required widths remain outstanding.

Root granted exactly one existing CSS declaration change before this edit:
`.me-hub--reference .me-profile-header` background fallback returns from
`#573344` to its former `#1c1a18`. It preserves the original fallback's
2.288:1 ring contrast; it is **not** claimed as a 3:1 pass. The source checker
now pins the actual new header bytes/revision and unchanged scrapbook hash,
and enforces that fallback. No asset, focus style, control, character,
typography, geometry, layout, import or shared root file is authored here.
Root owns asset import and publication; this bounded source fix does not
complete the original four-screen goal.

`python3 docs/mcr/evidence/art-screen-id01/validate-source.py` exits 0:
20 granted selectors and seven closed assets pass. An independent read-only
XML/painter-order and original-report geometry calculation exits 0, confirming
all four original outline boxes lie inside the shelf and reproducing the
2.288089 fallback and 4.583275 conservative shelf ratios. The exact CSS diff
is one fallback declaration across four assigned CSS/doc/evidence paths;
`git diff --check` exits 0. No browser or capture job was started.

## Corrected native02 disposition

The owner directly inspected the actual immutable packet
`5186f11017185a040e4ccde1c59589e140563ef0`, retained in
`docs/mcr/evidence/cozy-backgrounds-v1/native-02/`. Its production/fixture/asset
revision is `8972b0b634035055eb023dbc2f786e4463b172e5`; runner revision is
`e237572719602da6a1a08b7fe0579b0e9c96da1f`; driver SHA-256 is
`58f553d9abfcfab1cac4e67ccf86c378493ec337d5c17f55749673957ba6dbde`.
Job `PANEL-CORRECTED-202611040733Z` ran on 2026-11-04 from
07:38:12.062 to 07:40:02.112 UTC, exit 0, Chrome `156.0.8078.80`.
Raw report SHA-256 is
`7f34e10279212d5f5d01017cbd644f02a33bd8939393b30818326ab2a489395b`;
capture-manifest SHA-256 is
`ba34f05a25a30b42475fa13c1349576bdc0480a69eb4aeec1d444b7d91dc16fe`.

The comparison restores only the same four paint integration files to
`7c25078f` for baseline, keeping all other production and fixture source at
`8972b0b6` in both modes. It uses actual application components with controlled
synthetic HTTP/C05/local admission. It is not a whole historical revision
replay or live-service/device qualification.

All 24 owned core PNGs were viewed in order: 390px before/after first, followed
by 360px and 430px pairs. The eight actual Me icon-focus PNGs, Me forced-colors
PNG and three Auth-state PNGs were also viewed: **36 owned captures** total.
The source collage JPEG (`2b3988d5…f12d23`) and the preserved ten-screen PNG
(`d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`)
were reinspected. The following disposition applies to background adoption
only, within the frozen character, geometry, type and control constraints.

| Route | 360 / 390 / 430px visual disposition |
| --- | --- |
| A01 Landing | PASS: broad dusty-rose/plum fields, large cream moon and blush/lilac clouds visibly soften the former black cemetery poster and the collage. The light wordmark and copy stay on dark fields. The four selected characters and entry action retain their composition. |
| A02 Sign in | PASS: broad vanilla/peach/lilac/sage paper, top clouds and lower garden replace the cemetery scenery across the form. Headings, labels, actions and provider-unavailable copy remain readable. |
| U02 Customize | PASS: the formerly unpainted lower body now has large peach/lilac/cream fields and a soft garden edge at every width. This is a dominant surrounding-paper change, rather than tiny ornament. The preview, four direct character canvases, choices and controls remain intact. |
| U01 Me | PASS: berry upper paper, a light cloud shelf behind all four header links, and broad blush scrapbook paper/clouds visibly soften both the old UI and collage. Bottom placement exposes the large ornament in the sparse lower body. The portrait, controls and existing footer silhouette remain intact; unavailable activity stays truthful. |

No additional asset or CSS defect was found. The Entry asset owner separately
reported the same all-width background disposition for A01/A02/U02 and no
required source-art revision. The earlier Customize surround and Me placement/
focus-backing findings are resolved by actual corrected composition, not merely
by the presence of their URLs.

### Preservation and accessibility evidence

`evidence/art-screen-id01/owner-native-02.json` records the exact case data and
artifact/provenance checks. Its reproducible read-only auditor is:

```sh
python3 docs/mcr/evidence/art-screen-id01/validate-native02.py
```

It exits 0, independently verifying all 60 packet artifacts against their
manifest and immutable Git blobs, all 729 recorded loaded source/asset pins
against Git objects, and all 12 owned route/width before/after pairs. For those
pairs the complete recorded control rectangles/font/color/state, body text,
HTML images and actual character records are identical. All recorded controls
are at least 44×44px, with 8 Landing, 9 Sign in, 15 Customize and 15 Me controls
per case. There are no recorded errors or horizontal overflow. The ordinary
core check records one Tab and a solid focus outline; it does not certify a
complete keyboard traversal of every product flow.

Actual character coverage is **four Landing CSS spans**, zero Sign in character
elements, **four Customize direct canvases** and **one Me direct canvas**, in
each mode and at each width. Canvas RGBA hashes, backing dimensions and display
rectangles match exactly. All four Landing spans match their resolved
36,030-byte selected sprite source (`e124c888…d7efe`), rectangles, UV positions,
background sizes and pixelated rendering. The earlier coordination claim that
all span counts were zero was corrected by Root; the raw report already had
the four nonempty Landing records and was not rewritten. Empty `images` arrays
are not used as character proof. A direct Git diff from `7c25078f` to `8972b0b6`
also finds no changes under the avatar entities/assets, avatar feature, public
UI or Auth UI source paths.

All four Me header links were actually focused at 390px. Their unchanged 3px
red outlines with 3px offset remain visible and retain their geometry. Exact
PNG pixel reads reproduce the four-side sampled minimum contrast values:

| Header link | Baseline minimum | Corrected minimum |
| --- | --- | --- |
| Edit profile | 2.2881:1 | 5.6374:1 |
| Dress up | 2.2881:1 | 6.2343:1 |
| Manage private social links | 2.2881:1 | 5.8360:1 |
| Sharing requests | 2.2881:1 | 5.5357:1 |

The 360/430px core PNGs separately show the same link geometry and light shelf.
Read-only samples at their outline footprint coordinates have backing contrast
of at least 4.8097:1 to the unchanged red. Those are backing-only checks, not
additional actual focused-state captures. The 390px forced-colors/reduced-motion
capture shows a visible system focus ring and readable controls/text; it is not
an all-screen forced-colors audit.

Actual 390px Auth pending, denied-credentials error and options-unavailable
captures show truthful status text, the appropriate disabled controls and
visible focus. Recorded targets remain at least 44px and have no page errors.
Observed pending/error glyph cores (`#791d25`) against the sampled paper fields
give at least 8.1389:1; unavailable status text gives at least 13.8502:1.
Other bounded PNG samples include Landing tagline 9.2058:1, Sign in heading
15.9312:1, Me name 9.1294:1, Me bio 15.0424:1 and Customize Random 14.2484:1.
Disabled Save remains 4.4972:1, not rounded into a 4.5:1 pass. These are selected
observed glyph/backing samples, not a global accessibility certification or
evidence of an enabled Save/live account flow.

### Source cut and delivery

The PostCSS-imported identity/community CSS leaves are absent from the raw
Vite load-hook list. Supplemental attestation
`2579ac15cb1d934de3f7a081cc7f2fb96498b5ac` records their identical tracked
runner/production blobs. The auditor independently verifies those blobs,
hashes and byte counts, the retained `guard.py.snapshot` prebrowser clean-HEAD/
empty-status assertions, and the `guard.json` successful e237 result. Combined
with the observed paint and Root's sole-writer/no-source-mutation receipt, this
supports capture cut identity by clean-checkout inference. It is explicitly
**not** an individual imported-leaf Vite load-hook read receipt; the raw 729
records remain unchanged.

The owned CSS and all seven closed assets are byte-identical on published
master `aea02a262f7d834be7ef2267368812520a5d9c0c`; frozen `8972b0b6` is an
ancestor of that publication (exit 0). Source deliveries are `e0707c88`
(initial CSS), `3b429555` (Customize outer paper), `7c55b4e2` and `948fdeea`
(Me outer paper/placement), `21493621` (closed light-shelf asset), and
`f86c99bd570f5d104b6d9d6093c821f923fd8cce` (fallback and revised pins).
This owner's original implementation handoffs through `d2f9c8e5` remain
preserved. Root owns final publication of this evidence handoff.

`validate-source.py` also exits 0 for the unchanged 20-selector/seven-asset
boundary; `git diff --check` exits 0. No new browser, native, build, test matrix,
packet restoration or artifact copy was started by this owner. Browser/server
closure and temp cleanup are recorded by the original allocated job. No live
multiuser flow, physical device, Profile/avatar save, Me504 behavior, whole IA
acceptance or release acceptance is claimed by this bounded background task.
