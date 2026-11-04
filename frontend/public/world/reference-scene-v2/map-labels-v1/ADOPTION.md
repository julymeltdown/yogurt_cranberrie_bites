# MAP destination-label source composition v1

Status: **CONTRACT_READY source presentation; application adoption NOT_RUN.**

This fixes the concrete plain-label finding in the ten-screen comparison. The
five pictograms (stage, bag, cutlery, people and gift) are original hand-authored
filled vector polygons. PLAZA remains text-only, as in the target. All six names,
anchors and interaction bindings are exactly those in quality-v8. No new map,
place, route, collision, destination or interaction identity is created.

`presentation.json` is 13,268 bytes, SHA-256
`f9599a9859fe0b660fee591440dee2f7ab8e025a5f525df82091b61b091b23cc`.
It pins the exact v8 manifest and spatial-v4 geometry/projection from published
`492160b15465818eca172f4017f3816d58c989a9`. Normalized geometry remains
`sha256:0448d5086dc251c6a7aed8c136dbbbf751f95038ea1f7f98e3e40fe6e8067e6f`.
Every existing v8 and earlier source/runtime input remains unchanged.

## Consumer rendering instructions

Draw this overlay **only in MAP**, after scenery, in the same 768×1296 map
coordinate space. Scale it with that map, rather than using fixed CSS text sizes.
Each label contains an authored rectangle; the source anchor is retained, while
the visible rectangle is clamped to a 5px scene margin. Label rectangles do not
overlap. The symmetric dark outline and cream face surround the label. An icon's
tan square is its semantic pictogram field, not a decorative accent rail.

Use the declared 40px font and 42px line height. Text starts at rectangle X plus
padding plus the icon cell, if present. First baseline is
`rect.y + (rect.height - lineCount * lineHeight) / 2 + 32`; each following line
adds `lineHeight`. Rectangle width includes a 4px allowance for measured
FreeType/browser advance differences. `icons` contains filled 24×24 polygons;
scale by `icon_size/24`, position at `rect.x+padding` and
`rect.y+(rect.height-icon_size)/2`. Draw `icon_cutouts` in `icon_paper` after ink.
The saved SVG, native sources and source-reference renderer implement these
instructions. Text remains actual editable text, not an outlined bitmap.

The font is an explicitly renamed, static 800-weight instance of the existing
repository's SIL OFL Roboto Condensed variable source. Exact upstream revision,
source/license hashes and the transformation are in `presentation.json`.
`map-labels-bold.ttf` is 145,012 bytes, SHA-256
`f44e493f61e2d91c2156dd5abc720eeb6274a9c02b11d9097d1ea38d50011e8b`.
`OFL.txt` is retained byte-for-byte. No font download, package install, glyph
redesign or legacy icon reuse occurred. The assigned consumer must deliberately
load this pinned font, or coordinate and validate an explicit font substitution.

The overlay introduces **no additional PNG texture** and does not change v8's
two-PNG scenery budgets. Each native project packages its exact prior v8 atlas
and ground plus the one exact font. Runtime can draw native paths/text; do not
flatten the labels into a third scene texture. This sidecar is not a Scenery v1
schema extension or an implicit mandatory resource of existing theme manifests.

Only MAIN STAGE → `stage_info` and MERCH → `gifts_info` retain existing bindings.
The other four labels remain unbound decoration. They must not acquire hit
targets or claim physical meeting/pickup eligibility. A red selected-location
pin from the visual target is intentionally absent because no actual selected
state was supplied. Product controls and avatars belong to their assigned owners.

## Actual evidence and limits

Three actual native 768×1296 PNG exports and matching `.tsrct` projects contain
independently editable original icon shapes and eight text lines per era.
`verify.py` checks exact old scenery-layer membership, unchanged two packaged PNG
hashes, one exact packaged font, original anchors/bindings, nonoverlap/bounds and
actual rendered icon ink. **Every native pixel outside the six label rectangles
is exactly equal to its prior v8 PNG**, across all three eras.

`check.mjs` passed 18 actual Chrome 156.0.8078.80 source-composition cases through
two era cycles at 360/390/430px: 108 labels checked, exact font loaded, all text
advances within the declared padding, no page errors/overflow and one canvas.
Network evidence contains one frozen shared ground, three frozen era atlases and
one font. Nine target/candidate captures retain the supplied target unmodified.
The first browser check caught a 1.05px PLAZA advance difference; the final
4px allowance fixes it without changing the font, accepting overflow or reducing
the target typography. All final validation commands exited 0. The verbatim OFL license retains its
upstream trailing space on line21; the code/doc whitespace check excludes only
that exact copy after byte equality verification.

This is actual **source/native/browser art composition**, not current application,
Phaser, backend admission, source provenance qualification, physical review,
moving-avatar or multiuser acceptance. No fourth scored critic panel or 8.5
target-fidelity pass is asserted. The existing scored checkpoint remains frozen.
Full reference fidelity and concrete application handoff findings remain open.

Root owns its explicitly assigned v8 derivation/adoption; this supplemental
presentation does not alter its current source pins or block that work. Adoption
needs an explicit consumer patch and its own actual application checks. Existing
map4/geometry899/v6 bindings remain immutable; the new0448 map revision is
allocated only by the canonical backend owner. Artist writes no `runtime/**`,
application, backend, central contract, selected character or source-archive file.

Reproduce source/native output and checks with the existing environment:

```sh
python3 frontend/public/world/reference-scene-v2/map-labels-v1/build.py --native
python3 frontend/public/world/reference-scene-v2/map-labels-v1/verify.py
# Serve the repository on localhost:8896, then:
MCR_PLAYWRIGHT_PACKAGE=/absolute/path/to/installed/frontend/package.json node frontend/public/world/reference-scene-v2/map-labels-v1/check.mjs
```

The Playwright package path is an environment selection, not a committed
dependency or an instruction to install/upgrade it. Native executable is the
existing Tesseract CLI 0.4.0, revision
`d7ccb4bc20fc69f2356c53b80336e52cff31dcc5`; fontTools 4.62.1 is already installed.
