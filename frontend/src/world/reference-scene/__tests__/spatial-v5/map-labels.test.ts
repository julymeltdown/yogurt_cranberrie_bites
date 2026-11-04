// @vitest-environment jsdom
import { readFile } from 'node:fs/promises'
import { afterEach, expect, it, vi } from 'vitest'
import { createSpatialV5Labels } from '../../spatial-v5/map-labels'

const bytes = new Uint8Array(await readFile('public/world/reference-scene-v2/map-labels-v1/map-labels-bold.ttf'))
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks() })
function setup(raw = bytes) {
  const faces = new Set<unknown>(), load = vi.fn()
  class Face { load() { load(); return Promise.resolve(this) } }
  vi.stubGlobal('FontFace', Face)
  vi.stubGlobal('document', { fonts: faces })
  const fetcher = vi.fn(async (_url: URL, _init: RequestInit) => new Response(raw))
  vi.stubGlobal('fetch', fetcher)
  return { faces, fetcher, load, owner: createSpatialV5Labels() }
}
it('loads the pinned font once, with no credentials/redirects, and releases its registered face', async () => {
  const s = setup(); await Promise.all([s.owner.prepare(), s.owner.prepare()])
  expect(s.fetcher).toHaveBeenCalledOnce(); expect(s.load).toHaveBeenCalledOnce(); expect(s.faces.size).toBe(1)
  expect(s.fetcher.mock.calls[0]).toMatchObject([expect.any(URL), { credentials: 'omit', redirect: 'error', cache: 'no-store', referrerPolicy: 'no-referrer' }])
  s.owner.dispose(); s.owner.dispose(); expect(s.faces.size).toBe(0)
  await expect(s.owner.prepare()).rejects.toThrow('disposed')
})
it.each(['oversize', 'truncated', 'hash'] as const)('rejects %s bytes before FontFace or registration', async fault => {
  const raw = fault === 'oversize' ? new Uint8Array(bytes.length + 1) : fault === 'truncated' ? bytes.slice(1) : bytes.slice()
  if (fault === 'hash') raw[0] = raw[0]! ^ 255
  const s = setup(raw); await expect(s.owner.prepare()).rejects.toThrow()
  expect(s.load).not.toHaveBeenCalled(); expect(s.faces.size).toBe(0); s.owner.dispose()
})
it('fences an abort-resistant fetch after disposal, without registering a late face', async () => {
  const s = setup(); let release!: (r: Response) => void
  s.fetcher.mockImplementation(() => new Promise(r => { release = r }))
  const pending = s.owner.prepare(), rejected = expect(pending).rejects.toThrow('disposed')
  s.owner.dispose(); release(new Response(bytes)); await rejected
  expect(s.load).not.toHaveBeenCalled(); expect(s.faces.size).toBe(0)
})
it('fences a late native FontFace decode after disposal', async () => {
  const s = setup(); let release!: (f: unknown) => void
  class SlowFace { load() { return new Promise(r => { release = r }) } }
  vi.stubGlobal('FontFace', SlowFace)
  const pending = s.owner.prepare(), rejected = expect(pending).rejects.toThrow('disposed')
  await vi.waitFor(() => expect(release).toBeTypeOf('function'))
  s.owner.dispose(); release({}); await rejected; expect(s.faces.size).toBe(0)
})
