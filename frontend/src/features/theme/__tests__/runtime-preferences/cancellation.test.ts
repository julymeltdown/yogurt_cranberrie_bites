import { expect, it, vi } from 'vitest'
import { createThemeController } from '../../model/controller'
import type { ThemeId } from '../../model/preferences'
import type { LoadedMap } from '../../../../world/map/resources'
const deferred = <T,>() => { let resolve!: (v: T) => void; const promise = new Promise<T>(r => { resolve = r }); return { promise, resolve } }
function setup() {
  let skin: ThemeId = 'REVENGE'
  const pending = deferred<LoadedMap>(), write = vi.fn(), dispose = vi.fn()
  const map = (theme: ThemeId) => ({ theme: { theme_id: theme }, dispose }) as unknown as LoadedMap
  const load = vi.fn(async (theme: ThemeId) => theme === 'PARADE' ? pending.promise : map(theme))
  const controller = createThemeController({ initial: skin, scope: { userId: 'user-a', eventId: 'event-a' }, preferences: { read: () => null, write }, load,
    runtime: { readSkin: () => skin, replaceSkin: async (read, signal) => { const next = await read(signal!); if (!signal?.aborted) skin = next.theme.theme_id as ThemeId } } })
  return { controller, pending, write, dispose, map, skin: () => skin }
}
it('cancels an in-flight load, immediately reports actual skin and disposes ignored-abort resources', async () => {
  const f = setup(), caller = new AbortController(), task = f.controller.select('PARADE', caller.signal)
  expect(f.controller.read()).toMatchObject({ phase: 'loading', pending: 'PARADE' }); caller.abort()
  expect(f.controller.read()).toMatchObject({ phase: 'ready', pending: null, selected: 'REVENGE' })
  f.pending.resolve(f.map('PARADE')); await task
  expect(f.skin()).toBe('REVENGE'); expect(f.dispose).toHaveBeenCalledOnce(); expect(f.write).not.toHaveBeenCalled()
})
it('does not overwrite a newer selector when an older caller aborts or resolves late', async () => {
  const f = setup(), caller = new AbortController(), task = f.controller.select('PARADE', caller.signal)
  await f.controller.select('DANGER'); caller.abort(); f.pending.resolve(f.map('PARADE')); await task
  expect(f.controller.read()).toMatchObject({ phase: 'ready', selected: 'DANGER', pending: null }); expect(f.write).toHaveBeenCalledExactlyOnceWith({ userId: 'user-a', eventId: 'event-a' }, 'DANGER')
})
it('leaves an already committed selection intact after caller abort or controller disposal', async () => {
  const f = setup(), caller = new AbortController(); await f.controller.select('DANGER', caller.signal); caller.abort()
  expect(f.controller.read()).toMatchObject({ selected: 'DANGER', phase: 'ready', pending: null }); f.controller.dispose(); expect(f.skin()).toBe('DANGER')
})
it('preaborted selection does not interrupt an existing valid request', async () => {
  const f = setup(), task = f.controller.select('PARADE'), caller = new AbortController(); caller.abort()
  await f.controller.select('DANGER', caller.signal); expect(f.controller.read().pending).toBe('PARADE')
  f.pending.resolve(f.map('PARADE')); await task; expect(f.skin()).toBe('PARADE')
})
it('disposal synchronously detaches caller cancellation and rejects a late candidate', async () => {
  const f = setup(), caller = new AbortController(), remove = vi.spyOn(caller.signal, 'removeEventListener')
  const task = f.controller.select('PARADE', caller.signal); f.controller.dispose(); expect(remove).toHaveBeenCalled()
  f.pending.resolve(f.map('PARADE')); await task
  expect(f.dispose).toHaveBeenCalledOnce(); expect(f.controller.read().phase).toBe('disposed'); expect(f.write).not.toHaveBeenCalled()
})
it('reports the actual indivisibly committed skin if cancellation occurs at visual commit', async () => {
  const caller = new AbortController(), write = vi.fn(); let skin: ThemeId = 'REVENGE'
  const controller = createThemeController({ initial: skin, scope: { userId: 'a', eventId: 'b' }, preferences: { read: () => null, write },
    load: async () => { throw Error('unused') }, runtime: { readSkin: () => skin, replaceSkin: async () => { skin = 'DANGER'; caller.abort() } } })
  await controller.select('DANGER', caller.signal)
  expect(controller.read()).toMatchObject({ phase: 'ready', selected: 'DANGER', pending: null, saved: false }); expect(write).not.toHaveBeenCalled()
})
it('does not revive a cancelled loading state through a reentrant subscriber', async () => {
  const f = setup(), caller = new AbortController()
  f.controller.subscribe(() => { if (f.controller.read().phase === 'loading') caller.abort() })
  await f.controller.select('PARADE', caller.signal)
  expect(f.controller.read()).toMatchObject({ phase: 'ready', selected: 'REVENGE', pending: null }); expect(f.write).not.toHaveBeenCalled()
})
