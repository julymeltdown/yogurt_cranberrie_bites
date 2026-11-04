import { expect, it } from 'vitest'
import { sha256 } from '../../../../world/map/json'
import { map4Labels, presentationHash, presentationVersion, getMap4ServiceAnnotations } from '../../../../world/reference-scene/map4-service-v1/annotations'
import style from '../../../../world/reference-scene/map4-service-v1/style-source.json'
import { map4PaintSource } from '../../../../world/reference-scene/map4-service-v1/paint-source'
import { map4Parade, map4ThemeHashes } from '../../map-presentation-v1/identity'
import { loadedMap4 } from './fixture'
import originalMap4 from '../../../../world/reference-scene/spatial-v3/source-inputs.json'

it('pins separately authored Map4 annotations/style/ground as one presentation, with no canonical authority', async () => {
  expect(await sha256(new TextEncoder().encode(JSON.stringify({ labels: map4Labels, style, ground: map4PaintSource })))).toBe(presentationHash)
  expect(map4Labels).toHaveLength(6)
  expect(map4Labels.map(l => ({ x: l.x, y: l.y }))).toEqual(originalMap4.map_labels.map(l => ({ x: l.x, y: l.y })))
  for (const l of map4Labels) {
    expect(l.placeId).toBeNull(); expect(l.interactionId).toBeNull()
    expect(l.rect.x).toBeGreaterThanOrEqual(5); expect(l.rect.x + l.rect.width).toBeLessThanOrEqual(763)
    expect(l.rect.y).toBeGreaterThanOrEqual(5); expect(l.rect.y + l.rect.height).toBeLessThanOrEqual(1291)
  }
})
it.each(['REVENGE', 'PARADE', 'DANGER'] as const)('exposes only four virtual routes for a published exact %s presentation', async theme => {
  const map = await loadedMap4()
  const identity = { ...map4Parade, themeId: theme, themeHash: map4ThemeHashes[theme],
    presentationVersion, presentationHash }
  expect(getMap4ServiceAnnotations(identity).map(a => a.path).sort()).toEqual(['/feed', '/gifts', '/meet', '/wall'])
  expect(getMap4ServiceAnnotations({ ...identity, presentationHash: '' })).toEqual([])
  expect(getMap4ServiceAnnotations({ ...identity, mapRevision: 7005 })).toEqual([])
  expect(getMap4ServiceAnnotations({ ...identity, mapHash: 'sha256:' + '0'.repeat(64) })).toEqual([])
  expect(getMap4ServiceAnnotations({ ...identity, themeHash: map.theme.content_hash + 'foreign' })).toEqual([])
})
