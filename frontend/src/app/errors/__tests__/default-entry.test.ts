// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { createDefaultConnectionStatus } from '../default-entry'
import { harness } from './harness'
import { deferred } from '../../../features/auth/__tests__/fixtures/data'

const stops: (() => void)[] = []
afterEach(() => { stops.splice(0).reverse().forEach(stop => stop()); vi.restoreAllMocks() })
function setup() {
  const h = harness(); h.controller.dispose(); stops.push(h.dispose)
  const reconnect = vi.fn(async () => true)
  h.world.set({ ...h.binding, reconnect })
  const online = vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(true)
  const add = vi.spyOn(window, 'addEventListener'), remove = vi.spyOn(window, 'removeEventListener')
  const entry = createDefaultConnectionStatus({ auth: h.auth, world: h.world, browser: window, reload: vi.fn(), scheduler: h.clock, pendingMs: 1000 })
  stops.push(entry.dispose); entry.start()
  return { ...h, entry, reconnect, online, add, remove }
}
it('owns one hint and release subscription across repeated start and world publications; releases everything', () => {
  const h = setup(); h.entry.start(); h.world.set(h.world.read())
  expect(h.entry.controller.read().kind).toBe('ready')
  expect(h.world.listeners.size).toBe(1); expect(h.view.listeners.size).toBe(1)
  expect(h.add.mock.calls.map(call => call[0])).toEqual(['online', 'offline'])
  h.patch({ phase: 'opening' }); expect(h.clock.pending).toBe(1)
  h.entry.dispose(); h.entry.dispose(); h.entry.start()
  expect(h.world.listeners.size).toBe(0); expect(h.view.listeners.size).toBe(0); expect(h.clock.pending).toBe(0)
  expect(h.remove.mock.calls.map(call => call[0])).toEqual(['online', 'offline'])
})
it('does not reconnect on browser hints, keeps stale after online, and clears it only on a fresh baseline', async () => {
  const h = setup()
  h.online.mockReturnValue(false); window.dispatchEvent(new Event('offline'))
  expect(h.entry.controller.read()).toMatchObject({ kind: 'offline', stale: true })
  h.online.mockReturnValue(true); window.dispatchEvent(new Event('online'))
  expect(h.entry.controller.read()).toMatchObject({ kind: 'resync', stale: true })
  expect(h.reconnect).not.toHaveBeenCalled()
  h.patch({ baseline: { ...h.view.read()!.baseline!, streamId: 'fresh-default-entry' } })
  expect(h.entry.controller.read()).toMatchObject({ kind: 'ready', stale: false })
  h.patch({ phase: 'closed' }); expect(h.entry.controller.read().canRetry).toBe(true)
  expect(await h.entry.controller.retry()).toBe(true); expect(h.reconnect).toHaveBeenCalledOnce()
})
it('fences a delayed reconnect from an old exact release and withdraws on privacy change', async () => {
  const h = setup(), held = deferred<boolean>()
  h.reconnect.mockReturnValue(held.promise); h.patch({ phase: 'closed' })
  const result = h.entry.controller.retry()
  const replacement = { ...h.binding, release: { ...h.binding.release }, reconnect: vi.fn(async () => false) }
  h.world.set(replacement); held.resolve(true)
  expect(await result).toBe(false); expect(replacement.reconnect).not.toHaveBeenCalled()
  h.auth.privacy.changePolicy(h.binding.release.privacyScope, 'block')
  expect(h.entry.controller.read()).toEqual({ kind: 'unavailable', stale: false, pending: false, canRetry: false })
  expect(h.view.listeners.size).toBe(0)
})
it('handles absent world/recovery without inventing operational policy or retry', async () => {
  const h = setup()
  h.world.set(h.binding); h.patch({ phase: 'closed' })
  expect(h.entry.controller.read()).toMatchObject({ kind: 'unavailable', canRetry: false })
  expect(await h.entry.controller.retry()).toBe(false)
  const absent = createDefaultConnectionStatus({ auth: h.auth, browser: window, reload() {} })
  stops.push(absent.dispose); absent.start()
  expect(absent.controller.read()).toEqual({ kind: 'unavailable', stale: false, pending: false, canRetry: false })
})
it('bounds default-entry pending time without retrying or claiming service maintenance', () => {
  const h = setup(); h.patch({ phase: 'opening' })
  expect(h.entry.controller.read()).toMatchObject({ kind: 'reconnecting', pending: true })
  h.clock.advance(1000)
  expect(h.entry.controller.read()).toMatchObject({ kind: 'unavailable', pending: false, canRetry: false })
  expect(h.reconnect).not.toHaveBeenCalled()
})
