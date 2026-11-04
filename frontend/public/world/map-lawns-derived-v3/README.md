# Garden interior overlay source

AR02/AR04-MAP-LAWNS-01 supplies original small planting motifs and irregular ground grain for the two frozen garden interiors. It is an illustration overlay with no World, collision, interaction, label, character or consumer authority.

The baseline is the ten pinned roof/street v2 files at `da33467cd084ecba071244a1939b6217f4b0de02`. `paint-contract.json` records Root153's exact mask. Eligible pixel centers lie at least 22 pixels inside a garden, strictly beyond the 23-pixel path clearance and 45-pixel canonical anchor discs, outside all 18 era label exclusion instances, and outside every nonzero alpha pixel of the 285 actual projected scenery layers and complete roof painters. Only the helper's four exact ground colors are eligible; actual helper ground must equal v2 ground. Every whole primitive must fit. Original art is never clipped or covered by an opaque background.

Each era has a transparent 768 by 1296 RGBA PNG, binary L8 allowed-mask PNG, editable SVG and a JSON array of ordered opaque integer rectangles. Alpha is 255 only for genuinely changed eligible pixels and zero RGBA elsewhere. SVG rectangles and JSON operations are exactly equivalent. SHA256 strings in this new interface are bare lowercase hex; inherited original input pins retain their historical `sha256:` prefix.

`interface.json` is consumable only with qualification status PASS and a pinned raw JSONL receipt whose final event is FULL/PASS with all children exited and temporary storage absent. Its qualification source commit equals the receipt ACCEPT head. The receipt pins the independent source qualification and source artifacts without pinning the final interface, avoiding a hash cycle. Source implementation and artifact commits are supplied separately in the handoff.

Artist231 is the sole final facade/lawn compositor. It consumes these qualified operations and overlays against frozen v2 and qualifies one combined facade-derived-v3 source pack. Map registration requires its own later explicit renewal. Full compositions and paired 360/390/430 source crops in the evidence folder are source assessments, not App or native Map screenshots.

Run the new builder, boundary probes, independent qualifier and previews only through the fresh finite admitted guard. No old builder main, old matrix, package installation or browser is required. Original crop and license provenance remains pinned in `source-provenance.json`.
