# Photo 2 PARADE source art

Original editable vector scenery for the qualified Photo 2 display contract at canonical commit `33491ccf9b0ae3835ad691072d8479b299f543d8`. `ground.svg` and `atlas-editable.json` are editable sources; the owned `author.py` deterministically rasterizes their original primitives using the existing Pillow installation. No reference, avatar, or character pixels are sampled into these assets.

The ground is 499 × 1200 display pixels with viewBox `-112 -20 499 1200`. The transparent atlas is 512 × 1024 pixels, with 22 native-foot billboard frames and two nonphysical MEET/WALL panel shortcut frames. The frozen native geometry, coordinates, IDs, collision, rooms and existing C06 native tiled texture remain separate inputs. Blocked native cells are clipped against the contract triangles and painted through the same projection, including support beyond decorative sprite alpha.

`asset-interface.json` records exact new image hashes, transfer bytes, frames, native objects and the shared display mesh. The pack contains 1,123,088 source pixels and 4,492,352 decoded RGBA bytes. These units differ from the 2,097,152-byte PNG transfer ceiling and the separate 2,097,152-pixel ceiling.

This source pack requires a coherent new PARADE revision/content hash/catalog binding before activation. It does not replace old hashes or admit a WORLD release. REVENGE/DANGER and supplied originals remain unchanged. Renderer interaction, projection/inverse, occlusion, accessibility, native-device and complete Photo 2 acceptance remain consumer-owned work.

The existing `/meet` and `/wall` shortcuts have no physical interaction ID, proximity, arrival, attendance or room authority. Consumers must retain their actual authorization, disabled/pending, keyboard focus and minimum 44 CSS-pixel target rules.

Owned source diagnostics and qualifier results are under `docs/mcr/art/evidence/photo2-world-display-v1/`. Any reference comparison there is evidence only; its original reference crop is not part of the delivered atlas or ground.
