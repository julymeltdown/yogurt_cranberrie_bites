# Published source-qualification input data

These verbatim public release JSON records come from backend source publication `0a8c422645458c9397af9289e15089ee3f9cb0ad`. They are fixture input data, not imported backend implementation or Product authorization.

| Retained file | Original release path | SHA-256 |
| --- | --- | --- |
| scene-source-qualification.json | backend/releases/reference-scene-v2/quality-v6/scene-source-qualification.json | 70c3d1ac268c6e5da39d30a20e01406fc7eb27668d6f0e6a66e1d87de15d7d07 |
| source-binding.json | backend/releases/reference-scene-v2/quality-v6/source-binding.json | 0f15f5614d5502442a4e58cc995041f7ee1f2f906e21d4b630db7b13af65ee6b |

The producer actually reviewed exact per-era scenery and texture inputs and supplied source-level C06 provenance; its packet explicitly does not provide or qualify an arbitrary ThemeManifest, avatar or bootstrap. The root fixture separately derives NEW descriptors, validates their complete C06/hash/map/avatar bindings and exact World source metadata, and tests rejection of changed bindings/resources/scenery. Original World fixtures, artist provenance and the four virtual place records stay unchanged. The source fixture's map content hash is deliberately not the backend canonical candidate hash.

This is engineering source/manifest qualification for a deterministic client fixture. Product/operator admission, physical-place authorization, fidelity/user acceptance and live multiplayer qualification are not supplied. Production guards are unchanged. See the root delivery document and `qualified-themes.test.ts` for the packaging checks and native receipts for actual source consumption.
