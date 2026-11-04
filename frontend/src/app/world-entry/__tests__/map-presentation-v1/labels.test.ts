// @vitest-environment jsdom
import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { afterEach, expect, it, vi } from 'vitest'
import { prepareMap4Labels } from '../../../../world/reference-scene/map4-service-v1/labels'

const originalFonts = Object.getOwnPropertyDescriptor(document, 'fonts')
afterEach(() => {
  vi.unstubAllGlobals()
  if (originalFonts) Object.defineProperty(document, 'fonts', originalFonts)
  else Reflect.deleteProperty(document, 'fonts')
})
async function setup() {
  const bytes = new Uint8Array(await readFile(resolve(process.cwd(), 'public/world/reference-scene-v2/map-labels-v1/map-labels-bold.ttf')))
  const fonts = new Set<FontFace>(), add = vi.fn((f: FontFace) => { fonts.add(f) }), remove = vi.fn((f: FontFace) => fonts.delete(f))
  Object.defineProperty(document, 'fonts', { configurable: true, value: { add, delete: remove, has: (f: FontFace) => fonts.has(f) } })
  const fetch = vi.fn(async () => {
    const response = new Response(bytes)
    Object.defineProperty(response, 'url', { value: new URL('/world/reference-scene-v2/map-labels-v1/map-labels-bold.ttf', location.origin).href }); return response
  })
  vi.stubGlobal('fetch', fetch)
  vi.stubGlobal('FontFace', class { async load() { return this } })
  return { bytes, fonts, add, remove, fetch }
}
it('checks the real immutable font bytes and releases the independent Map-only font lease', async () => {
  const s = await setup(), owner = await prepareMap4Labels(new AbortController().signal)
  expect(s.add).toHaveBeenCalledOnce(); expect(s.fonts.size).toBe(1)
  owner.dispose(); owner.dispose(); expect(s.remove).toHaveBeenCalledOnce(); expect(s.fonts.size).toBe(0)
})
it('rejects font tamper before FontFace installation', async () => {
  const s = await setup(); s.bytes[32] = s.bytes[32]! ^ 1
  await expect(prepareMap4Labels(new AbortController().signal)).rejects.toThrow()
  expect(s.add).not.toHaveBeenCalled()
})
it('does not install an abort-resistant late FontFace after privacy cancellation', async () => {
  const s = await setup(), abort = new AbortController()
  let finish!: (value: FontFace) => void
  const load = vi.fn(() => new Promise<FontFace>(resolve => { finish = resolve }))
  vi.stubGlobal('FontFace', class { load = load })
  const pending = prepareMap4Labels(abort.signal), rejected = expect(pending).rejects.toThrow()
  await vi.waitFor(() => expect(load).toHaveBeenCalledOnce()); abort.abort(); await rejected
  finish({} as FontFace); await Promise.resolve(); await Promise.resolve(); expect(s.add).not.toHaveBeenCalled()
})
