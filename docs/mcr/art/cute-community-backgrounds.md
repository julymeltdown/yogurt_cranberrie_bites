# Cute community backgrounds — FE27 / FE21 / FE24

Status: **IN_PROGRESS**. Asset source authored; in-app adoption, rendered evidence,
and publication remain **NOT_RUN**. This is a background-only contribution to
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
- Visual decoding and actual computed browser contrast: pending.
- Same-candidate actual in-app 390px before/after all three screens and
  360/430px responsive evidence: **NOT_RUN**, resource-coordinated capture only.
- Build and real-server behavior: **NOT_RUN** in this asset-only patch.
- Commit / master publication ancestry: pending. Persistent goal remains active
  through adoption, developer validation, publication verification and fixes.
