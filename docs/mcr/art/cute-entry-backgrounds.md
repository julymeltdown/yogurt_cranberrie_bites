# Cute entry backgrounds — FE08 / FE09 / FE12

Status: **IN_PROGRESS**. Five original SVG assets are authored and statically
validated. CSS adoption, actual rendered comparisons and publication are
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

The existing immutable QA06 390px frames were inspected as historical before
evidence. Their source candidate is `980fc4d85020f297b36fee8195e1ca92079ff33f`,
not the present application tree. Pins are in
[`comparison.json`](../evidence/visual/qa06/human-reference-20261104/comparison.json).

| Screen | Existing frame | SHA-256 |
| --- | --- | --- |
| A01 | `docs/mcr/evidence/visual/qa06/fidelity-v3/equal-landing-390.png` | `0a979de9e0fa7d5c9b7839756f2825eda5358c2c6db34465e18a67a6470bb637` |
| A02 | `docs/mcr/evidence/visual/qa06/fidelity-v3/equal-login-390.png` | `f3b79abb641ba8e2d8dad61f472f4f449a7e07b35bc567f0aee14231ce78bead` |
| U02 | `docs/mcr/evidence/visual/qa06/fidelity-v3/equal-customize-390.png` | See the pinned QA06 comparison record; older frame, no new device claim. |

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
| `sign-in-clouds-top.svg` | `0 0 390 240` | Transparent | Optional decorative background only, contingent on a precise authorized composition hook. Never replace or hide the existing Cemetery foreground. |
| `sign-in-garden-bottom.svg` | `0 0 390 180` | Transparent | Optional low garden background only. Do not move or clip content to expose it. |
| `customize-paper.svg` | `0 0 390 700` | Opaque pale vanilla | Outer non-avatar panel body/footer only, subject to World/Root hook grant. Preview, canvas, figure, character backdrop and item image surfaces remain excluded. |

Flat bases are `#624250` (rose night) and `#fbf2df` (paper), with a broad
`#6b4b5b` top wash and `#503b4a` lower copy field. The environment uses large
blush/cream/lilac cloud shapes, peach washes and sage garden ground to make the
background change visible at a glance, rather than relying on tiny motifs.
No repeated card accents, left rails,
animation, filters or patterned center. Existing opaque foregrounds may obscure
some decorative geometry; do not claim visible adoption from asset presence.

## Validation and remaining evidence

| Check | Result | Limits |
| --- | --- | --- |
| `python3 frontend/public/backgrounds/cute-entry-v1/author.py` | Exit 0 | Five SVGs, 13,817 SVG bytes; editable source and manifest generated. |
| `python3 frontend/public/backgrounds/cute-entry-v1/verify.py` | Exit 0 | [`static-validation.json`](../../../frontend/public/backgrounds/cute-entry-v1/static-validation.json); XML, local-reference allowlist, manifest integrity, deterministic rebuild and flat contrast checks. Inert vector elements only; no external/character references. |
| `python3 scripts/mcr/check-plan.py` | Exit 0 | Planning/provenance gate only. Application build/tests/device/deployment NOT_RUN. |
| Actual in-app 390px before/after, all three screens | NOT_RUN | Must wait for Root's first actual World/Map cue and Resource21d's single browser/native/output allocation. Historical QA06 before images are not a current same-candidate render. |
| Relevant actual 360/430px captures, focus, 44px controls, no overflow, character-pixel invariance | NOT_RUN | Consumer must preserve existing functions, foreground, layout and readable text. No static palette probe certifies full composed accessibility. |
| CSS import/adoption and master ancestry | NOT_RUN | Root153 is sole publisher. Goal stays active through publication verification and required fixes. |

Sampled flat palette contrast is at least 4.5:1 for existing body, muted and
error text against every broad paper wash and for existing cream print against
each rose/plum text field. The light moon/cloud midzone is illustration-only;
white lettering must not overlap it. Actual layers, disabled controls and focus
indicators require rendered verification after adoption.
