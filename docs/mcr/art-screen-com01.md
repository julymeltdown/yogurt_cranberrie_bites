# ART-SCREEN-COM01 — Community background adoption

Status: **IN_PROGRESS**. Background source adoption is implemented; rendered adoption,
responsive checks, publication and visual acceptance are **NOT_RUN**.

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
| M01 | `.party-browse` | Container background only; existing Meet typography belongs to its owner |
| G01 | `.gifts-reference .gift-reference-row` | Row background only; exclude item images and actions |
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
| Feed post/footer | `/backgrounds/cute-community-v1/feed-backdrop.svg` | `2a5c67e5` |
| Meet container | `/backgrounds/cute-community-v1/meet-backdrop.svg` | `2a5c67e5` |
| Gifts rows | `/backgrounds/cute-community-v1/gifts-backdrop.svg` | `2a5c67e5` |
| Wall surrounding body | `/backgrounds/cute-wall-me-v1/wall-stationery.svg` | `8ff3a58f556bf2811ad1415cd31c5ba1953ccd58` |

The three community SVGs are opaque 390×640 editable vector source; Wall is
opaque 390×180. The community surfaces use Root's granted `center`, `100% 100%`,
`no-repeat` mapping and matching solid fallbacks. Wall uses `center top` and
`100% auto` with its matching solid fill. Exact bytes and hashes are
in [source evidence](evidence/art-screen-com01/source-preparation.json).
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
3. After the first actual World/Map 390px cue and an explicit coordinator21d
   lease, capture actual mounted components before/after at 390px in identical
   states. Capture 360/430px only after inspecting the 390px result.
4. Verify broad visual improvement, computed contrast, visible focus, unchanged
   typography/control bounds, no horizontal overflow, truthful unavailable and
   disabled states, and no change to avatars, media or mural pixels. Keep
   desktop viewport evidence distinct from actual mobile-device acceptance.
5. Commit only the granted paths; hand off exact base, SHA, changed paths,
   commands/exit codes, evidence and limits to Root. Verify published ancestry
   after Root publication and address concrete fixes before goal completion.

No browser, native, build or export workflow has been started for this task.
The reserved Wall CSS does not prove adoption until Root's semantic hook and
import are present in the rendered candidate. Real multiuser flows, physical
devices, and release gates remain separate obligations.
