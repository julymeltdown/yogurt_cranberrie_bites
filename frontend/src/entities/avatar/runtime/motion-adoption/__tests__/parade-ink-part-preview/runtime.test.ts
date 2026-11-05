// @vitest-environment jsdom
import { afterEach, beforeAll, expect, it, vi } from 'vitest'
import type { MotionArt } from '../../art'
import type { MotionPart } from '../../runtime'
import { testMotionAuthority } from '../../../../../../world/avatar/__tests__/motion-adoption/test-authority'
import { artwork, fixture, part, signal } from './fixture'

let art: MotionArt
const stops: (() => void)[] = []
beforeAll(async () => { art = await artwork() })
afterEach(() => { stops.splice(0).forEach(fn => fn()); vi.restoreAllMocks() })
function setup() { const f = fixture(art); stops.push(f.runtime.dispose); return f }

it('fails closed before an owner flight and never initiates a decode', async () => {
  const f = setup()
  await expect(f.runtime.acquirePartPreview(part, { signal: signal() })).rejects.toThrow()
  expect(f.load).not.toHaveBeenCalled(); expect(f.canvases).toHaveLength(0)
})
it('draws exactly one verified down idle cell for every admitted kind/key without composing an outfit', async () => {
  const f = setup(); const catalog = await f.ready()
  expect(catalog.items).toHaveLength(28)
  for (const item of catalog.items) {
    f.drawImage.mockClear()
    const lease = await f.runtime.acquirePartPreview({ kind: item.kind, key: item.key }, { signal: signal() })
    const snapshot = lease.read()!, index = item.directions.find(direction => direction.direction === 0)!.frames[0]!
    expect(snapshot).toMatchObject({ kind: item.kind, key: item.key, representation: 'isolated-part', direction: 'down', phase: 0, composition: 'raw-item-cell' })
    expect([snapshot.canvas.width, snapshot.canvas.height]).toEqual([64, 64])
    expect(f.drawImage).toHaveBeenCalledExactlyOnceWith(f.image, index % 20 * 64, Math.floor(index / 20) * 64, 64, 64, 0, 0, 64, 64)
    lease.dispose(); lease.dispose(); expect(lease.read()).toBeNull(); expect(snapshot.canvas.width).toBe(0)
  }
  expect(f.load).toHaveBeenCalledOnce()
  expect(f.runtime.diagnostics().composition).toMatchObject({ entries: 0, created: 0, partPreviews: 0 })
  expect(f.runtime.diagnostics().clockListeners).toBe(0)
})
it.each([{ kind: 'hair', key: 'eyes-0' }, { kind: 'eyes', key: 'hair-0' }, { kind: 'accessory', key: 'none' },
  { kind: 'outfit', key: '' }, { kind: 'unknown', key: 'hair-0' }, { kind: 'body', key: 'body-8' },
  { kind: 'hair', key: 'hair-0', authority: true }])('rejects invalid pair or shape %j', async input => {
  const f = setup(); await f.ready()
  await expect(f.runtime.acquirePartPreview(input as MotionPart, { signal: signal() })).rejects.toThrow()
  expect(f.canvases).toHaveLength(0); expect(f.runtime.diagnostics().partPreviews).toBe(0)
})
it('does not treat verified art membership as permission from a different authority', async () => {
  const base = testMotionAuthority(art)
  const f = fixture(art, { ...base, replace() { throw Error('Not admitted by this authority') } }); stops.push(f.runtime.dispose)
  await f.ready(); await expect(f.runtime.acquirePartPreview(part, { signal: signal() })).rejects.toThrow()
  expect(f.canvases).toHaveLength(0)
})
it('bounds active previews to four and returns capacity immediately after idempotent dispose', async () => {
  const f = setup(); await f.ready()
  const leases = await Promise.all(Array.from({ length: 4 }, () => f.runtime.acquirePartPreview(part, { signal: signal() })))
  await expect(f.runtime.acquirePartPreview(part, { signal: signal() })).rejects.toThrow()
  expect(f.canvases.filter(canvas => canvas.width)).toHaveLength(4)
  leases[0]!.dispose(); leases[0]!.dispose()
  const replacement = await f.runtime.acquirePartPreview(part, { signal: signal() })
  expect(f.canvases.filter(canvas => canvas.width)).toHaveLength(4)
  replacement.dispose(); leases.forEach(lease => lease.dispose())
})
it('does not add a timer or a scene clock when borrowing a ready owner', async () => {
  const f = setup(); await f.ready(); const timer = vi.spyOn(globalThis, 'setTimeout')
  const lease = await f.runtime.acquirePartPreview(part, { signal: signal() })
  expect(timer).not.toHaveBeenCalled(); expect(f.runtime.diagnostics().clockListeners).toBe(0); lease.dispose()
})
it('reserves four slots before a shared asynchronous decode and frees an aborted reservation', async () => {
  const f = setup(); let finish!: (value: Awaited<ReturnType<typeof f.load>>) => void
  f.load.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
  const ready = f.ready(), controllers = Array.from({ length: 4 }, () => new AbortController())
  const requests = controllers.map(controller => f.runtime.acquirePartPreview(part, { signal: controller.signal }).catch(() => null))
  expect(f.runtime.diagnostics().partPreviews).toBe(4)
  await expect(f.runtime.acquirePartPreview(part, { signal: signal() })).rejects.toThrow()
  controllers[0]!.abort(); expect(f.runtime.diagnostics().partPreviews).toBe(3)
  const replacement = f.runtime.acquirePartPreview(part, { signal: signal() })
  finish({ art, image: f.image, dispose: f.imageDispose }); await ready
  const leases = await Promise.all(requests); await replacement
  expect(leases[0]).toBeNull(); expect(f.canvases).toHaveLength(4); expect(f.load).toHaveBeenCalledOnce()
})
it('rejects an already aborted request without drawing or reserving', async () => {
  const f = setup(); await f.ready(); const controller = new AbortController(); controller.abort()
  await expect(f.runtime.acquirePartPreview(part, { signal: controller.signal })).rejects.toThrow()
  expect(f.canvases).toHaveLength(0); expect(f.runtime.diagnostics().partPreviews).toBe(0)
})
it('releases a canvas if the request is aborted during its synchronous draw', async () => {
  const f = setup(); await f.ready(); const controller = new AbortController()
  f.drawImage.mockImplementationOnce(() => controller.abort())
  await expect(f.runtime.acquirePartPreview(part, { signal: controller.signal })).rejects.toThrow()
  expect(f.canvases.every(canvas => canvas.width === 0)).toBe(true)
  expect(f.runtime.diagnostics()).toMatchObject({ partPreviews: 0, composition: { partPreviews: 0 } })
})
it('clears all private canvases and reads before the first withdrawal notification', async () => {
  const f = setup(); await f.ready()
  const leases = await Promise.all(Array.from({ length: 4 }, () => f.runtime.acquirePartPreview(part, { signal: signal() })))
  const canvases = leases.map(lease => lease.read()!.canvas), notified = vi.fn(() => {
    expect(leases.every(lease => lease.read() === null)).toBe(true)
    expect(canvases.every(canvas => canvas.width === 0 && canvas.height === 0)).toBe(true)
  })
  leases.forEach(lease => lease.subscribe(notified)); f.revoke()
  expect(notified).toHaveBeenCalledTimes(4); expect(f.imageDispose).toHaveBeenCalledOnce()
  expect(notified.mock.results.every(result => result.type === 'return')).toBe(true)
  leases.forEach(lease => lease.dispose()); expect(notified).toHaveBeenCalledTimes(4)
  await expect(f.runtime.acquirePartPreview(part, { signal: signal() })).rejects.toThrow()
})
it('releases a live preview on abort and does not deliver removed listeners', async () => {
  const f = setup(); await f.ready(); const controller = new AbortController()
  const lease = await f.runtime.acquirePartPreview(part, { signal: controller.signal }), canvas = lease.read()!.canvas
  const removed = vi.fn(), notify = vi.fn(); lease.subscribe(removed)(); lease.subscribe(notify)
  controller.abort(); expect(lease.read()).toBeNull(); expect(canvas.width).toBe(0)
  expect(removed).not.toHaveBeenCalled(); expect(notify).toHaveBeenCalledOnce(); expect(f.runtime.isCurrent()).toBe(true)
})
it('notifies once when a withdrawal listener reenters dispose and isolates a throwing listener', async () => {
  const f = setup(); await f.ready()
  const lease = await f.runtime.acquirePartPreview(part, { signal: signal() }), canvas = lease.read()!.canvas
  const reentrant = vi.fn(() => { lease.dispose(); throw Error('Observer failed') }), other = vi.fn()
  lease.subscribe(reentrant); lease.subscribe(other); lease.dispose(); lease.dispose()
  expect(reentrant).toHaveBeenCalledOnce(); expect(other).toHaveBeenCalledOnce()
  expect(lease.read()).toBeNull(); expect(canvas.width).toBe(0); expect(f.runtime.diagnostics().partPreviews).toBe(0)
})
it('prevents a late draw after owner withdrawal during an abort-resistant atlas decode', async () => {
  const f = setup(); let finish!: (value: Awaited<ReturnType<typeof f.load>>) => void
  f.load.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
  const ready = f.ready().catch(() => null), pending = f.runtime.acquirePartPreview(part, { signal: signal() }).catch(() => null)
  f.revoke(); finish({ art, image: f.image, dispose: f.imageDispose })
  expect(await ready).toBeNull(); expect(await pending).toBeNull()
  expect(f.canvases).toHaveLength(0); expect(f.imageDispose).toHaveBeenCalledOnce(); expect(f.runtime.diagnostics().partPreviews).toBe(0)
})
