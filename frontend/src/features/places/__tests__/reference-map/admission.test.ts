import { expect, it } from 'vitest'
import { decodeBootstrap } from '../../../../features/auth/model/bootstrap'
import { authHarness } from '../../../../app/world-entry/__tests__/admitted-auth'
import { bundle, seal } from '../../../../app/world-entry/__tests__/data'
import { referenceMapRelease } from './scene-data'

it('keeps the coherent PARADE-only proposal outside actual bootstrap admission', async () => {
  const { auth } = authHarness()
  try {
    const scope = auth.session.read(), baseline = await bundle()
    expect((await decodeBootstrap(baseline, { generation: scope.generation, userId: scope.userId!, eventId: scope.eventId! })).worldAvailable).toBe(true)
    const candidate = structuredClone(baseline), release = await referenceMapRelease(scope)
    candidate.map_manifest = release.manifest as typeof candidate.map_manifest
    candidate.theme_manifests = candidate.theme_manifests.filter(t => t.theme_id === 'PARADE')
    await seal(candidate)
    await expect(decodeBootstrap(candidate, { generation: scope.generation, userId: scope.userId!, eventId: scope.eventId! })).rejects.toThrow()
    expect(candidate.map_manifest.provenance.review_state).toBe('UNREVIEWED')
    expect(candidate.map_manifest.tilemap.resource_state).toBe('NOT_PROVIDED')
    expect(candidate.map_manifest.places.every(p => p.provenance.review_state === 'UNREVIEWED')).toBe(true)
  } finally { auth.dispose() }
})
