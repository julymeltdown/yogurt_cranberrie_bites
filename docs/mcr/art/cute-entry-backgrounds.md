# Cute entry backgrounds — FE08 / FE09 / FE12

Status: **IN_PROGRESS**. Five original SVG assets are authored and statically
validated. Assets, CSS consumption and the application import are published and
verified on `origin/master` at `c8a731e5`. Actual same-state 390px comparisons
were executed and inspected for all three routes. Landing and Sign in visibly
adopt the new backgrounds; Customize still needs the outer shell's unpainted
surround addressed. 360/430px and additional accessibility/state checks remain
**NOT_RUN** at this checkpoint. This is a background-art increment, not task or
release acceptance.

## Scope and ownership

IA A01 Landing, A02 Sign in and U02 Customize. Source base:
`bfa0193939c3eebd6c80ade85083c6530c6875e1`; isolated worktree
`/Users/lhs/.codex/worktrees/ac70/mcr`; branch
`codex/mcr-art-cute-entry-v1`. This worker owns only new
`frontend/public/backgrounds/cute-entry-v1/**` and this document.

ID01 (`01a1a533-50a8-7fb2-b3ed-96b941b96bbf`) owns the separate, explicitly
granted background stylesheet. Root153
(`01a1a153-1227-7e72-801f-a2a6b91981e1`) owns import, application composition
and master publication. Existing functional CSS, JSX, characters, sprite
sources, renderer, pixel data, crop, sizing and framing are unchanged. No old
ornamental foreground is hidden, replaced or removed by this asset patch.
No routes, text, controls, guards, focus, layout or typography are changed.

## Editable source and provenance

All vector geometry is newly hand-authored in
[`author.py`](../../../frontend/public/backgrounds/cute-entry-v1/author.py).
It uses Python's standard library only. Reproduce with:

```sh
python3 frontend/public/backgrounds/cute-entry-v1/author.py
```

There is no copied Bigyeting code or art, third-party illustration, raster
embedding, font, external URL or runtime dependency. Source originals under
`docs/source/` and all selected character resources remain byte for byte
untouched. These are decorative paper, flowers, clouds, hearts, stars and music
notes; they contain no words, faces, characters, mascots or controls.

The human collage was inspected for existing composition and foreground
contrast only. Its SHA-256 is
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`;
none of its pixels or contours were copied. The exact attachment remains at
`/tmp/codex-remote-attachments/01a1a385-9f32-73c2-8715-ff54bd1e0497/120DC5EC-1683-42AB-AF36-0DA8EB43CDE0/1-Photo-1.jpg`.

The existing immutable QA06 frames were inspected as historical before
evidence. Landing and Sign in use 390px frames from source candidate
`980fc4d85020f297b36fee8195e1ca92079ff33f`. Customize is the qualified
350x685 component capture inside a 390x698 viewport, from application source
`14d61ee7d179732d3fd582de0876de664613ff79`, copied byte for byte from the
`57c4fbb1554e25ef3a1756e0d4ea9cfe2147ab91` evidence artifact. None represents
the present application tree or a new device check. Pins are in
[`comparison.json`](../evidence/visual/qa06/human-reference-20261104/comparison.json).

| Screen | Existing frame | SHA-256 |
| --- | --- | --- |
| A01 | `docs/mcr/evidence/visual/qa06/fidelity-v3/equal-landing-390.png` | `0a979de9e0fa7d5c9b7839756f2825eda5358c2c6db34465e18a67a6470bb637` |
| A02 | `docs/mcr/evidence/visual/qa06/fidelity-v3/equal-login-390.png` | `f3b79abb641ba8e2d8dad61f472f4f449a7e07b35bc567f0aee14231ce78bead` |
| U02 | `docs/mcr/evidence/visual/qa06/human-reference-20261104/customize-portrait.png` | `c4cb177d72ae8ebe9dcc57ef28a5880abb7171aaa88429b275db847d6a5d7af2` |

## Consumer interface

All URLs begin `/backgrounds/cute-entry-v1/`. Exact bytes, hashes, viewBoxes
and alpha flags are in
[`manifest.json`](../../../frontend/public/backgrounds/cute-entry-v1/manifest.json).
Use as CSS background images only. Use sizing and position within the owner's
exact granted surface; these files do not determine element size or layout.

| File | ViewBox | Background | Intended use |
| --- | --- | --- | --- |
| `landing-garden-night.svg` | `0 0 390 660` | Opaque dusty rose | Existing poster background behind unchanged light lettering, crowd and ornamental foreground. Broad rose field, large cream moon and blush/lilac rounded clouds in the illustration midzone. |
| `sign-in-paper.svg` | `0 0 390 720` | Opaque pale vanilla | Existing form window paper. Quiet unpatterned center; broad peach/lilac/sage fields across the side thirds and faint sparse motifs at the perimeter. |
| `sign-in-clouds-top.svg` | `0 0 390 240` | Transparent | Decorative top background. Root subsequently granted ID01 replacement of only the existing Cemetery image payload, preserving its SVG container and layout. |
| `sign-in-garden-bottom.svg` | `0 0 390 180` | Transparent | Optional low garden background only. Do not move or clip content to expose it. |
| `customize-paper.svg` | `0 0 390 700` | Opaque pale vanilla | Granted outer body/footer of the motion, whole and layered editors only. Preview, canvas, figure, character backdrop and item image surfaces remain excluded. |

Flat bases are `#624250` (rose night) and `#fbf2df` (paper), with a broad
`#6b4b5b` top wash and `#503b4a` lower copy field. The environment uses large
blush/cream/lilac cloud shapes, peach washes and sage garden ground to make the
background change visible at a glance, rather than relying on tiny motifs.
No repeated card accents, left rails,
animation, filters or patterned center. Existing opaque foregrounds may obscure
some decorative geometry; do not claim visible adoption from asset presence.

For an exact `100% 100%` stretch of the landing SVG, the frozen contour
wordmark's source bounds `[47,55,283,213]` in `[10,35,286,485]` map to
background y `27.2..242.2`; the new light moon begins at y `258` and the
cloud mass begins at y `267`. Frozen copy bounds y `392..456` map to
`485.8..572.9`, within the deep lower copy field. This source-coordinate check
supports safe placement; it does not replace an actual composed screenshot.

## Validation and remaining evidence

| Check | Result | Limits |
| --- | --- | --- |
| `python3 frontend/public/backgrounds/cute-entry-v1/author.py` | Exit 0 | Five SVGs, 13,817 SVG bytes; editable source and manifest generated. |
| `python3 frontend/public/backgrounds/cute-entry-v1/verify.py` | Exit 0 | [`static-validation.json`](../../../frontend/public/backgrounds/cute-entry-v1/static-validation.json); XML, local-reference allowlist, manifest integrity, deterministic rebuild and flat contrast checks. Inert vector elements only; no external/character references. |
| `python3 scripts/mcr/check-plan.py` | Exit 0 | Planning/provenance gate only. Application build/tests/device/deployment NOT_RUN. |
| Separate committed asset and CSS handoffs | Asset bytes at `9a962ea6f8c20fe59846cbcf4f468b902e3f280e` match all five manifest pins; CSS at `e44b5d1c38f05b03a74904c94004d5fc983b2493` references all five URLs | Read directly from separate Git objects. The CSS-only commit does not contain the asset files; both must be integrated. This proves committed source compatibility, not an application import or rendered result. |
| Actual in-app 390px before/after, all three screens | IN_PROGRESS | Root's completed `PANEL-COZY-202611040632Z` packet was directly inspected. Landing and Sign in show dominant cute backgrounds; Customize's large lower shell surround remains old grain. Required surface correction and responsive follow-up remain open. |
| Relevant actual 360/430px captures, focus, 44px controls, no overflow, character-pixel invariance | NOT_RUN | Consumer must preserve existing functions, foreground, layout and readable text. No static palette probe certifies full composed accessibility. |
| CSS import/adoption and master ancestry | INTEGRATED | Fresh fetched `origin/master` at `c8a731e5c7f03c3fbcbd0581c6e872429806fa22` contains byte-identical SVGs, matching manifest pins and consuming stylesheet/import. Integrated asset `3f06f54d`, identity CSS `e0707c88` and application import `a6633eef` are all verified remote ancestors. Goal stays active through rendered verification and required fixes. |

Sampled flat palette contrast is at least 4.5:1 for existing body, muted and
error text against every broad paper wash and for existing cream print against
each rose/plum text field. The light moon/cloud midzone is illustration-only;
white lettering must not overlap it. Actual layers, disabled controls and focus
indicators require rendered verification after adoption.

## Committed handoff

Asset/source/document handoff: `9a962ea6f8c20fe59846cbcf4f468b902e3f280e`.
Manifest SHA-256:
`3bb475a289ab056e9398b565ad68d497a815e5c2d791d6a5be9dbe047f3dc77b`.
Its ten changed paths are exclusively the granted asset directory and this
document. Root153, ID01, the Screen ART SL and UI SL received the handoff;
delivery does not wait for a lead review. Manifest pins identify asset bytes,
not the consuming application candidate. CSS consumption and remote ancestry
still require actual evidence before this objective can be complete.

ID01 delivered `e44b5d1c38f05b03a74904c94004d5fc983b2493` with
`frontend/src/shared/styles/cozy-identity-backgrounds-v1.css`. All five CSS URLs
reference the separate asset handoff `9a962ea6`; its asset hashes match this
manifest. The first attempted combined-object read failed because the CSS-only
commit correctly contains no asset files. The corrected check read each asset
from `9a962ea6` and each URL from `e44b5d1c`, and passed for all five files.
Root's narrow decorative-payload
grant lets that separate CSS owner hide only `landing-web > g`,
`landing-scene > g` and `auth-cemetery > image`, while preserving their SVG
boxes. New top/bottom backgrounds use the existing Cemetery containers. The
landing wordmark, copy, selected crowd, character pixels and every control
remain excluded from this replacement. This updates the earlier optional-hook
proposal; the asset worker never acquired CSS or component write rights.

An earlier remote observation at `7c25078f` had no artwork or import. A later
fresh fetch on 2026-11-04 verified publication at
`c8a731e5c7f03c3fbcbd0581c6e872429806fa22`. Reading actual remote Git objects
proved all five SVG bytes match this worker's closed handoff and their manifest
hashes; every URL is consumed by the integrated identity CSS. Git ancestry checks
for `3f06f54d`, `e0707c88` and `a6633eef` exited 0. The import is present in
`frontend/src/shared/styles/base.css`.

The integrated change from `7c25078f` through `c8a731e5` has no changed paths
under `frontend/src/entities/avatar`, `frontend/public/world/avatars`,
`frontend/src/features/public/ui`, `frontend/src/features/auth/ui` or
`frontend/src/features/avatar`. This is source invariance evidence; rendered
character geometry and interaction evidence still comes from the combined
capture. Root's published capture harness is
`frontend/src/app/__tests__/cozy-backgrounds-v1/native.mjs` and its declared
scope is the actual default router with explicit synthetic transport, not a
live-service or physical-device gate.

## Actual 390px packet inspection

Job: `PANEL-COZY-202611040632Z`, 2026-11-04 06:33:32.440–06:34:13.328 UTC,
exit 0, Chrome `156.0.8078.80`, viewport 390x844. Actual default router/Auth/
Phaser with explicit QA06 synthetic HTTP/C05/local admission. The baseline is
the same `c8a731e5` candidate with only four paint-integration files restored to
`7c25078f`; Meet typography, fixtures and all other source remain identical.
This is a controlled background comparison, not a full historical application
baseline or live service/device acceptance.

The actual emitted packet exists at
`/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish/docs/mcr/evidence/cozy-backgrounds-v1/native-01/`.
`report.json` SHA-256 is
`59e0f2540a1fdc79f240230048385fda678c1029e9d3df3ab2a209320ae86da1`.
The initial `report.partial.json` read raced with finalization; reading the
completed report succeeded. There was no restart or independent capture job.

| Actual frame | SHA-256 |
| --- | --- |
| `baseline-landing-390.png` | `dbb09ff5b80b7ceb578c17778b3021722655ff2c55f9af1904ded21345e288cc` |
| `candidate-landing-390.png` | `daae6b6d413ded95bec493383444cc3860fa121c456e07c8fd462d3ad825fbb2` |
| `baseline-login-390.png` | `bf98a92a890b1f1c9c34019c0675b143d260f2225ca1a51fa18344c50bb6979e` |
| `candidate-login-390.png` | `c8d74794b59ea9674662b73dd857e51ad3737a8ed042918bfd491b20d5b0f7e5` |
| `baseline-customize-390.png` | `1c7f1bcf709449da0a36c9a0a949f37cd5dc0f77be4dafbff774e8f1985098b2` |
| `candidate-customize-390.png` | `a3a54afe233b4fd49f91afa8c33506c2a4039ef0cfae915e367ebea3bb596175` |

All six image hashes were independently checked against the completed report;
all six images were directly viewed. For each candidate entry route, report
checks show identical text and visible control rect/type/color/state, identical
visible image src/rect/RGBA, no horizontal overflow, no errors and a solid
keyboard focus outline. All visible control rectangles recorded for these six
cases are at least 44px in both dimensions. `document.images` does not include
the Landing crowd's CSS spans; character-source invariance and direct visual
comparison support that part, rather than a false claim of sampled crowd RGBA.

Owner findings:

- **A01:** broad dusty-rose field, large cream moon and blush/lilac clouds
  visibly replace black cemetery scenery. Existing light wordmark/copy remain
  readable on the dark fields; no light-illustration overlap with copy is seen.
  The four-character line, print, entry action and their composition match.
  No source-art fix is established from this 390px pair.
- **A02:** broad peach/lilac/sage paper, top clouds and lower flowers/garden
  visibly replace cemetery ornament. Form text, red action, provider-unavailable
  explanation and account links remain readable. No source-art fix is
  established from this 390px pair. Native error/pending/unavailable states and
  a composed contrast audit are still open.
- **U02:** preview and character choices correctly remain unchanged. The new
  paper shows only in limited body/footer surroundings; the large blank area
  around y565..837 still shows the original grain. Source and CSS-owner evidence
  identify the actual `.shell-panel-body` (x9, y57, 372x780) as the unpainted
  backdrop outside the granted editor body/footer hooks. The dominant cute
  background requirement is **not yet satisfied for this screen**. Root/ID01
  received this concrete non-avatar hook gap; this asset owner has no CSS rights.

The job closed its browser/server and removed its temporary directory. Required
360/430px captures, the corrected U02 surround, additional state/accessibility
checks and any resulting asset fixes remain open; the persistent goal stays
active. No source asset change is justified merely by the shell-hook gap.
