// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { createWorldRuntime, type EngineFactory, type WorldRuntime } from '../../runtime'
import { createEnginePreferenceEffects } from '../engine'
import type { LoadedMap } from '../../map/resources'
const runtimes: WorldRuntime[] = []
afterEach(async () => { for (const runtime of runtimes.splice(0)) { runtime.dispose(); await runtime.settled() }; vi.unstubAllGlobals(); document.body.replaceChildren() })
function setup() {
  vi.stubGlobal('requestAnimationFrame', () => 1); vi.stubGlobal('cancelAnimationFrame', vi.fn())
  const renderer = { preRender: vi.fn(), render: vi.fn(), postRender: vi.fn() }, original = { ...renderer }
  const tweens = { timeScale: 1, setGlobalTimeScale(value: number) { this.timeScale = value } }
  const effects = createEnginePreferenceEffects(renderer, tweens), host = document.createElement('div'); document.body.append(host)
  const map = { widthPx: 160, heightPx: 128, theme: { theme_id: 'REVENGE' }, spawn: { cell_x: 1, cell_y: 1 },
    map: { geometry: { tile_width_px: 32, tile_height_px: 32 } }, dispose: vi.fn() } as unknown as LoadedMap
  const factory: EngineFactory = vi.fn(() => { let close!: () => void; const closed = new Promise<void>(r => { close = r }); return {
    ready: Promise.resolve(), closed, effects, resize() {}, setVisible() {}, destroy() { effects.dispose(); close() } } })
  const runtime = createWorldRuntime({ host, load: async () => map, importEngine: async () => factory,
    createOverview: () => ({ replace() {}, read: () => null, subscribe: () => () => {}, dispose() {} }) })
  runtimes.push(runtime); return { runtime, effects, renderer, original, tweens, factory, map, host }
}
it('forwards a preference lease only after real runtime ownership is ready and restores it synchronously on stop', async () => {
  const f = setup(); expect(f.runtime.effects.motion).toBe(false)
  expect(() => f.runtime.effects.acquire({ reducedMotion: true, lowPower: true }, new AbortController().signal)).toThrow()
  await f.runtime.start(); expect(f.runtime.effects.motion).toBe(true)
  const lease = await f.runtime.effects.acquire({ reducedMotion: true, lowPower: true }, new AbortController().signal)
  expect(f.tweens.timeScale).toBe(0); expect(f.renderer.render).not.toBe(f.original.render)
  f.runtime.stop(); expect(f.runtime.effects.motion).toBe(false); expect(f.tweens.timeScale).toBe(1); expect(f.renderer).toEqual(f.original)
  lease.dispose(); await f.runtime.settled(); expect(f.factory).toHaveBeenCalledOnce(); expect(f.map.dispose).toHaveBeenCalledOnce(); expect(f.host.children).toHaveLength(0)
})
it('withdraws a settled preference lease immediately when the runtime privacy/start signal aborts', async () => {
  const f = setup(), lifetime = new AbortController(); await f.runtime.start(lifetime.signal)
  await f.runtime.effects.acquire({ reducedMotion: true, lowPower: true }, new AbortController().signal)
  lifetime.abort(); expect(f.effects.read()).toMatchObject({ disposed: true, lowPower: false, reducedMotion: false })
  expect(f.renderer).toEqual(f.original); expect(f.runtime.read().phase).toBe('stopped')
})
