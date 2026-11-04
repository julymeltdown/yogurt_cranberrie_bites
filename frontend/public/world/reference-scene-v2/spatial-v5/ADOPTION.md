# Continuous diagonal terrain proposal v5

Status: **CONTRACT_READY virtual geometry candidate; NOT release-bound.**

Normalized geometry:
`sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395`.

The source comparison showed a southwest diagonal approach; spatial-v4 instead
followed a long vertical detour. This proposal replaces that detour and redundant
cross-route with centerline `(-20,1030) → (310,741) → (557,494) → (800,300)`,
radius43px, and a lower west connector of radius32px. Northern courtyard and
approaches, the descending road and the lower venue link remain.

Only canonical terrain changes: **560 of3,888 cells**. Collision bytes and its
identifier remain exact spatial-v4, with no changed solid cell. All34 WORLD props,
integer placements, floating footprints, cameras, two spawns, four interaction
cells, four existing place identities/states and six label anchors remain exact.
Eight MAP-only representations remain exact published v9. They add no solids or
identities. Original decorative ground ellipses are removed from the new ground.

`revision-evidence.json` records every old/new terrain cell and12 reachable routes.
Four manually annotated source-road centers are7.24/0.53/0.04/13.27px from the
new centerline, versus162.83/39.68/98.26/121.15px from the old primary route.
These approximate semantic annotations are **not an independent fidelity score**.
`verify-diagonal-spatial.py` recomputes all terrain cell centers, all collision
cells with convex-footprint SAT, normalized hash, immutable fields and12 BFS routes.

Canonical backend allocation5 for0448 is recorded separately in producer commit
`ed34331cac41d1997335b5f6dc93f33429005a91`; allocation does not prove database
publication or release qualification. This new ef52 geometry needs a **distinct
canonical Tiled and server/client revision greater than5**. Revision6 is merely
proposed here; art does not allocate, reserve, publish or admit it. The actual
spatial producer and frontend consumer must coordinate explicit adoption and
relevant application/multiuser tests. Do not edit or reseal map4/899 or map5/0448.

Use matching `../quality-v10/manifest.json` and its explicitly rebound MAP label
presentation. No prior source review, resource qualification or runtime acceptance
carries forward. Artist changes no reserved runtime, backend or application path.
