# Cute entry backgrounds — FE08 / FE09 / FE12

Status: **VALIDATED — pending final document publication receipt.**
The corrected immutable `native-02` packet has been directly inspected for
A01/A02/U02 at 360, 390 and 430px, together with three actual Sign in state
captures. The five original SVGs need no revision. The Customize lower-surround
finding is resolved in the corrected composition. Paired controls, text,
character canvases and CSS spans are unchanged in the measured states; all
recorded targets are at least 44px, with no horizontal overflow or page errors.
This supersedes the earlier blocked checkpoint below. Root153 owns final
integration/publication. The verdict covers this background increment only;
it does not establish live-service, physical-device or release acceptance.

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
| Actual in-app before/after, all three screens | PASS, corrected native-02 | All nine pairs at 360/390/430px directly viewed. The lower Customize surround is covered; see corrected-packet disposition below. |
| Actual 360/430px, focus, 44px controls, overflow and character invariance | PASS within recorded states | Independently compared report metrics and retained PNGs. Nine candidate first-focus records are solid; three 390px Auth variants were also inspected. Sampled contrast is bounded, not full accessibility certification. |
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

## Historical first 390px packet inspection

Job: `PANEL-COZY-202611040632Z`, 2026-11-04 06:33:32.440–06:34:13.328 UTC,
exit 0, Chrome `156.0.8078.80`, viewport 390x844. Actual default router/Auth/
Phaser with explicit QA06 synthetic HTTP/C05/local admission. The baseline is
the same `c8a731e5` candidate with only four paint-integration files restored to
`7c25078f`; Meet typography, fixtures and all other source remain identical.
This is a controlled background comparison, not a full historical application
baseline or live service/device acceptance.

The actual emitted packet exists at
`/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish/docs/mcr/evidence/cozy-backgrounds-v1/native-01/`.
Its immutable artifact commit is
`16bf30c617995f87c89f94a13d9cc5f4c65d51b5` (initially local to the publisher,
pending normal remote publication). A later sparse-checkout reapplication
temporarily dematerialized the physical PNGs; it did not remove their Git bytes.
Root restored the exact evidence path with a targeted sparse-checkout inclusion.
All six entry PNGs were then read again and independently matched their original
report hashes. No image was regenerated or overwritten.
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
checks show identical text and visible control rect/type/color/state, no
horizontal overflow, no errors and a solid keyboard focus outline. All visible
control rectangles recorded for these six cases are at least 44px in both
dimensions. The report's image-preservation booleans are true, but the actual
`metrics.images` arrays are empty for **all six entry cases**. They therefore
provide no character-pixel proof: `document.images` excludes both the Landing
crowd's CSS spans and Customize's canvases. Character-source invariance and
direct visual comparison are the available evidence; CSS span source/geometry
and actual canvas RGBA/rect sampling remain requested for the next job.

A subsequent read-only check of the retained actual screenshots confirms the
entire opaque Customize preview interior `(21,69)..(369,391)` is identical
between the baseline and candidate: 348x322 RGBA, SHA-256
`386115826443781cd84dfabcf9acc4f6c30ec95169e05bf66ee3d240d31b8046`.
The asset owner independently reproduced the CSS owner's bounded pixel result
from both original PNGs without exporting or modifying an image. This proves
the original 390px screenshot's preview-region invariance; it does not sample
Landing's CSS spans, the complete avatar runtime, other widths or the later
surround-fixed candidate.

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

ID01 subsequently committed the exact Root-granted surround fix at
`cff736a816603f1f79f38af1b510841e1a026eb5`: the new stylesheet paints only
`.shell-panel[data-mcr-panel='customize'] > .shell-panel-body` with the same
pinned `customize-paper.svg`. Root owns the route marker. This is a committed
source fix, not a corrected render; its actual rendered result, responsive
coverage and publication must still be checked. No asset revision is needed
unless that next actual composition establishes a source-art defect.

Existing-PNG read-only spot sampling adds bounded composed contrast evidence.
The actual 390px Login heading/label glyph cores are `#191817` over the new
`#fbf2df` and `#f4dfd9` fields; the account-link region also shows the same dark
core over the sage field. Landing print, status and footer remain light on the
rose/plum background. These are selected observed glyph/background samples,
not a full audit of antialias edges, every character, focus state or unavailable/
pending/error variants. No browser/export workflow or image edit was involved.

| Observed region | Sampled core / backing | Contrast |
| --- | --- | --- |
| Login heading | `#191817` / `#fbf2df` | 15.93:1 |
| Login Email label | `#191817` / `#f4dfd9` | 13.85:1 |
| Login provider note | `#5a5043` / `#f3e4d1` | 6.32:1 |
| Login account link | `#191817` / `#e9eddd` | 14.88:1 |
| Landing tagline print | `#f8f7f5` / `#523d4c` | 9.21:1 |
| Landing entry status | `#e5d8bf` / `#503b4a` | 7.22:1 |
| Landing footer | `#d3c2a4` / `#503b4a` | 5.82:1 |

## Historical blocked checkpoint — superseded by native-02

Independent source, first-390 inspection, targeted contrast and retained-preview
checks are complete. The owned worktree is clean. The following explicit
requirements are unproven and are not replaced by those narrower checks:

- Corrected U02 outer surround must be directly inspected in an actual 390px
  application capture; the original screenshot still has the known old-paper gap.
- Actual A01/A02/U02 360/430px rendering and relevant accessibility/geometry
  checks remain unavailable.
- Full direct character canvas/CSS-span invariance on the corrected candidate
  remains unavailable; the retained 390px preview-region proof is bounded.
- The corrected consuming candidate, retained follow-up packet and final owner
  document must be published and verified on master.

The same Root-only capture/publication dependency has persisted across at least
three goal turns. The latest readback still has only the completed native-01
report and no matching active cozy capture process or supplied live follow-up
handle. Individual capture launches and consumer/root writes remain outside this
owner's authorization. The goal is blocked pending the external packet, not
complete, and this changes neither product acceptance nor the Root queue. All
source, actual first-packet observations, pins and required follow-up scope are
preserved for continuation when the packet is available.

## Corrected native-02 owner disposition

Owner verdict: **PASS for A01/A02/U02 background art and its measured adoption**.
No owned SVG change or further ID01 paint correction is justified by this
packet. The original human collage was viewed again alongside the actual
application pairs. Large rose, cream, peach and lilac fields, rounded clouds,
moon and garden make the softer, more playful direction visible at a glance.
The existing character style, red actions, wordmark and composition remain.
No left rail or new control is introduced.

### Immutable provenance and inspection boundary

- Artifact commit: `5186f11017185a040e4ccde1c59589e140563ef0`.
- Frozen production/application/fixture revision:
  `8972b0b634035055eb023dbc2f786e4463b172e5`.
- Runner revision: `e237572719602da6a1a08b7fe0579b0e9c96da1f`;
  driver SHA-256:
  `58f553d9abfcfab1cac4e67ccf86c378493ec337d5c17f55749673957ba6dbde`.
- Job: `PANEL-CORRECTED-202611040733Z`,
  2026-11-04 07:38:12.062–07:40:02.112 UTC, reported exit 0,
  Chrome `156.0.8078.80`; 360/390/430px widths, 844px viewport height.
- Canonical retained directory:
  `/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish/docs/mcr/evidence/cozy-backgrounds-v1/native-02/`.
- Independently read `report.json` SHA-256:
  `7f34e10279212d5f5d01017cbd644f02a33bd8939393b30818326ab2a489395b`.
- Independently read `capture-packet.json` SHA-256:
  `ba34f05a25a30b42475fa13c1349576bdc0480a69eb4aeec1d444b7d91dc16fe`.

The packet reports 48 core captures and 60 total PNGs, with 729 loaded source
pins verified by the publisher. This owner directly viewed **18 Entry PNGs and
three Auth-state PNGs**, independently checked all 21 hashes against the report
and their exact immutable Git blobs, and confirmed the report itself matches
that commit. The other route and Me-specific screenshots are outside this
owner's verdict. The raw packet retains `PENDING_OWNED_SOURCE_INSPECTION`;
this document supplies the owner disposition without rewriting evidence.

Both modes use the same frozen candidate and fixture. Baseline restores only
`frontend/src/app/shell.css`, `frontend/src/app/shell-chrome/shell-chrome.css`,
`frontend/src/shared/styles/base.css` and `frontend/src/app/WorldShell.tsx`
to `7c25078f118da53dd9a524d0a58e707cca190f0f`. Other application source,
Meet typography and fixtures remain identical. It is a controlled paint
comparison, not a full historical build. The actual default router/Auth/Phaser
runs through QA06 synthetic HTTP/C05/local admission; Wall remains guarded.
There is no live IN01, Profile/avatar-save, Me504 or physical-device proof.

The candidate's nine owned asset/source/manifest/validation files were read
from Git and are byte-identical to this branch. The manifest remains
`3bb475a289ab056e9398b565ad68d497a815e5c2d791d6a5be9dbe047f3dc77b`.
The candidate identity stylesheet consumes the five pinned SVG URLs, including
`.shell-panel[data-mcr-panel='customize'] > .shell-panel-body` for the corrected
lower surround. It leaves preview/character surfaces opaque. No application,
CSS, evidence or packet-retention files were edited by this owner. No new
browser, capture, export or test matrix was launched.

### Visual and preservation findings

| Surface | Disposition across 360/390/430px |
| --- | --- |
| A01 Landing | PASS. Dominant dusty rose with a large cream moon and rounded blush/lilac clouds replaces the old cemetery field. Existing light wordmark and lower copy remain readable; the four character sprites and red entry action retain their placement and size. No new overlap is visible. At 430px, lower footer content remains below the 844px fold in both modes; this is not evidence of full-page visibility. |
| A02 Sign in | PASS. Broad quiet pastel paper, top clouds and lower garden are visible at every width. Labels, fields, sign-in action, provider explanation and account link retain their composition. Garden decoration stays below account text. |
| U02 Customize | PASS. The old grain below the editor is replaced across the full lower shell surround at every width. Broad blush/lilac paper and the sage wave are visible; the preview grid, large avatar, three choices and actions retain their pixels/geometry. Header contrast remains readable. |
| Auth pending, 390px | PASS for captured state. Status text and disabled inputs/action remain clear. The focused recovery link has a visible solid outline; garden decoration remains below account text. |
| Auth error, 390px | PASS for captured state. The two-line error is readable over the pastel field, with no overlap of the next label. Controls and account text remain distinct as the form grows. |
| Auth options unavailable, 390px | PASS for captured state. Explanation, 44px retry control and disabled sign-in controls are readable. Expanded form and lower garden do not obscure account text; focused close control is visible. |

Independent comparison of the report's nine Entry baseline/candidate pairs
confirms exact equality of control tag/label/rect/font/color/disabled records,
text and character metrics. Every pair has no reported horizontal overflow or
page errors. All recorded Entry control rectangles, including the normally
offscreen skip link, are at least 44px in both dimensions. All three Auth
variants also record controls at least 44px and no page errors. The Auth
variants have no baseline-state pair; their geometry is not claimed invariant
against the default form. They were captured at 390px only.

For all nine candidate Entry cases the runner records a solid first keyboard
focus outline. The three Auth records also have a solid outline, visibly
confirmed in their PNGs. These checks do not traverse every control or certify
all focus combinations. The Me focus/forced-colors result belongs to its owner.

Character evidence now measures the relevant rendering paths: Landing has four
CSS spans with identical source SHA-256
`e124c88815b22220df2f2dc59e2f26b3ceb845e57b77674fd8e55383116d7efe`,
rectangles, sprite positions, size and pixelated rendering. Customize has four
canvases with exact matching RGBA hashes, dimensions and rectangles per width;
the main 256x256 canvas hash is
`f3c4a50b5b594db1afdd674a57e3d6085a3776bf68f4f53c5e2ff94bd20bbcf0`.
Login has explicitly zero canvases/spans. The empty `metrics.images` arrays
provide no additional character proof. This covers the visible frozen fixture,
not all selectable avatars, animations or future states.

An independent read-only RGBA comparison of each retained Customize preview
interior also matches exactly. No image was copied, altered or exported:

| Width | PNG box, exclusive right/bottom | Equal RGBA SHA-256 |
| --- | --- | --- |
| 360 | `(21,69)..(339,391)` | `b27a8813e8b11d069ec3ddc2c361e24956ef764d1f2662349d56e63293276977` |
| 390 | `(21,69)..(369,391)` | `386115826443781cd84dfabcf9acc4f6c30ec95169e05bf66ee3d240d31b8046` |
| 430 | `(21,69)..(409,391)` | `c3d97b9bd3eb8bdf0412cd047e8e7353cdd244fe1700e477a16607c5f6a0ac8c` |

### Bounded accessibility and integrity checks

Fresh `verify.py` exits 0: five deterministic inert SVGs, 13,817 SVG bytes,
minimum sampled text contrast 5.98:1. Existing PNGs were read without mutation
to sample Auth text and the recovery-link focus outline. Observed glyph cores
are error/pending `#791d25`, standard text `#191817`, and outline `#921f28`.
Against the observed broad paper fields `#fbf2df`, `#f4dfd9`, `#eee4f0` and
`#e9eddd`, calculated minima are 8.14:1, 13.85:1 and 6.70:1 respectively.
These are selected core-color/background checks; they do not assert that every
outline pixel meets the minimum or substitute for a full accessibility audit.
The earlier Landing/Sign-in text samples remain applicable to their unchanged
390px PNGs. No foreground or focus implementation was changed here.

### Inspected PNG pins

Each path below is relative to the canonical `native-02` directory. Every
listed file was directly viewed and independently matched to both the report
and immutable artifact commit.

| File | SHA-256 |
| --- | --- |
| `baseline-landing-360.png` | `a8ba5739c8a3eaeeb4fda35264e95438d552a9bd89dc8ee6b785b07b2063af54` |
| `candidate-landing-360.png` | `7b33f1c26795a2449891bedcad83f4f1f99582db909059f239ba1441eeea5815` |
| `baseline-landing-390.png` | `dbb09ff5b80b7ceb578c17778b3021722655ff2c55f9af1904ded21345e288cc` |
| `candidate-landing-390.png` | `daae6b6d413ded95bec493383444cc3860fa121c456e07c8fd462d3ad825fbb2` |
| `baseline-landing-430.png` | `5b5795ffc225bb590b83f7d2d51779d7f0ca3fbb4dae461534fa27a62a14c85d` |
| `candidate-landing-430.png` | `a38e96629aae06d37dae5a63e6006463f60cbc32561ae905b9b424e89e8a9a64` |
| `baseline-login-360.png` | `c12fefa18b6ad458c8cd647843c2b66f4140fb8136ba28b7333e038b8ad69900` |
| `candidate-login-360.png` | `3354f588b0dc33b5bd3d76a1669ce8d7e90653c420a8079f33a2460bdc040bff` |
| `baseline-login-390.png` | `bf98a92a890b1f1c9c34019c0675b143d260f2225ca1a51fa18344c50bb6979e` |
| `candidate-login-390.png` | `c8d74794b59ea9674662b73dd857e51ad3737a8ed042918bfd491b20d5b0f7e5` |
| `baseline-login-430.png` | `59c9f0806ec9a182031390e5db59c214f9ee15b109ca71f1ebf0ff7117fc6af3` |
| `candidate-login-430.png` | `340a71c59aaba324ba2a4e96ce6c6d72b98b8b57f17cdbce7f879655e4e6b7e3` |
| `baseline-customize-360.png` | `16f50df1480aca0e52fac5fc2feea738c64280ced8dd1d71b2ede3ab0a78b05d` |
| `candidate-customize-360.png` | `f04561fd0754873ce7f01ef44004009db484b931d7363f626231165516ea1f35` |
| `baseline-customize-390.png` | `1c7f1bcf709449da0a36c9a0a949f37cd5dc0f77be4dafbff774e8f1985098b2` |
| `candidate-customize-390.png` | `b4f5e1744fe635ac6c21cdcb23a7f7ad50d2fc698309110dde30bf49f65f6ec1` |
| `baseline-customize-430.png` | `19bf72f5d2894fe568f876b2ecb1e4214821fff4fb6c5a79c0c068762a56c939` |
| `candidate-customize-430.png` | `63acabafe664138d723476b95531d301b609830f42e425e585fdbe629a0187f7` |
| `candidate-login-pending-390.png` | `576d54e1ff020aa2382f4a840aed30a0c873bd8c8103705c298d4d2a935dcbb6` |
| `candidate-login-error-390.png` | `b3c239cac808e74faa1ac863e3642499e5dde8b59aca12ddb5f9c0ad9787f76d` |
| `candidate-login-options-unavailable-390.png` | `0fdf033963cf5d5e6b011cad868f972c7c758c6e5187f9356267dc906cab3a4c` |

### Final handoff boundary

Owned validation and required source-fix assessment are complete. Only this
document changes on the resumed branch; all artwork remains the previously
published pinned source. ID01 has received the concrete PASS disposition and
resolved U02 finding. Root153 must import this document and verify the corrected
candidate, immutable packet and final owner documentation on remote master.
The persistent goal must not be marked complete before that receipt. Its API
still reported `blocked` when this resumed inspection began; that historical
status was not reset or substituted for the work performed above.
