// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { createWorldEntryResolver } from '../../../app/world-entry/resolver'
import { authHarness } from '../../../app/world-entry/__tests__/admitted-auth'
import { assetBytes, bundle, inputConfiguration, requestFor, seal } from '../../../app/world-entry/__tests__/data'
import { sha256 } from '../../map/json'
import { sceneryResources, type Scenery } from '../../illustrated/model'
const owners: ReturnType<typeof authHarness>[] = []
afterEach(() => owners.splice(0).forEach(h => h.auth.dispose()))
async function fixture() {
  const b = await bundle(), hash = await sha256(assetBytes.png)
  for (const t of b.theme_manifests) {
    const texture = (id: string, width: number, height: number) => ({ id, path: `world/test-only/${t.theme_id.toLowerCase()}-${id}.png`, sha256: hash, width_px: width, height_px: height, bytes: assetBytes.png.length })
    Object.assign(t, { scenery: { version: 1, ground: texture('ground', 160, 160), atlases: [texture('details', 32, 32)],
      objects: [{ id: 'test', atlas_id: 'details', frame: { x: 0, y: 0, width: 32, height: 32 }, x: 0, y: 0, width: 32, height: 32, depth_y: 32 }] } })
  }
  return { b, scenery: (id: string) => (b.theme_manifests.find(t => t.theme_id === id)! as unknown as { scenery: Scenery }).scenery }
}
const setup = () => { const h = authHarness(); owners.push(h); return h }
it('admits original qualified selected/optional scenery descriptors and fences all reads after privacy withdrawal', async () => {
  const h = setup(), { b } = await fixture(), request = await requestFor(h.auth, seal(b)), calls: string[] = []
  const resolve = createWorldEntryResolver({ auth: h.auth, origin: 'https://mcr.test', localInput: inputConfiguration,
    assetFetch: async input => {
      calls.push(String(input)); const response = new Response(assetBytes.png, { headers: { 'Content-Type': 'image/png' } })
      Object.defineProperty(response, 'url', { value: String(input) }); return response
    } })
  const result = await resolve(request)
  for (const t of b.theme_manifests) for (const resource of sceneryResources(t as unknown as { scenery: Scenery })) {
    expect(Array.from(await result.resources.read(resource, request.signal))).toEqual(Array.from(assetBytes.png))
  }
  expect(calls).toHaveLength(6)
  const r = sceneryResources(b.theme_manifests[0]! as unknown as { scenery: Scenery })[0]!
  h.auth.privacy.changePolicy(request.privacyScope, 'block')
  await expect(result.resources.read(r, request.signal)).rejects.toThrow(); expect(calls).toHaveLength(6)
})
it('rejects invalid selected scenery semantics before room or asset fetch', async () => {
  const h = setup(), { b, scenery } = await fixture(); Object.assign(scenery('PARADE').ground, { width_px: 161 })
  const request = await requestFor(h.auth, seal(b))
  await expect(createWorldEntryResolver({ auth: h.auth, origin: 'https://mcr.test', localInput: inputConfiguration })(request)).rejects.toThrow()
  expect(h.requests).toHaveLength(0)
})
it.each(['selected-skin', 'candidate-tile'])('never partially admits optional ground or tile when its last atlas conflicts with %s', async kind => {
  const h = setup(), { b, scenery } = await fixture(), optional = b.theme_manifests.find(t => t.theme_id === 'REVENGE')!
  Object.assign(scenery('REVENGE').atlases[0]!, { path: kind === 'selected-skin' ? scenery('PARADE').ground.path : optional.texture.path, sha256: 'sha256:' + 'f'.repeat(64) })
  const request = await requestFor(h.auth, seal(b))
  let fetches = 0
  const result = await createWorldEntryResolver({ auth: h.auth, origin: 'https://mcr.test', localInput: inputConfiguration, assetFetch: async () => { fetches++; throw Error('Unexpected fetch') } })(request)
  expect(result.availableThemeIds).not.toContain('REVENGE'); expect(result.availableThemeIds).toContain('DANGER')
  for (const r of [{ path: optional.texture.path!, sha256: optional.texture.sha256!, kind: 'png' as const }, sceneryResources({ scenery: scenery('REVENGE') })[0]!]) {
    await expect(result.resources.read(r, request.signal)).rejects.toThrow()
  }
  expect(fetches).toBe(0)
})
