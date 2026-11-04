# Cute community backgrounds — FE27 / FE21 / FE24

Status: **IN_PROGRESS**. Initial 390px rendered evidence inspected; an asset
surface-fit correction awaits shared recapture. Publication and responsive
evidence remain **NOT_RUN**. This is a background-only contribution to
F01 Feed, M01 Meet, and G01 Gifts, not whole-task acceptance.

## Ownership and provenance

- Author chat: `01a1a534-4fcf-7291-abe3-a97ba2fcafcd`.
- Isolated worktree: `/Users/lhs/.codex/worktrees/bf97/mcr`.
- Branch: `codex/mcr-art-cute-community-v1`.
- Base: `bfa0193939c3eebd6c80ade85083c6530c6875e1`.
- Exclusive writes: new `frontend/public/backgrounds/cute-community-v1/**`
  and this document. Existing feature CSS rights were explicitly withdrawn.
- CSS adopter: COM01 `01a1a533-50a9-73a1-8d4f-a16bf0b69aee` after exact Root grant.
  Root153 retains imports, composition and master publication.
- Visual reference: the supplied ten-screen JPEG, SHA-256
  `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
  It was inspected read-only; no pixels or assets were copied from it.
- All seven SVGs are original, editable, code-native UI background graphics authored
  in this task. No third-party source, raster generation, embedded fonts, scripts,
  external resources, copied legacy art, or new runtime dependency is used.
- Character originals, source pack, derivatives, crop/framing, renderer, and
  product text, data, controls, routes and layout are outside this patch.

## Consumer interface

All URLs below are same-origin static files with MIME `image/svg+xml`.

| URL under `/backgrounds/cute-community-v1/` | Intrinsic / viewBox | Alpha | Intended use |
|---|---|---|---|
| `paper-tile.svg` | 64 × 64 / `0 0 64 64` | Opaque | Repeat at 64 CSS pixels behind unchanged row text; warm cream/blush paper, faint texture. |
| `feed-corner.svg` | 160 × 96 / `0 0 160 96` | Transparent | Single stationery/music ornament in an already empty Feed edge area. |
| `meet-corner.svg` | 160 × 96 / `0 0 160 96` | Transparent | Single ribbon/heart ornament in an existing unused Meet backdrop region. |
| `gifts-corner.svg` | 160 × 96 / `0 0 160 96` | Transparent | Single parcel/ribbon ornament in an existing unused Gifts backdrop region. |
| `feed-backdrop.svg` | 390 × 640 / `0 0 390 640` | Opaque | Broad blush/lilac cloud-paper field for an explicitly granted Feed backdrop. |
| `meet-backdrop.svg` | 390 × 640 / `0 0 390 640` | Opaque | Broad peach/sage cloud-paper field for the Meet container. |
| `gifts-backdrop.svg` | 390 × 640 / `0 0 390 640` | Opaque | Broad lilac/blush wrapping-paper field for an explicitly granted Gifts backdrop. |

The broad backdrops answer the refinement that the new background must read
softer and cuter at a glance. They should supply the dominant available backdrop
paint, with the warm paper tile behind dense rows. Small corners alone do not
satisfy that direction. Each backdrop is an original editable vector field, not
a screenshot, simulated product screen, or replacement for working components.
Suggested starting CSS is one nonrepeated backdrop sized `100% 100%` on an
already existing container; the adopter must inspect actual short/long states
and all three widths before choosing final sizing. This suggestion grants no
selector rights; Root records exact allowed background properties and URLs.
The three broad SVGs now specify `preserveAspectRatio="none"` so their broad
fields fill the existing painted viewport at row and container aspect ratios.
This adjusts only the background artwork; no element box or character changes.

The motifs have no faces, people, avatar silhouettes, product labels, stock,
venue guidance, or interaction semantics. They must remain CSS backgrounds;
they are not product photos, avatar fallbacks, action icons, or new DOM content.
Do not reserve extra space or resize existing controls to display them. If a
surface has no clear empty region, omit its ornament rather than place it under
text. Do not repeat the ornaments on every row. Preserve existing foregrounds,
focus outlines, minimum targets, disabled/access/stock states and typography.
No filter or opacity may be applied to a parent containing character rendering.
No decorative left bars or rails are present.

## Validation and remaining evidence

- `python3 scripts/mcr/check-plan.py`: exit **0**, planning provenance PASS;
  not application acceptance.
- Historical 390px Feed and Meet `postfix-12a5-repairs-tail` and Gifts
  `fidelity-v3/equal-gifts-390.png` were visually inspected. They are mixed
  historical candidates, not current before/after evidence.
- Source parsing and content pins are recorded in
  `frontend/public/backgrounds/cute-community-v1/manifest.json`.
- Inline Python 3 / `xml.etree.ElementTree` source qualification: exit **0**.
  All seven SVGs parse; only `svg`, `title`, `desc`, `path`, `g`, and `circle`
  elements are present, with no event handlers, resource hrefs, embedded rasters,
  or external/data URLs. Intrinsic dimensions match viewBoxes; each SVG is under
  4 KiB. SHA-256 and byte sizes were calculated from the exact committed bytes.
- The darkest unblended paint across the paper tile and broad fields is
  `#efd6cd`. Conservative contrast is **12.320:1** with Feed ink `#201b18`,
  **12.287:1** with Meet ink `#211b18`, **4.882:1** with Meet hint `#67594b`,
  and **5.886:1** with Gifts note `#5a4d43`. This is source-level arithmetic,
  not a substitute for computed browser contrast or checking ornament overlap.
- `git diff --check HEAD^ HEAD`: exit **0**. Changed-path enumeration is limited
  to this document and the granted new pack. Diff against the parent for
  `docs/source`, character source metadata, original avatars, and avatar renderer
  paths is empty; no existing application source was modified.
- Visual decoding and actual computed browser contrast: pending.
- Consumer source interface qualified: commit
  `8824401dec061102c9af33ba302855e48563170a`, new
  `frontend/src/shared/styles/cozy-community-backgrounds-v1.css`, references the
  pinned broad Feed asset twice (post and footer), Meet asset once (container),
  and Gifts asset once (row). Inline Python extraction of these four URLs and
  byte/SHA-256 comparison against this pack: exit **0**, all three unique opaque
  backdrop pins match. The rules specify center / no-repeat / `100% 100%` and
  matching fallback colors. This proves the source interface, not actual root
  import, decoded rendering, visual acceptance, or publication.
- Same-candidate actual in-app 390px before/after all three screens and
  360/430px responsive evidence: **NOT_RUN**, resource-coordinated capture only.
- Build and real-server behavior: **NOT_RUN** in this asset-only patch.
- Asset commit: `2a5c67e5d269db818431b28dcef1217b9782b71f`, parent is the base
  stated above. Master publication ancestry remains pending. A fresh fetch found
  `origin/master` at `7c25078f118da53dd9a524d0a58e707cca190f0f`; this asset commit
  was not its ancestor at that checkpoint (exit **1**, expected not-published).
- Coordinator21d consolidated validation into one Root153 actual-app job;
  individual source owners consume that packet and fix their own paths. No
  individual browser, native, build, image export or output workflow was started.
  Persistent goal remains active through adoption, developer validation,
  publication verification and fixes.

## Initial actual 390px inspection and required correction

The shared Root job `PANEL-COZY-202611040632Z` captured the actual default router
at candidate `c8a731e5c7f03c3fbcbd0581c6e872429806fa22`, Chrome
`156.0.8078.80`, 390 × 844, using DEV synthetic HTTP/C05/local admission. The
comparison uses the same candidate and fixture with four background integration
files restored to base `7c25078f118da53dd9a524d0a58e707cca190f0f`; Meet typography
is identical in both modes. This is desktop Chrome evidence, not physical-device
or live-service acceptance.

Actual packet (read-only, no separate artist workflow):
`docs/mcr/evidence/cozy-backgrounds-v1/native-01/report.json` in Root's publishing
checkout. Report SHA-256:
`59e0f2540a1fdc79f240230048385fda678c1029e9d3df3ab2a209320ae86da1`.
All six Feed/Meet/Gifts PNGs were visually inspected and their bytes/hashes
matched the report. The completed job reports zero errors, clean resource
shutdown, and exact per-pair visible-image source/rectangle/RGBA, text and control
rectangle/type/color/state equality. Candidate focus outline was solid for all
three. These checks do not prove every target is 44px or full interaction states.

| Screen | Baseline PNG SHA-256 | Candidate PNG SHA-256 |
|---|---|---|
| Feed | `e4d99d561d4c3b2ee5c52a60682920190885fea104efe286faf22e3d7f183851` | `40420f102f2cefc4fae6e9dad0fe3a6314425176f1e0238a1cd0a6ac5c4d7022` |
| Meet | `2d2fa482226c4b39f4c8ea958376957372c84c25c303d2819e37ab4a36a1e9cc` | `8cb3333b58838bc6cc73e08dd85f43198480d4eb603436d6bb081a0f004c97d7` |
| Gifts | `a9c8c7dff188c9262d132caec9302f40e2c3426ba4dd271447ed1df7cdd51f02` | `d9a42c33136e67d4ae6ca60bf3caa95811e55bbbad3309ffee36493b54c24679` |

Owner finding: the warmer fields are visible, with a large lilac cloud region in
Meet. Feed and Gifts show a narrow central art strip within wide flat-colored
rows: the original SVG default `xMidYMid meet` aspect policy keeps 390:640 artwork
inside the wide row viewport. This is unintended composition. The three broad
SVG roots now set `preserveAspectRatio="none"`; their colors and geometry sources
remain otherwise unchanged, with fresh manifest pins. No CSS, typography,
controls, character assets or renderer changes are required.

The corrected assets require another actual 390px inspection, plus the required
360/430px cases and computed contrast/target checks in Root's bounded follow-up.
The initial packet is not acceptance of the corrected candidate. Root was sent
the precise missing responsive cases; no individual artist capture was started.
