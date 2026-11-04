# After-show Bloom — initial mural art pack

Candidate source qualification is recorded in the companion evidence below.
Runtime producer validation and event provisioning are NOT_RUN by this author.

This is original, editable 128 × 128 pixel artwork for
`mcr-mural-initial-seed-art/1`, asset `wall-initial-motif-art-01`, revision 1.
The design uses a stepped cream flower, crimson center and leaves, a bent stem,
and sparse concert-night sparks on a near-black ground. It has no lettering,
logos, figures, faces or reused character assets. The asymmetric leaves and
broken corner rhythms leave dark open space for later participants.

The supplied reference was inspected directly at
`docs/mcr/design/reference-target-2026-11-04.png`, exact SHA-256
`d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`.
Its Wall panel informed high black/cream/red contrast and hard pixel steps.
No reference pixels, marching figure, text, logo or character source was copied,
sampled or traced. No exclusive rights warranty, third-party approval, operator
identity or production provisioning authority is asserted.

## Files and palette semantics

Exactly six files comprise this pack: `source.svg`, `pixels.bin`, `palette.json`,
`preview.png`, `manifest.json`, and this `README.md`. The source SVG is editable
integer rectangles grouped by palette index, with complete nonoverlapping grid
coverage and no external resources. `pixels.bin` contains exactly 16,384 unsigned
indices, origin top left, offset `y * 128 + x`. `preview.png` is opaque native
RGB, reconstructed from every binary index and its actual palette entry.

Index **0 = #171617** is the actual opaque background and immutable erase role.
The backend must preserve that index and color. The artist chose index 1 as
cream, 2 as crimson, 4 as cream fold shadow, 5 as deep red, 6 as highlight,
9 as red highlight and 11 as a small warm core accent. These are this artwork's
choices, not backend roles or defaults. All 16 ordered uppercase RGB entries
are explicit; unused supporting shades remain valid, intentional palette
capacity. Do not sort, infer, replace or silently reorder the palette.

## Actual qualification evidence

Authoring recipe and independent read-only checker:
`../../../../docs/mcr/art/evidence/wall-initial-motif-v1/author.py` and
`../../../../docs/mcr/art/evidence/wall-initial-motif-v1/qualify.py`.
Run only under the current named bounded source admission, from the repository:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 docs/mcr/art/evidence/wall-initial-motif-v1/author.py
PYTHONDONTWRITEBYTECODE=1 python3 docs/mcr/art/evidence/wall-initial-motif-v1/qualify.py
```

The checker parses a closed SVG element/attribute set; proves every SVG cell
equals the binary index; checks exact lengths/ranges/palette order, closed
manifest/provenance keys and file-byte SHA-256; CRC-checks the opaque PNG; and
compares every native RGB pixel and every 2× proof pixel with indexed palette
reconstruction. Seven in-memory negative probes reject short binary, index 16,
lowercase palette, reordered source colors, an invented manifest field,
corrupt PNG and an external SVG resource. The checker never changes pack bytes.

Machine results, actual native/390px visual disposition, source pins and resource
measurements are in `../../../../docs/mcr/art/evidence/wall-initial-motif-v1/`.
The ordinary 390px proof is explicitly **ART PACK ONLY / NOT A LIVE WALL**;
it is not application, authoritative canvas, client overlay or server evidence.
The immutable manifest digest is computed from the exact complete stored JSON
bytes. Source digest means `source.svg` bytes, not a screenshot or ownership
assertion. See the companion delivery document for the final digest and checks.

## Producer boundary

The manifest is event-independent: `OPERATOR_BOUND_AT_PROVISION`. No production
UUID, cooldown, state or operator identity is invented. A responsible local
producer must bind a canonical event, the exact selected manifest digest and
verified source-qualified pack, background index 0, explicit cooldown within
0–3,600,000 ms, and explicit `EDITABLE`, `PAUSED` or `FROZEN` state. Only atomic
pristine revision-1 creation and exact same-seed pristine retries are permitted;
existing content, history, checkpoint or different seed must be refused. This
pack implements none of that server behavior and grants no client authority.
