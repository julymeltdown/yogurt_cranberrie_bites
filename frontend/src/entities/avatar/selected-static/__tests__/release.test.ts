import { afterEach, describe, expect, it, vi } from 'vitest'
import { webcrypto } from 'node:crypto'
import { validateSelectedStaticRelease } from '../release'
import { characterSourcePack, characterArtSources } from '../../source-catalog'

function release(hash = 'a'.repeat(64)) {
  return { format: 'mcr-selected-static-v1', pose: 'front_idle_only', source_pack_id: characterSourcePack.source_pack_id,
    source_pack_revision: characterSourcePack.revision, reviewed_by: 'Deterministic test fixture', reviewed_at: '2026-11-04T00:00:00Z',
    atlas: { path: 'world/avatars/derived-v1/test-front-idle.png', sha256: `sha256:${hash}`, width_px: 384, height_px: 64 },
    characters: Array.from({ length: 6 }, (_, index) => ({ id: `photo01-front-0${index + 1}`, label: `Fixture ${index + 1}`,
      source_id: 'photo_01', source_sha256: characterArtSources.find(source => source.source_id === 'photo_01')!.sha256,
      frame: { x: index * 64, y: 0, width_px: 64, height_px: 64 }, anchor: { x: 32, y: 60 } })) }
}
afterEach(() => { vi.unstubAllGlobals(); vi.resetModules() })
describe('selected-source static illustration boundary', () => {
  it('rejects altered provenance, paths and fabricated directional poses', () => {
    for (const mutate of [
      (value: ReturnType<typeof release>) => { value.characters[0]!.source_sha256 = 'b'.repeat(64) },
      (value: ReturnType<typeof release>) => { value.atlas.path = 'world/avatars/derived-v1/../legacy.png' },
      (value: ReturnType<typeof release>) => { value.pose = 'walking' },
      (value: ReturnType<typeof release>) => { value.characters[1]!.frame.x = 0 },
    ]) { const value = release(); mutate(value); expect(() => validateSelectedStaticRelease(value)).toThrow() }
    expect(validateSelectedStaticRelease(release()).characters).toHaveLength(6)
  })
  it('loads one hash-checked atlas once across concurrent consumers and subsequent screen changes', async () => {
    const bytes = new Uint8Array(33), signature = [137, 80, 78, 71, 13, 10, 26, 10]
    bytes.set(signature); const view = new DataView(bytes.buffer)
    view.setUint32(8, 13); bytes.set([73, 72, 68, 82], 12); view.setUint32(16, 384); view.setUint32(20, 64)
    const digest = new Uint8Array(await webcrypto.subtle.digest('SHA-256', bytes))
    const hash = Array.from(digest, value => value.toString(16).padStart(2, '0')).join('')
    const fixture = release(hash), fetcher = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input), atlas = url.endsWith('.png')
      const response = new Response(atlas ? bytes : JSON.stringify(fixture), { headers: { 'Content-Type': atlas ? 'image/png' : 'application/json' } })
      Object.defineProperty(response, 'url', { value: atlas ? `http://localhost/${fixture.atlas.path}` : 'http://localhost/world/avatars/derived-v1/static-release.json' })
      return response
    })
    vi.stubGlobal('crypto', webcrypto); vi.stubGlobal('location', { origin: 'http://localhost' }); vi.stubGlobal('fetch', fetcher)
    const { loadSelectedStaticArtwork } = await import('../release')
    const [first, second] = await Promise.all([loadSelectedStaticArtwork(), loadSelectedStaticArtwork()])
    expect(first).toBe(second); expect(await loadSelectedStaticArtwork()).toBe(first)
    expect(fetcher).toHaveBeenCalledTimes(2)
    for (const [, options] of fetcher.mock.calls as unknown as [string, RequestInit][]) expect(options.credentials).toBe('omit')
  })
})
