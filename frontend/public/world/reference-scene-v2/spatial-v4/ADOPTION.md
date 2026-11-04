# Natural perimeter geometry proposal v4

Status: **CONTRACT_READY virtual-art geometry candidate; not release-bound.**

Normalized geometry hash: `sha256:0448d5086dc251c6a7aed8c136dbbbf751f95038ea1f7f98e3e40fe6e8067e6f`.

| Source | Raw SHA-256 | Bytes |
| --- | --- | ---: |
| `geometry-proposal.json` | `371b6bdd898f3e8757ba2dfdb96f9948d6f375e2f09353776a9cf6c0861b44c4` | 185,548 |
| `integer-projection.json` | `2d0cd0c9d7fd07f0ede84ac0e20a3f8a00b4addd1a40706a66e351c46dff3717` | 21,491 |

The finite 48×81 grid uses 16px cells, scene 768×1296. All 22 old integer placements and float footprint polygons are exactly frozen spatial-v3 values. Twelve new fixed unbound grove/root props bring the WORLD count to34. Each has explicit image bounds, frame, integer depth, solid root polygon and planted intent. None has a place or interaction identity. New images respect existing C06 overhang bounds. The six MAP-only illustrations remain noncollidable and overview-clipped.

`revision-evidence.json` lists every authored target/root/delta, allowed terrain, reason, root collision cells, all 72 changed cells and all 12 shortest grid routes. `changed_cells` is row-major and includes `cell_x`, `cell_y`, `previous:0`, `proposed:1`. Collision is recomputed by exact cell/polygon SAT, including the blocked perimeter. No previous solid cell is removed. Three named outer court/exit verges allow roots on existing paving; the other nine groups require lawn cells. Root candidates must stay within 64px of their authored target, clear old solid cells/boundary, at least 80px from all six named feet, and must not occlude their samples or the protected plaque face. Placement is deterministic, not random stamping.

Terrain, paving, ground, overdraw, existing places and labels, two spawns and four interaction cells remain unchanged. WORLD camera remains `{x:160,y:13,width:584,height:584}`; MAP remains `{x:0,y:0,width:768,height:1296}`. There is no player relocation, new destination or physical arrival claim. The existing four interaction/place pairs remain stage30,24; park38,27; gifts13,48; wall14,61. Spawns remain courtyard29,26 and community31,47.

The integer projection is authoritative art placement: copy its explicit values, never re-round old authored proposals. Every new image is positioned from its chosen root; old integer image values and float footprints remain exact. Natural canopies use ordinary foot-depth ordering. `protected_art_regions` records the Culture Park lettered face; foliage behind the sign is allowed, foreground foliage over the text is rejected.

`verify-density.py` passes all 3,888 collision cells, monotonic solid additions, exact72 changed cells,12 new unbound roots, unchanged old 22props/terrain/cells/cameras/places, C06 physical image bounds and12 routes between both spawns and all points. These are virtual-grid checks, not physical route safety or moving-avatar/multiuser proof.

Current published map `mcr_reference_scene_r3`, revision 4, geometry 899aca…, spatial-v3 and quality-v6 bindings remain frozen. This candidate **requires a separate new map revision greater than 4**; the proposed next revision is5, subject to allocation by the assigned runtime/backend owners. Source geometry proposal4 is not the same version number as server map revision 4. Actual new canonical Tiled, content hash, backend resource/release and consumer descriptors are **NOT_RUN** in the art lane. Never reseal revision 4 or mix this collision with its old Tiled/resources.

The reserved `runtime/**` writer receives the new canonical Tiled scope only through an explicit assignment. It can serialize this exact geometry to the existing finite orthogonal three-layer subset and reuse the frozen ground as the compatibility texture, preserving the two-PNG budget. This art package provides no competing canonical Tiled serializer. Backend must publish matching new map/theme resource pins; WORLD/MAP consumers must use the same new collision, all 34 objects and v8 metadata. Root's current spatial-v3 publication and reserved FE09 serial handoff remain primary and do not implicitly switch to this proposal. Archived writers are not revived for status; staffing follows the concrete published handoff.
