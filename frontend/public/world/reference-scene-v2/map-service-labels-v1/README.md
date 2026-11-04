# MCR service labels v1

Status: **CONTRACT_READY_METADATA_RENDER_ACTION_VALIDATION_NOT_RUN**.

This version implements the human product correction while preserving the
attached target's visual composition. The labels are MAIN STAGE, PLAZA, WALL,
FEED, MEET and GIFTS, in the same six-entry order and explicit panel rectangles.
WALL uses an original mural-grid glyph; FEED an original post/message glyph.
Stage, people and gift vectors, exact font bytes, palette, size and anchors stay
unchanged. Single-word MEET/GIFTS center vertically inside their original panels.
`source_label` records the previous display string; it is not a canonical ID.

All six annotation `interaction_id` and `place_id` values are null. MAIN STAGE
and PLAZA have `VIRTUAL_SCENERY_ONLY` authority. The four features have
`VIRTUAL_FEATURE_NAVIGATION_ONLY` authority and closed `navigation_intent`
values wall/feed/meet/gifts. The consumer maps those values to its actual shell
routes. Sources contain no arbitrary navigation URL. Original canonical
stage_info/gifts_info identities, genuine reviewed-place details and safety
remain separate. This annotation does not rename physical or canonical places.

`original-target.jpg` is the byte-exact supplied1280×853 JPEG,482,737B, SHA
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
It was actually viewed before authoring, is reference/provenance only and must
never be loaded as a product texture. The earlier1536×1024 reference is intact.

The final consumer freeze is label/glyph-only. New labels pin the old label,
landscape, architecture and two-wing source as immutable lineage. There are no
repinned landscape/wing derivatives or food-counter display replacements here.
Existing wing source9a441859/a0db1770 stays exact and needs explicit adoption;
the label input alone does not activate it. All original49 scene objects,
34 World objects, source PNGs, geometry, feet, cameras and character packs remain
untouched. Existing grocery/stall imagery remains a separate visual gap;
this patch does not claim complete semantic-art or target fidelity.

Reproduce source qualification without a browser/export:

```sh
python3 -B frontend/public/world/reference-scene-v2/map-service-labels-v1/build.py --verify
python3 -B frontend/public/world/reference-scene-v2/map-service-labels-v1/verify.py
```

Checks cover exact attachment/base/font/lineage bytes, all six retained anchors
and rectangles, four closed intents, null authority bindings, glyph coordinates,
text advance within retained paper width and closed handoff payload hashes.
They do not certify actual native typography,44px targets, navigation execution,
reviewed-place safety, visual density, mobile framing or full reference fidelity.
Those checks remain with Root's explicitly allocated consumer work. No new
browser/export/raster/font, second goal or manager approval gate is introduced.
The original ART goal remains unfinished with metadata BLOCKED.
