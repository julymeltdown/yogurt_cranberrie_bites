// @vitest-environment jsdom
import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { afterEach, expect, it, vi } from 'vitest'
import { compositionFixture } from './fixture'
import { prepareCompositionLabels } from '../../../../world/reference-scene/map4-service-v2/labels'

const originalFonts = Object.getOwnPropertyDescriptor(document, 'fonts')
afterEach(() => {
  vi.unstubAllGlobals()
  if (originalFonts) Object.defineProperty(document, 'fonts', originalFonts)
  else Reflect.deleteProperty(document, 'fonts')
})
async function setup() {
  const f = await compositionFixture(), bytes = new Uint8Array(await readFile(resolve(process.cwd(), 'public/world/reference-scene-v2/map-labels-v1/map-labels-bold.ttf')))
  const fonts = new Set<FontFace>(), add = vi.fn((face: FontFace) => fonts.add(face)), remove = vi.fn((face: FontFace) => fonts.delete(face))
  Object.defineProperty(document, 'fonts', { configurable: true, value: { add, delete: remove, has: (face: FontFace) => fonts.has(face) } })
  const fetch = vi.fn(async () => {
    const response = new Response(bytes)
    Object.defineProperty(response, 'url', { value: location.origin + '/world/reference-scene-v2/map-labels-v1/map-labels-bold.ttf' }); return response
  })
  vi.stubGlobal('FontFace', class { async load() { return this } })
  return { ...f, bytes, fonts, add, remove, fetch }
}
it('leases the immutable original font independently and draws six revised labels with no food/merch semantics', async () => {
  const s = await setup(), owner = await prepareCompositionLabels(s.model, new AbortController().signal, { origin: location.origin, fetch: s.fetch })
  const context = { save: vi.fn(), restore: vi.fn(), beginPath: vi.fn(), lineTo: vi.fn(), moveTo: vi.fn(), closePath: vi.fn(),
    fill: vi.fn(), fillRect: vi.fn(), fillText: vi.fn(), measureText: vi.fn(() => ({ width: 50 })) } as unknown as CanvasRenderingContext2D
  owner.draw(context)
  expect(context.fillText).toHaveBeenCalledTimes(6)
  expect(vi.mocked(context.fillText).mock.calls.map(c => c[0])).toEqual(['MAIN STAGE', 'PLAZA', 'WALL', 'FEED', 'MEET', 'GIFTS'])
  owner.dispose(); owner.dispose(); expect(s.remove).toHaveBeenCalledOnce(); expect(s.fonts.size).toBe(0)
})
it('rejects corrupt font bytes before installing a font lease', async () => {
  const s = await setup(); s.bytes[23] = s.bytes[23]! ^ 1
  await expect(prepareCompositionLabels(s.model, new AbortController().signal, { origin: location.origin, fetch: s.fetch })).rejects.toThrow()
  expect(s.add).not.toHaveBeenCalled()
})
it('does not install an abort-resistant late font after cancellation', async () => {
  const s = await setup(), abort = new AbortController()
  let finish!: (face: FontFace) => void
  const load = vi.fn(() => new Promise<FontFace>(resolve => { finish = resolve }))
  vi.stubGlobal('FontFace', class { load = load })
  const pending = prepareCompositionLabels(s.model, abort.signal, { origin: location.origin, fetch: s.fetch }), rejected = expect(pending).rejects.toThrow()
  await vi.waitFor(() => expect(load).toHaveBeenCalledOnce()); abort.abort(); await rejected
  finish({} as FontFace); await Promise.resolve(); await Promise.resolve(); expect(s.add).not.toHaveBeenCalled()
})
it('withdraws label drawing rather than publishing cropped text when actual font metrics exceed authored width', async () => {
  const s = await setup(), owner = await prepareCompositionLabels(s.model, new AbortController().signal, { origin: location.origin, fetch: s.fetch })
  const context = { save: vi.fn(), restore: vi.fn(), measureText: vi.fn(() => ({ width: 999 })) } as unknown as CanvasRenderingContext2D
  expect(() => owner.draw(context)).toThrow(); owner.dispose(); expect(s.fonts.size).toBe(0)
})
