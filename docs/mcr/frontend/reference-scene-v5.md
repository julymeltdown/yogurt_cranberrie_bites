# V10 diagonal scene runtime adoption

Status: IN_PROGRESS. This is the separately assigned FE06/FE14/FE19, AR02/AR04 and bounded QA06 contribution. Assignment base is published `93ef0d895dcc4b247b727e2cccba0bb84576b262`; branch `codex/mcr-fe-reference-scene-v10`; exact ownership is recorded in World `55a93289fcd4a2fbc6b94298c3b5b21dd769dc21`. Old v8/map5 and all earlier runtimes and artist source bytes remain frozen.

The artist's published `c7b0cb013e5ab1d9f8cac08be1d0046c249e1cd4` and `06562ff1962be525ed929ef431a54bb589860373` provide spatial-v5/quality-v10 source geometry `sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395`. All 107 handoff paths and 105 closed pins were independently verified at publication. The runtime derivation verifies the immutable handoff again, 560 exact terrain changes, no collision/prop/spawn/place/interaction/camera changes, all three scenes and the newly rebound MAP label sidecar.

The finite Tiled file is 24,058 bytes: `sha256:e5837dfe08a8d2f1f7cdb4428b87965b4a7c6ecdb1458b8fe910e3f4c2112078`. Its three row-major 48x81 layers preserve terrain, collision and overdraw exactly. Public files live only under `world/reference-scene-v2/runtime/spatial-v5/`.

A minimum revision of 6 is a compatibility constraint, not a reservation. The initial source binding has `map_revision: null`, `PENDING_CANONICAL_BACKEND_ALLOCATION`, `NOT_RELEASE_BOUND` and `NOT_ADMITTED`. The actual spatial backend owner must supply a committed allocation greater than5; source-review and release admission remain separate. Fixtures deliberately use synthetic7005 and preserve original UNREVIEWED/CONCEPT_ONLY provenance. No source, server or physical qualification is invented.

Implementation and native qualification are continuing. No complete original-task, visual fidelity, live service, multiuser, mobile device or release acceptance is claimed by this source checkpoint.
