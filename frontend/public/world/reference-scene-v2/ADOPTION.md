# Stable PARADE scenery adoption inputs

Status: IN_PROGRESS visual fidelity; stable source proposal, NOT production release authority.

Public base: `/world/reference-scene-v2/`.

- `geometry-proposal.json`: proposal version 2; physical normalized cells, terrain, collision, spawns, IDs and source/reference revisions.
- `painted/layout.json`: illustration version 3; sixteen source frame rectangles, 22 rooted WORLD props, six MAP representations and cameras.
- `painted/resource-inputs.json`: illustration version 3; PNG bytes, dimensions and SHA-256 pins. Runtime loads exactly two PNGs: `painted/parade-ground.png` (768x1296) and `painted/parade-source-atlas-v3.png` (1254x1254 RGBA).
- `painted/scene-layout.svg`, `painted/map-layout.svg`, `painted/parade-world.tsrct`, `painted/parade-map.tsrct`: editable authored layouts, not runtime release manifests.

Canonical geometry hash: `sha256:20168a1df64cd3c72ea3b8cb576ca895dcff8c7edfb0e77886cc647fb1df37e5`.

One coordinate system: 768x1296 virtual pixels, origin top-left, X right/Y down. 48x81 cells, 16x16 pixels. Point feet are `((cell_x+.5)*16,(cell_y+.5)*16)`. Do not reinterpret virtual coordinates as physical arrival or safe routing.

WORLD uses the shared base ground. Draw source atlas rectangles at the dimensions/positions in painted layout, sorted ascending `depth_y`. Sort an actor at its foot Y between these props; never use its image top for depth. Preserve the painted canopy aspect correction and rooted foot/footprint from layout; no new collisions follow from alpha overhang. WORLD camera: `{"x": 160, "y": 192, "width": 584, "height": 584, "notes": "Stage-led close courtyard; no backend position/admission meaning."}`.

MAP uses the same physical geometry and base ground, then a non-authoritative green Plaza gathering-zone overlay. Replace any physical prop named by `overview_representations[].replaces_prop_id`, and add those named overview illustrations in foot-depth order. Cropped edge illustrations intentionally extend beyond the overview and are clipped to it. None has a collision footprint. They do not create invisible WORLD solids, new places or backend IDs. MAP venue hall represents the same stage identity as the open black WORLD truss; green is a semantic zone, not a second terrain map. Native editable MAP uses solid green; browser/SVG use a sampled lawn pattern.

Render `map_labels` as actual UI text/controls, not part of ground texture. A null `interaction_id` denotes unbound decoration; do not invent a request or place. Stage MCR lettering and CULTURE PARK scenery signage are allowed sprite text. Only these existing pairs are used:

| Interaction | Existing virtual place UUID | Cell X,Y |
| --- | --- | --- |
| stage_info | 6a5c196b-58e1-47d5-99bb-000000000003 | 30, 35 |
| park_info | 6a5c196b-58e1-47d5-99bb-000000000002 | 38, 38 |
| gifts_info | 6a5c196b-58e1-47d5-99bb-000000000004 | 13, 59 |
| wall_info | 6a5c196b-58e1-47d5-99bb-000000000005 | 14, 68 |

No supplied/legacy character image is present. The root owner binds selected source-pack derivatives separately. Never substitute native polygon draft skins for qualified painted REVENGE/DANGER; this published art increment is painted PARADE only.

Adoption dependency: the responsible backend geometry/release owner and frontend World/Map/root owners must coordinate actual release pins and authoritative collision. The asset owner does not write those paths. Do not display this changed walkable scene against an unrelated collision release. No backend schema, C06, manifest registry, current renderer or production root is silently modified here.

Developer-owned evidence: `verification.json`, `painted/verification.json`, `browser-verification.json` (24 cases/144 clear-foot samples), source hashes and native CLI render exits. The technical review is Canvas2D, not actual Phaser/app/server multiuser proof. Visual fidelity remains below the independent panel threshold after three rounds; see `docs/mcr/art/reference-scene-v2.md` for exact scores and remaining differences. Technical source stability is separate from target acceptance.
