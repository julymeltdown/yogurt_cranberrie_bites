# Required spatial fidelity candidate v3

Status: **CONTRACT_READY candidate / NOT_RELEASE_BOUND / visual acceptance IN_PROGRESS**. This is required reference-fidelity work, not optional polish. Published geometry `20168…` and art v4/v5 are preserved. Do not use these new coordinates with their collision or release pins.

Canonical candidate geometry hash: `sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f`. Exact raw JSON and integer-projection hashes are pinned by `../quality-v6/manifest.json`. This is an art-owned virtual geometry proposal, not the WORLD-owned static Tiled/runtime descriptor.

The actual target comparison exposed the upper MAP district sitting too low. The new 48x81x16 proposal translates upper court roots upward by 179 virtual pixels and shifts the close WORLD camera from Y192 to Y13. Lower roots use a continuous monotonic redistribution: `Y <= 954 → Y - 179`; otherwise `775 + (Y - 954) * 521 / 342`. Each physical prop's footprint/body translates by its root delta, preserving its shape. The two north halls use explicit rooted depth96 so their artwork stays within the C06 -128px overhang and their collision polygons remain inside the map. Upper foliage deliberately reuses the red frame at the second fixed root to match the target's red canopy band; no random or extra tree roots are introduced.

The physical stage now has projected depth360; its MAP-only venue illustration has depth373, width300/height210, centerX335, and the label display anchor is335,147. MAP semantic art does not create solids. Ground paving polygons use the same continuous transformation, inserting kink intersections at old Y954, rather than an unrelated second layout. World and Map, and all three skins, use one new canonical geometry.

| Existing point | New cell X,Y |
| --- | --- |
| courtyard_entry | 29,26 |
| community_entry | 31,47 |
| stage_info / …0003 | 30,24 |
| park_info / …0002 | 38,27 |
| gifts_info / …0004 | 13,48 |
| wall_info / …0005 | 14,61 |

The complete existing UUIDs and truthful place records are unchanged. Cell projection uses transformed cell centers and deterministic nearest walkable cells within two cells; `revision-evidence.json` records every old/proposed point and root delta. `integer-projection.json` records all authored/runtime values and local foot anchors. Footprints retain float precision, while C06 image destinations and depths use explicit nearest-integer, ties-away-from-zero projection.

`verify-spatial-v3.py` checks all 3,888 collision and terrain cells against the actual polygons, unchanged identity/place records, footprint translations, physical overhang bounds, MAP-only separation, and all 12 spawn-to-point routes. `verification.json` records the result. `build-spatial-v3.py` reproduces the proposal and revision evidence without changing any original source.

Adoption requires an explicitly assigned new server/client map revision and actual static Tiled/descriptor publication by the owner of `../runtime/**`. This handoff does not activate or mutate the completed v4/map2 consumer. Root's bounded v4 adapter remains independent. Do not wake archived writers for status alone; coordinate the ready candidate's actual dependency and exact scope with the responsible developers.

All four places remain VIRTUAL_CONCEPT / CONCEPT_ONLY / UNREVIEWED, with photo permission NOT_APPLICABLE. They are not verified physical destinations. Original AR01/BE08/FE19 distinguish such concepts from PHYSICAL meeting-place obligations: these four states alone do not establish an external venue/operator prerequisite for virtual-art delivery. Source-art qualification and physical-place authorization are separate; no review field is fabricated, no guard bypassed, and physical beacon/IN02 scope is not removed.
