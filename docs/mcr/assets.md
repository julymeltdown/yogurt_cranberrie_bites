# C06 asset contracts v1.0.0

Current character-art decision (2026-11-03): use the user's ten supplied images
and their reviewed derivatives exclusively. See [selected artwork](character-art.md)
for the preserved source pack, frontend lookup, duplicate handling and required
layer-contract follow-up. The synthetic fixtures below remain metadata tests;
they are not product art or fallback images. Source registration does not turn
the JPEG sheets into the transparent, animated atlas described below.

Status: **CONTRACT_READY for parent review; not frozen or ACCEPTED**. This is an isolated common-contract handoff, based on `605b858b3baa2bb93ce4a01a6620cd25bc648c14`, branch `codex/mcr-contracts-C06`. Only the parent integrator reviews/freezes the contracts, integrates consumer work, and publishes to `julymeltdown/mcr/master`. No remote delivery is claimed here.

## Public schema references

Every schema uses JSON Schema Draft 2020-12. All DTO property names are `snake_case`; the named definitions below are the public integration surface. Every object rejects unknown properties. The schema ID prefix is `https://contracts.mcr.invalid/assets/v1/`. This is an identifier namespace, not a network service. The validator registers all schemas locally and refuses network retrieval.

| Public name | Repository reference |
|---|---|
| `AvatarConfig` | `contracts/assets/v1/avatar-config.schema.json#/$defs/AvatarConfig` |
| `AvatarManifest` | `contracts/assets/v1/avatar-manifest.schema.json#/$defs/AvatarManifest` |
| `MapManifest` | `contracts/assets/v1/map-manifest.schema.json#/$defs/MapManifest` |
| `ThemeManifest` | `contracts/assets/v1/theme-manifest.schema.json#/$defs/ThemeManifest` |
| `ThemeId` | `contracts/assets/v1/common.schema.json#/$defs/ThemeId` |
| `PlaceId` | `contracts/assets/v1/common.schema.json#/$defs/PlaceId` |
| `Place` | `contracts/assets/v1/map-manifest.schema.json#/$defs/Place` |

For an absolute reference, replace `contracts/assets/v1/` with the ID prefix. For example: `https://contracts.mcr.invalid/assets/v1/avatar-config.schema.json#/$defs/AvatarConfig`. C03 Bootstrap's `avatar`, `avatar_manifest`, and `map_manifest` should reference these definitions. Public HTTP generation must preserve/reference their local dependency closure, including `common.schema.json`, instead of maintaining another avatar shape. C07 still owns actual generated-client/Zod/OpenAPI producer/consumer qualification. OpenAPI tooling compatibility has not been executed by this lane.

`contract_version` is exactly `1.0.0`. Breaking changes need another versioned schema directory/ID and parent review; do not silently change a frozen v1 shape. Catalog content changes increment `manifest_version`, `map_revision`, or `theme_revision` as appropriate and recompute all dependent hashes. Released `(ID, revision)` pairs must be immutable in the eventual server registry; this static bundle validator cannot prove registry history or optimistic concurrency.

## Avatar wire shape and composition

`AvatarConfig` requires these fields:

```json
{
  "contract_version": "1.0.0",
  "kind": "avatar_config",
  "manifest_id": "avatar_fixture",
  "manifest_version": 1,
  "item_ids": {
    "body": "body_fixture",
    "eyes": "eyes_fixture",
    "hair": "hair_fixture",
    "outfit": "outfit_fixture",
    "accessory": "accessory_fixture"
  },
  "skin_tone_id": "tone_fixture"
}
```

`accessory` may be explicitly `null`; the other four slots and skin tone are required. Asset catalog IDs match `^[a-z][a-z0-9_]{0,63}$`; clients cannot supply image URLs or extra layers. The authoritative catalog checks every item against its assigned layer, the skin tone whitelist, and exact manifest identity/version. Presets contain complete `AvatarConfig` values and must pass the same checks. The fallback preset ID must resolve. Catalogs require all five layer categories and unique item, tone, and preset IDs. Fixture counts are deliberately minimal, not the production inventory target.

`AvatarManifest` contains `manifest_id`, `manifest_version`, `content_hash`, `provenance`, `layer_order`, `skin_tones`, `atlas`, `presets`, and `fallback_preset_id`, plus `kind` and `contract_version`. `skin_tones` maps stable IDs to six-digit uppercase hex colors. The draft v1 layer order is back-to-front `body`, `eyes`, `hair`, `outfit`, `accessory`. In this layout hair and outfits must leave the eyes readable. Additional front/back sublayers would require a reviewed contract revision rather than an undocumented compositor convention.

The atlas layout is fixed and untrimmed:

| Property | Rule |
|---|---|
| `atlas.cell` | `width_px: 64`, `height_px: 64` |
| Atlas and every item's `anchor` | Local cell pixel `x: 32`, `y: 60` |
| `frame_layout.direction_order` | Rows `down`, `left`, `right`, `up`, in that order |
| `frame_layout.idle_column` | Column `0`, one idle frame per direction |
| `frame_layout.walk_columns` | Columns `[1, 2, 3, 4]`, four walk frames per direction |
| Each item `region` | `width_px: 320`, `height_px: 256`; cell-aligned `x`, `y` |
| Texture bounds | Multiples of 64; all item regions fit and do not overlap |

The source rectangle for direction row `r` and animation column `c` is `(region.x + 64*c, region.y + 64*r, 64, 64)`. The frame key is deterministically `item_id/direction/idle/0` or `item_id/direction/walk/0..3`; walk indices map in order to columns 1..4. No packed rotation, trimming, spacing, or padding is encoded in v1. Layers share the same cell-local anchor. Phaser consumers can normalize the foot origin to `(32/64, 60/64)`.

The contract chooses a deterministic frame order, not animation speed or actual artwork. Server writes reject invalid/unknown items and stale catalog versions. A renderer encountering an unavailable config should temporarily render the manifest's complete fallback preset and request fresh state; it must not silently persist the fallback or mix layers from different versions. The same config/catalog must drive world, profile, feed, party, and giveaway representations. CSS display height, actual thumbnail export, texture loading, and fallback rendering remain consumer work.

## Map, place, and theme identity

`MapManifest` contains `map_id`, `map_revision`, `content_hash`, `geometry_hash`, `provenance`, `tilemap`, `geometry`, and `places`, plus `kind` and `contract_version`. `tilemap` declares `TILED_JSON`, resource presence, path, and hash. The fixture is normalized contract geometry for deterministic tests; it is not a production Tiled export. Actual Tiled export-to-geometry parity belongs to AR02/FE14/BE12 integration.

`geometry` is the one shared authoritative layout:

- `width_cells`, `height_cells`, `tile_width_px`, `tile_height_px` define a finite grid. Map tile dimensions are independent of the fixed 64×64 avatar sprite cells. The fixture uses 32×32 map tiles.
- `collision_id` names the collision release. `collision_grid` is row-major `[cell_y][cell_x]`, with `0` walkable and `1` blocked. Outside-map cells are blocked.
- `terrain_layer` and `overdraw_layer` are equal-sized row-major tile-ID arrays. `0` is empty; positive IDs address cells of the theme texture in row-major order starting at 1. IDs must fit every theme's declared tile texture dimensions. Overdraw changes presentation only.
- `spawns` have unique `spawn_id`, `cell_x`, and `cell_y`. `default_spawn_id` must resolve. Every spawn must be in bounds and walkable.
- `interactions` have unique `interaction_id`, `place_id`, `cell_x`, and `cell_y`. Place IDs must resolve, interaction cells must be walkable, and every interaction must be reachable from every spawn through four-neighbor walkable cells.

Coordinates start at the top-left; x increases right and y down. A cell-centered foot position is `((cell_x + 0.5) * tile_width_px, (cell_y + 0.5) * tile_height_px)`. This is virtual pixel geometry. It carries no GPS, physical distance, route safety, arrival, room assignment, or ticket/admission meaning. Runtime collision, speed, interaction range, and server authority are separate BE12/FE15 obligations.

`places` have canonical lowercase UUID `place_id` values, English `label`, `place_kind`, `verification_state`, `closure_state`, `accessibility_notes`, and `provenance`. Place identity is independent of theme and interaction IDs. An interaction's place reference also supplies its landmark association.

| Place metadata | Meaning |
|---|---|
| `place_kind: VIRTUAL_CONCEPT` | Requires `verification_state: CONCEPT_ONLY`; cannot be a physical beacon destination |
| `place_kind: PHYSICAL` | Requires `PLACE_REFERENCE` provenance and a non-null source URI; verification is `UNVERIFIED` or `VERIFIED` |
| `verification_state: VERIFIED` | Additionally requires reviewed provenance with reviewer/time; still needs operator/source-truth qualification |
| `closure_state: OPEN / CLOSED` | Explicit closure information, independently of verification |

The fixture uses UUID `00000000-0000-4000-8000-000000000001` for **Virtual fixture landmark**, with `CONCEPT_ONLY`, unreviewed synthetic provenance and no photos. Original fixture provenance cannot verify a physical place. Neither that UUID nor any fixture label identifies a real venue. BE08 must reject unknown/closed/unverified/concept-only destinations for new physical beacons and recheck current operator state. Static asset metadata cannot authorize physical sharing. Audience, revision, confirmation, expiry, and explicit physical arrival remain C04/BE19/FE20 concerns.

`ThemeId` is exactly `REVENGE | PARADE | DANGER`. `ThemeManifest` contains `theme_id`, `theme_revision`, `content_hash`, `provenance`, `map_id`, `map_revision`, `map_content_hash`, `geometry_hash`, `collision_id`, `place_ids`, `interaction_ids`, `avatar_manifest_id`, `avatar_manifest_version`, `avatar_content_hash`, `texture`, and `palette`, plus `kind` and `contract_version`.

All three themes reference the same map content/revision, geometry hash, collision ID, exact sets of place/interaction IDs, and avatar content/version. Themes have no geometry, collision overrides, per-theme outfits, spawn overrides, or room IDs. Switching theme changes only presentation. The palette keys `background`, `terrain`, and `accent` are fixture presentation metadata, not a complete production UI token system or a contrast claim. Texture metadata supplies dimensions and future resource identity; no production pixels are included.

## Hashes, provenance, and validation boundaries

Hashes use `sha256:` followed by 64 lowercase hexadecimal digits. `content_hash` covers the complete JSON document with only its own top-level `content_hash` removed. `geometry_hash` covers the complete `geometry` object. Theme references bind both map and avatar content hashes. Changing provenance or palette changes content identity; geometry changes propagate through geometry, map, and theme hashes.

C06 canonical bytes are JSON with keys sorted lexicographically, no whitespace, ASCII escaping of non-ASCII strings, and integers only. Equivalent Python encoding is `json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(',', ':'), allow_nan=False).encode('ascii')`. Object key order and file indentation do not affect hashes; array order does. Duplicate JSON keys, floats (including `1.0`), NaN and infinities are rejected. This is the documented C06 encoding, not a claim to implement general RFC 8785. Consumers must reproduce these bytes before comparing digests; generated cross-language parity is still C07 work.

`provenance` records `source_id`, `source_kind` (`ORIGINAL_FIXTURE`, `ORIGINAL_ART`, or `PLACE_REFERENCE`), `source_revision`, nullable `source_uri`, `author`, `license`, `review_state` (`UNREVIEWED`, `REVIEWED`, or `REJECTED`), nullable `reviewed_by`/`reviewed_at`, and `photo_permission` (`NOT_APPLICABLE`, `UNREVIEWED`, `GRANTED`, or `DENIED`). A reviewed record requires reviewer and RFC3339 timestamp; other states require null review fields. Source revisions and hashes do not establish copyright permission or venue truth by themselves.

Textures explicitly declare `image_state: NOT_PROVIDED | PROVIDED`, nullable `path`/`sha256`, and `width_px`/`height_px`. `NOT_PROVIDED` requires null path/hash. `PROVIDED` requires a PNG path/hash. The analogous map `resource_state` describes the future Tiled JSON. Declared dimensions let metadata tests check atlas bounds and tile coverage without inventing an image digest. The executable does not read these referenced resource files: resource byte/hash verification and actual atlas pixel coverage always report `NOT_RUN`, even for a declaration of `PROVIDED`. It is a contract validator, not a production resource release gate.

The complete golden bundle is six JSON documents under `contracts/fixtures/assets/`: one config, one avatar manifest, one map manifest, and `theme-revenge.json`, `theme-parade.json`, `theme-danger.json`. All three skins share the same synthetic 5×5 geometry with a central blocked cell, one spawn, one interaction and one virtual place. The atlas reserves five item regions, totaling 100 declared animation cells. There is **no PNG atlas, no production Tiled file, no verified real venue, and no actual frame/pixel coverage evidence**.

`validate-assets.py` validates every JSON file in the selected bundle, validates schema documents, resolves schema references offline, verifies manifest/geometry hashes, rejects duplicate identities, and checks the cross-file invariants above. A bundle must have exactly one config/avatar/map and exactly the three themes. Missing, empty, malformed, unknown-kind, or inconsistent bundles fail with exit code 1. The default validator output uses `CONTRACT_READY` to describe the validated bundle; it is not an integration/freeze/acceptance decision.

## Provenance and evidence for this handoff

This contract, validator, tests, and all fixtures are newly authored for C06; no Bigyeting/port-rust rendering, code, art, UI token, or asset was copied. The source is the active specification `MCR_IMPLEMENTATION_PLAN_V2_EN.md` at base `605b858b3baa2bb93ce4a01a6620cd25bc648c14`, especially §§5.2–5.4, 7.1, 7.4, C06 and AR01–AR04. Its SHA-256 at this base is `e19732ac9c5c5d5397652bf047645da0640ec72c97f59869974680c4e7f69a36`. `docs/mcr/project-assessment.md`, `docs/mcr/execution-plan.md`, `AGENTS.md`, and `docs/mcr/source-provenance.json` supplied the review/ownership context. Preserved upstream narrative revision is `dd47ed502edd5d4f9bc4b726e9088b1ee0f0547b` as recorded there. `docs/source/` remains unchanged.

The destination paths are exactly the five schemas under `contracts/assets/v1/`, six fixtures under `contracts/fixtures/assets/`, `scripts/mcr/validate-assets.py`, `scripts/mcr/tests/test_assets.py`, `scripts/mcr/requirements-contracts.txt`, and this document. No frontend/backend source, central manifest, lockfile, contract registry, CI, or infrastructure file is part of this patch. There are no migrations.

Dependencies are the already installed Python packages `jsonschema==4.26.0` and its referenced registry library `referencing==0.37.0`, pinned at their installed versions in `scripts/mcr/requirements-contracts.txt`. The recorded Python runtime is 3.14.3. No package installation or frontend dependency change was required. This requirements file pins the directly used libraries, not every transitive dependency.

Executed checks on this worktree:

| Command | Exit/result |
|---|---|
| `./scripts/mcr/validate-assets.py` | 0; all six documents and three themes validate; atlas image coverage `NOT_RUN`, image state `NOT_PROVIDED` |
| `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/mcr/tests -p test_assets.py -v` | 0; 21 test methods, including parameterized negative cases |
| `python3 scripts/mcr/check-plan.py` | 0; preserved source checksums, exporter probes and planning structure pass; separate from application qualification |
| `git diff --cached --check` | 0; staged patch whitespace check |

The negative suite rehashes altered manifests before testing semantic violations, so stale-hash rejection cannot hide missing semantic validation. It covers anchors/cells/frame order/count, overlap/out-of-bounds regions, unknown item/tone/preset/version, wrong layer, client URLs, duplicates, theme/map/avatar/place/interaction identity mismatches, stale hashes, blocked/outside/unreachable spawn/interaction cells, unknown default spawn, grid dimensions, unknown tile IDs, forbidden geometry overrides, unreviewed place verification, and malformed/empty CLI input.

The local delivery commit is the Git commit containing this handoff, descended directly from the stated base; its resolved SHA is reported to the parent alongside the changed-path list. Push is intentionally delegated to the parent. C06 parent review/freeze and downstream generated types are pending. C01/C02/C08 status is owned by the parent and is not re-certified from this branch's historical planning files.

## Open art and release choices

AR01 still needs authorized venue references, source/photo rights, review and closure evidence, and operator-controlled publication. AR02 still needs the real Tiled export and measured agreement with server collision and client loading. The contract fixture's walkability proof is limited to its encoded grid.

AR03 now derives its artwork from the user's [selected character sheets](character-art.md), which supply six preset designs and broader part choices. Transparent cutouts, compatible layer separation, animation timing and actual atlas production remain. Artist review must confirm that the proposed five-layer order and frame convention support the supplied silhouettes; the new front/back-hair, bottom and face-detail categories require a reviewed layer/layout decision before freeze. Pixel clipping, missing frames, eye readability, approximately two-head proportions and 56–72 CSS-pixel display are not validated by metadata tests.

AR04 still owes three original skin packs, final palettes/decorations, licenses, actual texture/hash verification, readable contrast and performance budgets. The sample palette values are deterministic fixtures, not accepted art direction. Actual mobile-scale checks at widths 360/390/430, iOS Safari/Android Chrome, two-account live-world flows, one-game/one-socket behavior, persistence/concurrency, real resource loading and release readiness are **NOT_RUN** here. No AR task, QA06, IN01, or IN05 acceptance is claimed.
