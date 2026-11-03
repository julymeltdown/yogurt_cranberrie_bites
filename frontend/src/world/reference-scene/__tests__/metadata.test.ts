import { expect, it } from 'vitest'
import proposal from '../../../../public/world/reference-scene-v2/geometry-proposal.json'
import { getReferenceOverviewMetadata, referenceSceneIdentity } from '..'

it('projects frozen source label offsets without creating additional interactions or physical destinations', () => {
  const metadata = getReferenceOverviewMetadata(referenceSceneIdentity)!
  expect(metadata.mapLabels).toEqual(proposal.map_labels.map(l => ({ label: l.label, x: l.x, y: l.y, interactionId: l.interaction_id })))
  expect(metadata.geometryHash).toBe(proposal.geometry_hash)
  const ids = new Set(proposal.geometry.interactions.map(i => i.interaction_id))
  for (const label of metadata.mapLabels) if (label.interactionId !== null) expect(ids.has(label.interactionId)).toBe(true)
  expect(metadata.mapLabels.filter(l => l.interactionId === null)).toHaveLength(4)
  expect(Object.isFrozen(metadata)).toBe(true); expect(Object.isFrozen(metadata.mapLabels)).toBe(true)
})
it.each(['geometryHash', 'width', 'height', 'themeId'] as const)('rejects mismatching %s before supplying illustration metadata', key => {
  const wrong = { ...referenceSceneIdentity, [key]: typeof referenceSceneIdentity[key] === 'number' ? 1 : 'unknown' }
  expect(getReferenceOverviewMetadata(wrong)).toBeNull()
})
