# Selected character artwork

The user's 2026-11-03 instruction replaces previous character imagery with the
ten supplied photographs. The selected source pack is
[`mcr_user_characters_20261103`, revision 1](../../frontend/src/entities/avatar/character-source-pack.json).
Original JPEG bytes are preserved in `frontend/public/world/avatars/supplied-v1/`.
The catalog records filenames, SHA-256, byte sizes, dimensions, categories and
duplicate provenance. These are user-supplied assets, not an upstream import or
newly generated substitutes.

No character image had been imported into the application before this change.
The existing C06 avatar manifest contains synthetic IDs and no image resource.
Those fixtures remain contract-test inputs; they are not eligible product art.
All future character rendering and fallback artwork must use this selected pack
or derivatives prepared from it. A missing asset must never load a legacy image.

| Attachment | Source ID | Contents | Visible choices |
|---|---|---|---:|
| Photo 1 | `photo_01` | Six complete characters, two each under Revenge / Parade / Danger | 6 |
| Photo 2 | `photo_02` | Bodies in eight skin tones | 8 |
| Photo 3 | `photo_03` | Byte-identical duplicate of Photo 2; kept but not counted again | 0 additional |
| Photo 4 | `photo_04` | Front hair | 16 |
| Photo 5 | `photo_05` | Back hair | 16 |
| Photo 6 | `photo_06` | Outfits | 16 |
| Photo 7 | `photo_07` | Bottoms and footwear | 16 |
| Photo 8 | `photo_08` | Eyewear | 18 |
| Photo 9 | `photo_09` | Eye pairs and expressions | 18 |
| Photo 10 | `photo_10` | Face treatments | 20 |

There are nine unique sheets and 134 visible examples/choices. These counts do
not assert 134 independently composable runtime items. Photos 2 and 3 share
SHA-256 `cb66e99ef86893e542728864933a694663cc4bdee9440df52de64a9323a46489`.
Photo 1 is 1280×853; all other sheets are 1280×960. All are RGB JPEGs without
alpha. The catalog's row/column hints describe the contact sheets, not validated
sprite rectangles, consistent anchors or animation frames.

## Frontend use

[`source-catalog.ts`](../../frontend/src/entities/avatar/source-catalog.ts)
exports the selected pack, the nine unique sheets and an allowlisted source-ID
URL lookup. Unknown IDs throw instead of returning old artwork. These source IDs
are for artwork preparation and review; they are not accepted AvatarConfig item
IDs and must not be saved to the backend as if they were.

The isolated browser fixture at
`/src/entities/avatar/__fixtures__/sources.html` displays the exact registered
files through that lookup. It is an artwork review page served by the existing
Vite development server, not a product route or avatar-editor implementation.

## Remaining preparation and contract changes

AR03 now prepares these supplied designs instead of inventing replacement
characters. Preserve each source and record derived-file hashes and source IDs.
Prepare transparent cutouts and pixel-aligned layers, with a common foot anchor,
before producing the actual runtime atlas. Do not make all white pixels
transparent: eye highlights, eyewear and pale hair/clothes contain intentional
white. Do not stretch or invent missing direction/walk frames and declare them
complete.

The current C06 five-slot layout (`body`, `eyes`, `hair`, `outfit`, `accessory`)
does not yet represent separate front/back hair, tops/bottoms and face details.
Bodies already contain eyes and base clothes; several outfits include hands and
lower hems, and face-detail sheets include eyes/mouths. These overlapping pixels
need deliberate separation, skin-tone handling and an explicit composition
order. Photo 5 is not a complete four-direction animation set or an automatic
one-to-one match with Photo 4. The integrator must review and version the
composition contract before consumers persist new slot/ID shapes.

The six examples establish designs, not a rule that changes everyone's avatar
when switching a world theme. Avatar identity remains independent of theme.
Any temporary/static preview must be identified as such; the existing four
directions × idle/four walk frames obligation is still outstanding.

Status: **source selection and repository registration complete; AR03
IN_PROGRESS; production atlas and FE12 composition NOT_RUN**. The existing
contract fixtures are unchanged. This change does not accept AR03, C06, FE12,
FE13, device rendering or world integration.

## Registration checks

[Verification evidence](evidence/character-sources.json) records exact copies of
all ten attachments, SHA-256/size/dimension checks, production-build byte parity,
strict TypeScript/build/lint, and successful image decoding in Chrome at widths
360, 390, 430 and 1280. The review page shows nine unique sheets, rejects an
unknown source ID and has no horizontal overflow. The
[390px review image](evidence/character-sources-390.png) was visually inspected.
This verifies source delivery and its review page; it does not verify extracted
layers or production avatar rendering.
