# Map4 local input deployment policy

Revision: `mcr_map4_local_intent_r1_20261104`.
Reviewer: MCR Root integration engineering review, 2026-11-04.

This newly authored deployment configuration supplies the explicit
`VITE_MCR_LOCAL_INPUT_CONFIG` input required by the production application. It
is separate from the unchanged source-bound release bundle and server protocol.
The live operator must supply its exact JSON through Vite configuration; the
application keeps its fail-closed behavior when configuration is absent.

The reviewed map is `mcr_reference_scene_r3`, revision 4. Its source manifest is
`backend/releases/reference-scene-v2/quality-v6/map-manifest.json`, SHA256
`81f6890b114a75a2885032c4f5c38a7fac685203fbde9a4481eef6b9c2bbb632`.
It has 48 by 81 cells of 16 by 16 pixels.

The values are deployment choices for local intent and path generation: 96
pixels per second expresses six map tiles per second; the 8 by 4 pixel
bottom-center foot occupies half a tile in width and a quarter tile in height;
the 32 pixel interaction search radius covers two tiles. Zoom 1 keeps the
native world scale. Foot bounds and every configured spawn are checked against
the actual collision grid. The application must also run its existing strict
configuration and map-bound policy validation before entry.

These choices are not taken from test fixtures, avatar sprite dimensions or
inferred server ticks. They do not attest authoritative C05 speed, collision,
interaction permission, prediction agreement or physical arrival. The server
continues to consume and validate actual inputs independently. A successful
browser journey must still demonstrate real movement and recovery; supplying
this policy is not that acceptance evidence.

The engineering review approves this bounded deployment input for the map4 and
approved static-avatar journey. It is not a claim of user art approval, a map6
admission, or a change to any retained contract or release resource.
