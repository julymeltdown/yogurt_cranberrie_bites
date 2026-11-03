# Illustrated Map integration — 2026-11-04

The default Places overview now consumes a bounded static projection of the
incumbent World's validated scenery. It reuses current authorized places and
camera controls, excludes characters/private entities, and retains the schematic
fallback. No second Phaser instance or concurrent world connection is created.

Author `80d91501694ede5ddc6ad28ac37010309c035878`, based on `1163b827`, is
delivered as `e506c102` over integration base `ef9422c4`. All 48 paths match the
assignment; seven existing-file preimages, 83 unchanged inputs and 47 delivery
hashes match. All integrated file bytes match the author. The author reported
the actual goal complete and a clean stopped checkout. Main used that qualified
handoff without an additional general review.

Fresh combined strict TypeScript and 764 tests across 31 affected files passed
(exit 0; tests 7.24 seconds). Whitespace checks passed. The initial unquoted
`HEAD^` shell argument was rejected by zsh; the corrected `HEAD~1` command passed.
The temporary dependency link was removed. No dependencies were installed.

The author's default-app browser fixture passed at 360/390/430px with 24 captures
and nine exact RGBA comparisons against independent source-scenery compositions.
It exercises era changes, fallback, keyboard/camera/Back focus, account changes,
late encoding and complete URL disposal. Main inspected its 390px capture.
The fixture uses deterministic HTTP/WebSocket peers and synthetic avatar resources;
these checks do not certify live services, approved characters, physical devices
or similarity to the user's full ten-screen collage. Orthogonal geography and
numbered markers remain visible differences. Status remains **MOCK_READY**.

See the [author handoff](illustrated-map-overview.md),
[integration receipt](../evidence/illustrated-map-integration-2026-11-04.json),
and [reference audit](../design/ui-ux-reference-review.md).
