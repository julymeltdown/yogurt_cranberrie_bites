# ART-SCREEN-ID01 — identity background adoption

Status: **IN_PROGRESS**. Implementation and rendered visual acceptance are unfinished.

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
`9a962ea6f8c20fe59846cbcf4f468b902e3f280e`; Me source revision is
`8ff3a58f556bf2811ad1415cd31c5ba1953ccd58`. The Entry manifest and exact Git
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
| U01 profile header | `cute-wall-me-v1/me-night-ribbon.svg` | Berry field with broad mauve clouds; original opaque portrait surface remains |
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
freeze and direct visual comparison remain valid narrower evidence. The owner
requested actual canvas RGBA/dimensions/rectangles and landing span resource/
position/size preservation in Root's next single allocated workflow.

Browser/server closure and temp removal are recorded. A later shared fourth
paint correction is outside this packet's cohort. Required remaining work is
the integrated Customize correction, actual 360/430px evidence, rendered
contrast, additional relevant auth/focus states, quantitative character
invariance, any resulting fixes, final documentation and fresh remote proof.
The full four-screen persistent goal remains **ACTIVE**.
