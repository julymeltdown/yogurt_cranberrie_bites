# AR04 original world textures

These three 256×128 RGBA PNGs are newly authored pixel scenery for MCR. The
matching Tesseract documents contain editable native rectangles, with no embedded
images, fonts, characters, copied photographs, venue logos or third-party art.
The generator and review evidence live in `docs/mcr/art/evidence/themes/`.

Use `binding-inputs.json` for exact texture bytes, layout, palettes and proposed
C06 provenance fields. It is a local source handoff, **not a ThemeManifest**.
Delivery must supply actual committed product map/avatar pins and calculate the
final C06 content hashes. No fixture map/avatar or release registry is bound here.

License/provenance: original Codex-authored artwork for `julymeltdown/mcr`; no
external asset-license dependency. This statement records creation and source
inputs, not a third-party rights certification or a new license for the supplied
concept image. The supplied image is inspiration only and is not embedded.

32×32 cells; 8 columns, 4 rows; no padding or spacing. GID 0 is empty. GIDs 1–14
have the same meaning in all packs; 15–32 are transparent reserved cells. All
three decoration alpha masks are identical. Map collisions remain explicit and
independent. Textures have no real venue, route, arrival or admission authority.

Only the PNGs belong in the world texture resource allowlist. Editable sources
and this handoff metadata are not runtime resources. See the task document at
`docs/mcr/art/theme-packs.md` for validation, limitations and delivery ancestry.
