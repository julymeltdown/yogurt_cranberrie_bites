// Metadata/decoded-handle unit fixtures only. These bytes are never registered as product art.
import { sha256 } from '../../../../world/map/json'
import { map4Parade, map4ThemeHashes } from '../../../../world/reference-scene/map4-service-v1/identity'
import { compositionVersion } from '../../../../world/reference-scene/map4-service-v2/semantics'
import { compositionDigest, createCompositionModel } from '../../../../world/reference-scene/map4-service-v2/model'
import type { CompositionRegistration } from '../../../../world/reference-scene/map4-service-v2/contract'

export async function compositionFixture() {
  const images = new Map<string, Uint8Array<ArrayBuffer>>()
  const assets = await Promise.all((Object.keys(map4ThemeHashes) as (keyof typeof map4ThemeHashes)[]).map(async (themeId, i) => {
    const bytes = new Uint8Array(33), view = new DataView(bytes.buffer)
    bytes.set([137, 80, 78, 71, 13, 10, 26, 10]); view.setUint32(8, 13)
    bytes.set([73, 72, 68, 82], 12); view.setUint32(16, 768); view.setUint32(20, 1296); bytes[30] = i
    const path = `world/map-reference-composition-v1/${themeId.toLowerCase()}-scene.png`; images.set(path, bytes)
    return { themeId, path, bytes: bytes.byteLength, sha256: await sha256(bytes), width: 768 as const, height: 1296 as const, mime: 'image/png' as const }
  }))
  const rows = [
    { key: 'main_stage', x: 410, y: 195, width: 312 }, { key: 'plaza', x: 329, y: 502, width: 143 },
    { key: 'wall', x: 160, y: 709, width: 221 }, { key: 'feed', x: 641, y: 692, width: 192 },
    { key: 'meet', x: 376, y: 933, width: 240 }, { key: 'gifts', x: 515, y: 1160, width: 240 },
  ]
  const document = {
    schemaVersion: 1, version: compositionVersion, coordinateSpace: 'MAP_ILLUSTRATION_ONLY', renderMode: 'full-composition',
    dimensions: { width: 768, height: 1296 }, originalMap4: {
      mapId: map4Parade.mapId, mapRevision: 4, mapContentHash: map4Parade.mapHash, geometryHash: map4Parade.geometryHash as string,
      sourceBinding: { path: 'test-only-public-source-binding.json', bytes: 1, sha256: 'sha256:' + 'a'.repeat(64) },
      themes: Object.entries(map4ThemeHashes).map(([themeId, themeContentHash]) => ({ themeId, themeRevision: 1, themeContentHash })),
    }, serviceLabelAnchors: rows.map(a => ({ ...a,
      exclusionRect: { x: a.x - a.width / 2 - 8, y: a.y - 56, width: a.width + 16, height: 112 } })),
    canonicalDisplayAnchors: [
      { interactionId: 'stage_info', x: 410, y: 403, worldCell: { x: 30, y: 24 }, authority: 'EXISTING_CANONICAL_IDENTITY_ONLY' },
      { interactionId: 'park_info', x: 646, y: 428, worldCell: { x: 38, y: 27 }, authority: 'EXISTING_CANONICAL_IDENTITY_ONLY' },
      { interactionId: 'gifts_info', x: 523, y: 1230, worldCell: { x: 13, y: 48 }, authority: 'EXISTING_CANONICAL_IDENTITY_ONLY' },
      { interactionId: 'wall_info', x: 143, y: 794, worldCell: { x: 14, y: 61 }, authority: 'EXISTING_CANONICAL_IDENTITY_ONLY' },
    ], assets,
  }
  const layoutBytes = new TextEncoder().encode(JSON.stringify(document))
  const registration = { version: compositionVersion,
    layout: { path: 'world/map-reference-composition-v1/layout.json', bytes: layoutBytes.byteLength, sha256: await sha256(layoutBytes) },
    assets, presentationHash: 'sha256:' + '0'.repeat(64) } satisfies CompositionRegistration
  registration.presentationHash = await compositionDigest(registration)
  return { document, registration, images, layoutBytes, model: createCompositionModel(document, registration) }
}
