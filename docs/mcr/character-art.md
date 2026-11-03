# Selected character artwork

The user's 2026-11-03 instruction replaces previous character imagery with the
supplied artwork. The corrected ten-sheet base selection and ten subsequently
added sheets are both selected. The source pack is
[`mcr_user_characters_20261103`, revision 3](../../frontend/src/entities/avatar/character-source-pack.json).
Original JPEG bytes are preserved in `frontend/public/world/avatars/supplied-v1/`.
The catalog records filenames, SHA-256, byte sizes, dimensions, categories and
upload ordering and archived-duplicate provenance. These are user-supplied assets, not an upstream import or
newly generated substitutes.

The second upload (`2CFE2FDF-BB17-4FFD-A0EF-8E9B8AD671CD`) is the base selection.
It retains nine sheets byte for byte, omits the earlier duplicate body sheet,
and adds a face/body-accessory sheet as its Photo 10. The third upload
(`5F80E458-192D-42D9-9860-BCF6F22F013C`) adds ten new sheets and replaces none of
the base selection. Storage filenames and source IDs stay stable; attachment
numbers are scoped to their upload ID. The gallery labels each batch. The storage
directory name `supplied-v1` identifies the original collection, not the current
catalog revision. The archived duplicate is excluded from `files`, the frontend
lookup and the review gallery.

No character image had been imported into the application before this change.
The existing C06 avatar manifest contains synthetic IDs and no image resource.
Those fixtures remain contract-test inputs; they are not eligible product art.
All future character rendering and fallback artwork must use this selected pack
or derivatives prepared from it. A missing asset must never load a legacy image.

Base selection:

| Attachment | Source ID | Contents | Reference groups |
|---|---|---|---:|
| Photo 1 | `photo_01` | Six complete characters, two each under Revenge / Parade / Danger | 6 |
| Photo 2 | `photo_02` | Bodies in eight skin tones | 8 |
| Photo 3 | `photo_04` | Front hair | 16 |
| Photo 4 | `photo_05` | Back hair | 16 |
| Photo 5 | `photo_06` | Outfits | 16 |
| Photo 6 | `photo_07` | Bottoms and footwear | 16 |
| Photo 7 | `photo_08` | Eyewear | 18 |
| Photo 8 | `photo_09` | Eye pairs and expressions | 18 |
| Photo 9 | `photo_10` | Face treatments | 20 |
| Photo 10 | `photo_11` | Face/body accessories shown on busts | 18 |

Additional selection:

| Attachment | Source ID | Contents | Reference groups |
|---|---|---|---:|
| Photo 1 | `photo_12` | Headwear shown on full characters | 18 |
| Photo 2 | `photo_13` | Hair portraits with faces | 16 |
| Photo 3 | `photo_14` | Hairstyles around blank face regions | 16 |
| Photo 4 | `photo_15` | Detailed tops and outfits | 16 |
| Photo 5 | `photo_16` | Detailed bottoms and footwear | 16 |
| Photo 6 | `photo_17` | Eye pairs, brows and expressions | 16 |
| Photo 7 | `photo_18` | Makeup and face treatments on heads | 16 |
| Photo 8 | `photo_19` | Headwear and hair decorations | 16 |
| Photo 9 | `photo_20` | Jewelry and wearable accessories | 16 |
| Photo 10 | `photo_21` | Handheld items and props | 16 |

There are twenty unique selected sheets: ten base sheets and ten additions.
The catalog's count fields total 314 visible reference groups, not independently
composable runtime items or a deduplicated item inventory. Eye/earring pairs and
multi-object groups count once. In particular, additional Photo 8's last group
contains four clip designs; Photo 9's last group contains two piercing designs;
Photo 10 includes a group of wristbands. Individual item IDs/counts will be
established during extraction. The first upload's Photos 2 and 3 were identical;
the base selection's Photo 3 is front hair.
Historical `photo_03` is preserved in `archived_files` with its original body
sheet bytes and is not a selectable source.
The base selection's Photo 1 is 1280×853; all other sheets are 1280×960. All are RGB JPEGs without
alpha. The catalog's row/column hints describe the contact sheets, not validated
sprite rectangles, consistent anchors or animation frames.

## Frontend use

[`source-catalog.ts`](../../frontend/src/entities/avatar/source-catalog.ts)
exports the selected pack, all twenty unique sheets and an allowlisted source-ID
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
order. The base accessory sheet also includes full heads/busts; its bandages,
face marks, jewelry, headphones, scarves, masks and glasses require isolation
and suitable anchor/occlusion rules. The back-hair sheet is not a complete
four-direction animation set or an automatic one-to-one match with front hair.
The integrator must review and version the
composition contract before consumers persist new slot/ID shapes.

The additions contain more detailed outlines, faces and body proportions than
some base sheets. Retain them as additional supplied artwork; do not silently
replace the base designs or assume all pieces fit one body without review.
Separate baked-in faces/skin, opaque white face regions and contact-sheet grid
lines before composition. Headwear, neckwear, body accessories and handheld
props need explicit anchors, overlap rules and a reviewed slot/attachment
decision. The prop sheet supplies artwork, not new inventory, entitlement or
ticket behavior. Existing white details must remain intact during cutout work.

The six examples establish designs, not a rule that changes everyone's avatar
when switching a world theme. Avatar identity remains independent of theme.
Any temporary/static preview must be identified as such; the existing four
directions × idle/four walk frames obligation is still outstanding.

Status: **source selection and repository registration complete; AR03
IN_PROGRESS; production atlas and FE12 composition NOT_RUN**. The existing
contract fixtures are unchanged. This change does not accept AR03, C06, FE12,
FE13, device rendering or world integration.

## Registration checks

[Revision 3 evidence](evidence/character-sources-r3.json) records exact original
bytes for ten added attachments, unchanged base entries and images, all twenty
source hashes and production-build copies, strict build/lint and successful
Chrome decoding of all twenty images at 390px width. The review page rejects
unknown and archived source IDs, labels both batches and has no horizontal
overflow. The [prop-sheet review image](evidence/character-sources-r3-props-390.png)
was visually inspected. [Revision 2 evidence](evidence/character-sources-r2.json)
and [revision 1 evidence](evidence/character-sources.json) remain historical.
This verifies source delivery and its review page; it does not verify extracted
layers or production avatar rendering.
