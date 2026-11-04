# Legacy map label reachability

Source inspection at publisher `101019a1` distinguishes supported historical
identity from rendered product text. The exact legacy r3 identity remains
recognized by `world/reference-scene/metadata.ts` and the validated scene
binding in `world/reference-scene/model.ts`. It is incorrect to declare that
identity unreachable merely because current native captures use v5.

The legacy metadata retains `FOOD` and `MERCH` as historical labels.
`features/places/ui/IllustratedMap.tsx` consumes only each matching metadata
entry's interaction ID and coordinates for catalog plaque placement. Plaque
text, accessible names, titles and selection text come from the authorized
catalog's `place.label`. `FOOD` has a null interaction and creates no destination.
`MapServiceLinks` obtains virtual service annotations only through the v5
identity helper; it does not invent legacy service destinations.

The existing legacy-r3 view fixture exercises the illustrated map with its
synthetic, unreviewed manifest. That fixture is suitable for DOM reachability
tests, and is not production admission. The additional regression in
`features/places/__tests__/reference-map/view.test.tsx` first confirms that the
historical helper still contains both strings, then checks actual rendered
text and every place link's accessible name, title and href. Existing tests
retain the canonical catalog destinations and image-failure fallback checks.

There is no confirmed metadata-string UI defect requiring a production patch.
Historical source strings, geometry, art and character sources are preserved.
Old atlas booth/food-counter pixels are a separate raster-art question: DOM
assertions and current-v5 captures cannot establish that historical r3 artwork
contains no lettering. No native legacy-art acceptance is claimed.

Validation: regression source committed as `101019a1`; the allocated
`ROOT-FOCUS-VIS-202611040902Z` run passed all 15 tests across the three affected
files, full TypeScript, scoped lint and runner syntax. Exact commands and logs
are retained in `../evidence/cozy-backgrounds-v1/focus-04/resource/`. The native
packet's separate forced-title sampling failures do not change this source
result or establish anything about historical r3 raster lettering.
