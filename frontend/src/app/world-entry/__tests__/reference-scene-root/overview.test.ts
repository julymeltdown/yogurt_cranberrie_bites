// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { RuntimeOverview } from '../../../../world/runtime'
import type { SceneryOverview } from '../../../../world/illustrated/overview'
import type { LoadedMap } from '../../../../world/map/resources'
import { validateManifests } from '../../../../world/map/manifest'
import { referenceFixture } from '../../../../world/reference-scene/__tests__/fixture'
import { synthetic } from '../../../../world/__tests__/fixtures/synthetic'

const fakes = vi.hoisted(() => {
  const owners: { kind: string; owner: RuntimeOverview; emit(): void; set(value: SceneryOverview | null): void }[] = []
  return { owners, make(kind: string) {
    let snapshot: SceneryOverview | null = null
    const listeners = new Set<() => void>(), emit = () => Array.from(listeners).forEach(fn => fn())
    const owner: RuntimeOverview = { read: () => snapshot,
      subscribe(fn) { listeners.add(fn); return () => { listeners.delete(fn) } },
      replace: vi.fn((map: LoadedMap | null) => {
        snapshot = map ? { imageUrl: `blob:test-${kind}`, width: map.widthPx, height: map.heightPx,
          mapId: map.map.map_id, mapRevision: map.map.map_revision, mapHash: map.map.content_hash, geometryHash: map.map.geometry_hash,
          themeId: map.theme.theme_id, themeRevision: map.theme.theme_revision, themeHash: map.theme.content_hash } : null
        emit()
      }), dispose: vi.fn(() => { snapshot = null; emit(); listeners.clear() }),
    }
    owners.push({ kind, owner, emit, set(value) { snapshot = value } }); return owner
  } }
})
vi.mock('../../../../world/illustrated/overview', () => ({ createSceneryOverview: () => fakes.make('ordinary') }))
vi.mock('../../../../world/reference-scene', async original => ({ ...await original<typeof import('../../../../world/reference-scene')>(),
  createReferenceSceneOverview: () => fakes.make('reference') }))
import { createRootSceneOverview, isReferenceSceneCandidate, preflightReferenceScene } from '../../reference-scene-overview'

const cleanup: (() => void)[] = []
beforeEach(() => { fakes.owners.length = 0 })
afterEach(() => { cleanup.splice(0).reverse().forEach(stop => stop()); vi.restoreAllMocks() })
async function metadata() {
  const f = await referenceFixture()
  return validateManifests(JSON.stringify(f.map), JSON.stringify(f.theme), f.pin)
}
async function ordinary() {
  const f = await synthetic()
  return validateManifests(JSON.stringify(f.map), JSON.stringify(f.theme), f.pin)
}
const loaded = (value: Awaited<ReturnType<typeof metadata>>) => ({ ...value, image: {} as HTMLImageElement, dispose: vi.fn() } satisfies LoadedMap)

it('preflights original coherent v4 metadata without art review relabeling or resource work', async () => {
  const value = await metadata()
  expect(value.map.places.every(p => p.place_kind === 'VIRTUAL_CONCEPT' && p.verification_state === 'CONCEPT_ONLY' && p.provenance.review_state === 'UNREVIEWED')).toBe(true)
  expect(isReferenceSceneCandidate(value)).toBe(true); expect(() => preflightReferenceScene(value)).not.toThrow()
  const normal = await ordinary(); expect(isReferenceSceneCandidate(normal)).toBe(false); expect(() => preflightReferenceScene(normal)).not.toThrow()
})
it.each(['revision', 'geometry', 'theme', 'scenery', 'dimensions', 'spawn'] as const)('rejects reference %s incoherency before a producer is selected', async kind => {
  const value = structuredClone(await metadata())
  if (kind === 'revision') value.map.map_revision++
  if (kind === 'geometry') value.map.geometry.collision_grid[0]![0] = 0
  if (kind === 'theme') value.theme.theme_id = 'DANGER'
  if (kind === 'scenery') value.theme.scenery = { ...value.theme.scenery!, objects: value.theme.scenery!.objects.map((o, i) => i === 0 ? { ...o, x: o.x + 1 } : o) }
  if (kind === 'dimensions') value.widthPx++
  if (kind === 'spawn') value.spawn.cell_x++
  expect(() => preflightReferenceScene(value)).toThrow()
  const owner = createRootSceneOverview(); cleanup.push(owner.dispose); owner.replace(loaded(value))
  expect(owner.read()).toBeNull(); expect(fakes.owners.every(f => vi.mocked(f.owner.replace).mock.calls.length === 0)).toBe(true)
})
it('treats borrowed reference geometry or resource paths as claims even under another map ID', async () => {
  const value = structuredClone(await metadata()); value.map.map_id = 'foreign_map'
  expect(isReferenceSceneCandidate(value)).toBe(true); expect(() => preflightReferenceScene(value)).toThrow()
  const normal = structuredClone(await ordinary()); normal.theme.scenery = value.theme.scenery!
  expect(isReferenceSceneCandidate(normal)).toBe(true); expect(() => preflightReferenceScene(normal)).toThrow()
})
it('multiplexes only the current map owner, withdraws immediately and ignores inactive child emissions', async () => {
  const owner = createRootSceneOverview(); cleanup.push(owner.dispose)
  const normal = loaded(await ordinary()), reference = loaded(await metadata())
  owner.replace(normal); const old = owner.read()!; expect(old.imageUrl).toBe('blob:test-ordinary')
  const reads: (SceneryOverview | null)[] = [], stop = owner.subscribe(() => reads.push(owner.read()))
  owner.replace(reference)
  expect(reads[0]).toBeNull(); expect(owner.read()).toMatchObject({ imageUrl: 'blob:test-reference', mapId: reference.map.map_id, mapRevision: 2 })
  const count = reads.length; fakes.owners[0]!.set(old); fakes.owners[0]!.emit()
  expect(reads).toHaveLength(count); expect(owner.read()?.imageUrl).toBe('blob:test-reference')
  owner.replace(null); expect(owner.read()).toBeNull(); fakes.owners[1]!.emit(); expect(owner.read()).toBeNull()
  expect(normal.dispose).not.toHaveBeenCalled(); expect(reference.dispose).not.toHaveBeenCalled(); stop()
})
it('fences reentrant replacements, dispose notifications and late child events', async () => {
  const owner = createRootSceneOverview(); cleanup.push(owner.dispose)
  const normal = loaded(await ordinary()), reference = loaded(await metadata())
  owner.replace(normal)
  let switched = false
  owner.subscribe(() => { if (!owner.read() && !switched) { switched = true; owner.replace(reference) } })
  owner.replace(normal); expect(owner.read()?.imageUrl).toBe('blob:test-reference')
  owner.dispose(); owner.dispose(); owner.replace(normal)
  fakes.owners.forEach(f => { f.set({ imageUrl: 'blob:late' } as SceneryOverview); f.emit(); expect(f.owner.dispose).toHaveBeenCalledOnce() })
  expect(owner.read()).toBeNull()
})
