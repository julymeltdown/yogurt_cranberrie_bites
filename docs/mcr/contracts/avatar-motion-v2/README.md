# Selected-source avatar motion 2.0.0

This is the C06-MOTION-V2 producer contract, independent of map revision 4 and
IN01. It establishes a closed, source-derived inventory and typed candidate
validation. It does not activate a public parser, persistence route, registry,
world dispatcher or production artwork. The immutable 1.0.0 and 1.1.0 contracts
remain unchanged.

## Status and authority

- Producer shape/source checks: CONTRACT_READY; Rust execution evidence is recorded
  in `validation.md` after its allocated run.
- Art qualification: CANDIDATE_ONLY. `production_eligible` is false, preserving the
  source descriptor's `productionEligible:false`. The Rust admission API always
  returns `ArtNotReviewed` for this release. No caller boolean, schema success or
  invented review receipt changes that result.
- Product/public integration, persistence, canonical legacy selection adapters and
  full C06 acceptance: NOT_RUN in this producer packet; delivery root owns them.
- Source pack: `mcr_user_characters_20261103`, revision 3. The 20 selected originals
  remain byte identical. The archived photo_03 is excluded. The 314 visible source
  groups are contact-sheet counts, not this release's inventory.

## Files and immutable release

`contracts/assets/v2/avatar-config.schema.json` describes exactly two disjoint
config representations. `contracts/assets/v2/avatar-manifest.schema.json` uses a
whole-document `const` for immutable manifest revision 1. This is deliberate: it
closes every frame, source pin, attribution, palette, nullable field and composition
rule. Adding an item or changing a source requires a new explicit manifest revision
and corresponding contract; it cannot silently extend this catalog.

`backend/releases/selected-character-motion-v2/manifest.json` is the actual
canonical manifest. Both manifest and config carry `contract_version:"2.0.0"`,
`manifest_id:"mcr_selected_motion_20261103"`, `manifest_version:1`; their `kind`
values are `avatar_manifest` and `avatar_config` respectively. The manifest's
representation is `selected_source_motion`. Its path/resource entries are pinned
repository-local resource identities, never client-selectable URLs. A public
resolver may map `frontend/public/` to a static URL only after checking this exact
allowlist; clients do not supply paths, hashes, frame indexes or sheet IDs.

The six preset IDs are `motion_v2_photo01_front_01` through `_06`. They map in order
to artwork keys `photo01-front-01` through `photo01-front-06`. These are new v2
canonical IDs, not aliases asserted to exist in a previous catalog.

Layered IDs are `motion_v2_body_0..7`, `motion_v2_eyes_0..3`,
`motion_v2_hair_0..5`, `motion_v2_outfit_0..5` and
`motion_v2_accessory_0..3`. The notation denotes each individual closed ID; the
schemas contain the explicit enumerations. Body, eyes and hair are required.
Outfit and accessory are required keys whose values may be null. There is no
independent skin-tone ID. Each of the eight body IDs selects its exact source-derived
four-color semantic RGB palette as well as the body geometry.

A preset config has exactly the four header fields, `representation:"motion_preset"`
and `preset_id`. A layered config has exactly the headers,
`representation:"motion_layered"` and `item_ids` with exactly the five slot keys.
Mixed representations, unknown keys, cross-slot IDs, URLs, legacy IDs, unknown
versions and missing nullable keys are rejected. Raw Rust JSON rejects duplicate
keys and trailing data; JSON Schema operates on already parsed JSON values, so a
transport must also enforce its own raw-JSON duplicate and size policy. JavaScript
cannot distinguish the numeric spellings `1` and `1.0`; Rust's typed revision is an
integer. The numeric revision value is always 1.

## Geometry and composition

Every output character has 20 cells: four directions in `down,left,right,up` order,
with `idle,walk-0,walk-1,walk-2,walk-3` in each direction. Source direction aliases are
`front,left,right,rear`; source evidence's other human walk labels are not extra
frames. Each output cell is 64 by 64 pixels, foot anchor (32,60).

The single pinned atlas is 1280 by 1856, 20 columns, with 576 referenced physical
cells. Its 29 rows have capacity 580; the four unused cells are not inventory. The
six complete presets use indices 0–119. Wardrobe indices in the combined descriptor
already include the 120 offset. Never add it again. Body/outfit animated directions
have five distinct cell indexes. Stationary hair/eyes/accessory attachments may
share an index across all five phases. Each manifest entry preserves its exact
source pointer and local artwork key; each preset preserves its 20 source frames,
poses and supplied/authored attribution.

Composition is ordered body, hands, outfit, eyes, hair, accessory. With an outfit,
render body/head only for local y < 42; without an outfit render the full body.
Use each outfit direction's `hand_frames` alpha mask filled with its selected body's
`skin_palette.colors[2]`. Render outfit above the hands. There is no blanket skin
tint or baked-hand substitution. Up/rear eyes are explicitly invisible. Keep
nearest-neighbor sampling and the original 64px geometry. Fallback is `NONE`;
unknown IDs or missing/unverified resources must fail explicitly.

## Authorship and provenance

The code is newly authored for this repository. No rendering code is copied.
Catalog mappings are derived from the selected art candidate at art commit
`bb7d21b3e931a9dc88056c1c1c436eb9817292da` and the published art receipt
`145d0e21b5ce92c55a04c31dc447267f78014a99`; the exact descriptor SHA256 is
`6cf3aad25ceba681f4c0306d1139109ee6c07508de617ecf1c0b08b1d43fb950`.
The atlas SHA256 is
`1cd8f05af8d76d58243e2f6e143961953b4d4291fdd553df8c8d33aabd700dbd`.
The candidate descriptor and all ten handoff pins are read only.

Six front idles originate in selected photo_01; 24 front walk cells articulate
supplied pixels. The remaining 90 side/rear preset cells are authored reconstruction
and articulation, not originally supplied side/rear pixels. Wardrobe body, hair,
outfit and accessory surfaces are authored source-faithful reconstructions; eye
cutouts and semantic body palettes preserve selected source provenance. Exact
method labels, source references, authored inputs and hashes remain in the catalog
and pinned evidence (`component-source-inputs.json`, `authored-reconstruction.json`,
per-family descriptors). These descriptions do not certify visual review.

## Consumer API and validation

Rust entrypoint: `concert_domain::avatar_motion_v2`.

- `decode_config(&[u8])` returns a closed typed `AvatarConfig` with bounded input.
- `Catalog::decode_candidate(&[u8])` requires typed fields and exact equality to the
  immutable catalog. It rejects relabelled qualification, flags, source changes,
  arbitrary paths, frames, direction aliases and layer/skin semantics.
- `Catalog::resolve(&AvatarConfig)` returns typed preset or layered references.
  Config constructors and decoder only expose IDs present in the closed catalog.
- `Catalog::resources()` and `verify_resource(path, bytes)` verify local bytes,
  length, SHA256 and pinned PNG header dimensions. The domain never opens a client
  path or URL. The producer Python verifier additionally decodes every PNG with
  Pillow. Supplied JPEGs are verified byte for byte against source-pack pins.
- `Catalog::admit_production()` always denies this candidate. A future reviewed
  release needs an explicitly versioned admission policy and real review evidence.

The frontend owner may derive a TEST_ONLY authority from the exact schema/catalog,
with fixture-only persistence. That authority is not permission to bypass existing
production dispatch. Root remains responsible for any public contract adapter,
legacy selection/persistence bridge, registry activation and actual reviewed art
admission. The old contracts keep their existing allowlists and default denials.

Shared deterministic configs live under `contracts/fixtures/assets/motion-v2/`.
`build_catalog.py --check` compares regenerated outputs against committed files;
`validate.py` independently verifies schemas, both fixture outcomes, sources,
geometry, masks, inventory, transparent rear eyes and manifest tamper refusals.
Python dependencies are the already available `jsonschema` and Pillow. Rust uses
only existing `serde`, `serde_json`, `sha2` and `thiserror` dependencies; no manifest,
lockfile, proto, registry or old parser changes are included.
